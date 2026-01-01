// alias_lib/src/lib.rs

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

// --- Macros ---
#[macro_export]
macro_rules! qprintln {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet { println!($($arg)*); }
    };
}

// --- Shared Constants ---
pub const ENV_ALIAS_FILE: &str = "ALIAS_FILE";
pub const ENV_ALIAS_OPTS: &str = "ALIAS_OPTS";
pub const ENV_EDITOR: &str = "EDITOR";
pub const ENV_VISUAL: &str = "VISUAL";
pub const DEFAULT_ALIAS_FILENAME: &str = "aliases.doskey";
pub const FALLBACK_EDITOR: &str = "notepad";
pub const REG_SUBKEY: &str = "Software\\Microsoft\\Command Processor";
pub const REG_VALUE_NAME: &str = "AutoRun";

// --- Enums ---
#[derive(Debug, Clone, Copy)]
pub enum HelpMode { Short, Full }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetupStatus {
    Synced,   // ✅ Everything is good
    Mismatch, // ⚠️ AutoRun exists but doesn't point to us
    NotFound, // ❌ Not installed
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum OutputMode {
    Silent,
    Normal,
    DataOnly, // Returns the string but prints nothing
}

impl OutputMode {
    pub fn set_quiet(quiet: bool) -> Self {
        if quiet { Self::Silent } else { Self::Normal }
    }
    pub fn is_quiet(&self) -> bool {
        match self {
            OutputMode::Silent => true,
            OutputMode::DataOnly => true,
            OutputMode::Normal => false,
        }
    }
}

pub fn print_results(results: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    for line in results {
        println!("{}", line);
    }
    Ok(())
}

#[derive(Debug, PartialEq, Clone)]
pub enum AliasAction {
    Set (SetOptions),
    Query(String),
    Edit(Option<String>),
    Clear,
    Help,
    Reload,
    Setup,
    ShowAll,
    Which,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasEntry {
    pub name: String,
    pub value: String, // This holds the full RHS, including any "quotes"
}

// export necessities.
pub trait AliasProvider {
    fn purge_ram_macros() -> io::Result<PurgeReport>;
    fn reload_full(path: &Path, quiet: bool) -> io::Result<()>;
    fn query_alias(name: &str, mode: OutputMode) -> Vec<String>;
    fn set_alias(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()>;
    fn run_diagnostics(path: &Path);
    fn alias_show_all();
    fn install_autorun(quiet: bool) -> io::Result<()>;
}

pub fn run<P: AliasProvider>(   action: AliasAction,
                                quiet: bool,
                                path: &Path )
                             -> Result<(), Box<dyn std::error::Error>> {
    let mode = OutputMode::set_quiet(quiet);

    // 1. Debug Block (The Wrapper legacy - always helpful for devs)
    #[cfg(debug_assertions)]
    if !quiet {
        eprintln!("--- DEBUG: Running Action {:?} ---", action);
    }

    match action {
        AliasAction::Clear => {
            qprintln!(quiet, "🧹 Clearing RAM macros...");
            let report = P::purge_ram_macros()?; // Success from either lib

            if !report.cleared.is_empty() {
                qprintln!(quiet, "✨ Removed {} aliases.", report.cleared.len());
            }
            if !report.failed.is_empty() {
                eprintln!("⚠️ Failed to clear {} aliases (protected by host).", report.failed.len());
            }
            if report.cleared.is_empty() && report.failed.is_empty() {
                qprintln!(quiet, "ℹ️ RAM was already empty.");
            }
        }

        AliasAction::Reload => {
            qprintln!(quiet, "🔄 Syncing RAM with {}...", path.display());
            P::reload_full(path, quiet)?;
        }

        AliasAction::ShowAll => {
            P::alias_show_all();
        }

        AliasAction::Query(term) => {
            let _ = print_results(P::query_alias(&term, mode));
        }

        AliasAction::Set(opts) => {
            // Both libs now handle the logic internally (Disk vs RAM)
            P::set_alias(opts, path, quiet)?;
        }

        AliasAction::Edit(custom_editor) => {
            open_editor(path, custom_editor, quiet)?;
            P::reload_full(path, quiet)?; // Using aligned 2-arg signature
            qprintln!(quiet, "✨ Aliases reloaded after edit.");
        }

        AliasAction::Which => {
            // Each lib provides its own specific diagnostics report
            P::run_diagnostics(path);
        }

        AliasAction::Setup => {
            qprintln!(quiet, "🛠️  Setting up Windows AutoRun hook...");
            P::install_autorun(quiet)?;
        }

        AliasAction::Help => print_help(HelpMode::Full, Some(path)),

        AliasAction::Invalid => {
            eprintln!("❌ Invalid command.");
            print_help(HelpMode::Short, Some(path));
        }
    }

    Ok(())
}

impl AliasEntry {
    pub fn parse(raw: &str) -> Option<Self> {
        raw.split_once('=').and_then(|(n, v)| {
            let name = n.trim();
            // --- THE INJECTION ---
            if is_valid_name(name) {
                Some(Self {
                    name: name.to_string(),
                    value: v.to_string(),
                })
            } else {
                None // Skip garbage lines in the file
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct AliasEntryMesh {
    pub name: String,
    pub os_value: Option<String>,   // None = Missing, Some("") = Empty Value
    pub file_value: Option<String>,
}

impl AliasEntryMesh {
    // This helper specifically identifies your "cdx= beast
    pub fn is_empty_definition(&self) -> bool {
        match &self.os_value {
            Some(val) => val.is_empty(),
            None => false,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SetOptions {
    pub name: String,
    pub value: String,
    pub volatile: bool,   // If true, skip Disk Strike (The "Suppress Disk" override)
    pub force_case: bool, // If true, skip .to_lowercase() (The "Injection" override)
}

// alias_lib/src/lib.rs

pub struct PurgeReport {
    pub cleared: Vec<String>,
    pub failed: Vec<(String, u32)>, // Name and the Win32 Error Code
}

impl PurgeReport {
    pub fn is_fully_clean(&self) -> bool {
        self.failed.is_empty()
    }
}

// --- Logic Dispatcher ---

pub fn resolve_command(cmd: &str) -> Option<AliasAction> {
    match cmd {
        "--help" | "-h" | "/?" => Some(AliasAction::Help),
        "--which"  => Some(AliasAction::Which),
        "--reload" => Some(AliasAction::Reload),
        "--clear"  => Some(AliasAction::Clear),
        "--setup"  => Some(AliasAction::Setup),
        _ => None,
    }
}

/// The final word: No spaces anywhere, must be alphanumeric start.
pub fn is_valid_name(name: &str) -> bool {
    // If it contains a space anywhere (leading, trailing, or middle), it's out.
    if name.contains(' ') { return false; }
    is_valid_name_loose(name)
}

// The initial guard: Must be alphanumeric start.
// (Allows spaces later in the string for the "Beast" value).
pub fn is_valid_name_loose(name: &str) -> bool {
    if name.is_empty() { return false; }
    let first = name.chars().next().unwrap();

    // Allows A-Z, a-z, 0-9 AND international letters (like ñ or λ)
    // but REJECTS weird Unicode symbols/numbers that aren't plain digits.
    first.is_alphabetic() || first.is_ascii_digit()
}

pub fn parse_alias_args(args: &[String]) -> (AliasAction, bool) {
    // 1. Detect Flags (Global/Pre-command)
    let quiet = args.iter().any(|a| matches!(a.to_lowercase().as_str(), "--quiet"));
    let volatile = args.iter().any(|a| a.to_lowercase() == "--temp");
    let force_case = args.iter().any(|a| matches!(a.to_lowercase().as_str(), "--force"));

    // 2. Filter out ALL flags so only "commands" and "content" remain
    let f_args: Vec<String> = args.iter()
        .filter(|a| {
            let low = a.to_lowercase();
            !matches!(low.as_str(), "--quiet" |"--temp" | "--force")
        })
        .cloned()
        .collect();

    // 3. Minimum args check (just the exe name left)
    if f_args.len() < 2 {
        return (AliasAction::ShowAll, quiet);
    }

    // Use the first non-flag argument to determine intent
    let first = f_args[1].to_lowercase();

    // 4. Resolve Keywords (clear, reload, etc.)
    if let Some(action) = resolve_command(&first) {
        return (action, quiet);
    }

    // 5. Handle Editor override (e.g., alias --edalias=nano)
    if first.starts_with("--edalias") {
        let ed = first.split_once('=')
            .map(|(_, e)| e.trim().to_string())
            .filter(|s| !s.is_empty());
        return (AliasAction::Edit(ed), quiet);
    }

    // 6. Safety: If it's not a known command/flag but starts with '-', it's garbage.
    if !is_valid_name_loose(&first) {
        return (AliasAction::Invalid, quiet);
    }

    // 7. Split Logic: Set (name=val) vs Query (name)
    // We join the REMAINING filtered args to allow spaces in values
    let input = f_args[1..].join(" ");
    let delim = if input.contains('=') { '=' } else { ' ' };

    if let Some((n, v)) = input.split_once(delim) {
        let name = n.trim();
        if !is_valid_name(name) {
            return (AliasAction::Invalid, quiet);
        }
        if name.is_empty() {
            (AliasAction::Invalid, quiet)
        } else {
            (AliasAction::Set(SetOptions {
                name: name.to_string(),
                value: v.trim().to_string(),
                volatile,    // <--- WIRED UP
                force_case,  // <--- WIRED UP
            }), quiet)
        }
    } else if f_args.len() == 2 {
        (AliasAction::Query(f_args[1].clone()), quiet)
    } else {
        (AliasAction::Invalid, quiet)
    }
}

// --- Path & File Persistence ---

pub fn get_alias_path() -> Option<PathBuf> {
    // Check override first
    if let Ok(val) = env::var(ENV_ALIAS_FILE) {
        let p = PathBuf::from(val);
        return Some(if p.is_dir() { p.join(DEFAULT_ALIAS_FILENAME) } else { p });
    }

    // Default search
    ["APPDATA", "USERPROFILE"].iter()
        .filter_map(|var: &&str| {
            let base = env::var(var).ok().map(PathBuf::from)?;
            let sub = if *var == "APPDATA" { "alias_tool" } else { "" };
            Some(base.join(sub).join(DEFAULT_ALIAS_FILENAME))
        })
        .find(|p| is_path_healthy(p))
}

pub fn is_path_healthy(p: &Path) -> bool {
    if !p.exists() {
        return p.parent().map_or(false, |parent| parent.exists() && parent.is_dir());
    }
    fs::metadata(p).map(|m| !m.permissions().readonly()).unwrap_or(false)
}

pub fn parse_macro_file(path: &Path) -> Vec<(String, String)> {
    fs::read_to_string(path).unwrap_or_default()
        .lines()
        // 1. Clean the whitespace
        .map(|l| l.trim())
        // 2. Ignore empty lines
        .filter(|l| !l.is_empty())
        // 3. Try to split
        .filter_map(|l| l.split_once('='))
        // 4. Validate the name (Kills #, //, ::, /*, and spaces)
        .filter(|(k, _)| is_valid_name(k.trim()))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn update_disk_file(name: &str, value: &str, path: &Path) -> io::Result<()> {
    if !is_valid_name(name) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Refusing to write illegal alias name to disk."));
    }
    let content = fs::read_to_string(path).unwrap_or_default();
    let search = format!("{}=", name.trim().to_lowercase());

    let mut lines: Vec<String> = content.lines()
        .filter(|l| !l.to_lowercase().starts_with(&search))
        .map(String::from).collect();

    if !value.is_empty() { lines.push(format!("{}={}", name.trim(), value)); }

    let output = lines.join("\n") + if lines.is_empty() { "" } else { "\n" };
    fs::write(path, output)
}

// --- OS Interop ---

pub fn open_editor(path: &Path, override_ed: Option<String>, quiet: bool) -> io::Result<()> {
    let ed = override_ed.or_else(|| env::var(ENV_VISUAL).ok()).or_else(|| env::var(ENV_EDITOR).ok())
        .unwrap_or_else(|| FALLBACK_EDITOR.to_string());

    qprintln!(quiet, "🚀 Launching {}...", ed);
    if Command::new(&ed).arg(path).status().is_err() {
        Command::new(FALLBACK_EDITOR).arg(path).status()?;
    }
    Ok(())
}

pub fn print_help(mode: HelpMode, path: Option<&std::path::Path>) {
    // Shared Header
    println!("{} (Rust) - High-speed alias management", "\x1b[1;36mALIAS\x1b[0m");

    if let HelpMode::Full = mode {
        println!(r#"
  {}
    alias                       List active macros
    alias <name>                Search for a specific macro
    alias <name>=[value]        Set or delete (if empty) a macro
    alias <name> [value]        Set a macro (alternate syntax)

  {}
    -h, --help                  Show this help menu
    -q, --quiet                 Suppress success output
    -r, --reload                Force reload from .doskey file
    -s, --setup                 Install AutoRun registry hook
    -w, --which                 Display the path to the current alias file
    -e, --edalias[=EDITOR]      Open alias file (Default: notepad)

  {}
    ALIAS_FILE                  Path to your .doskey file
    ALIAS_OPTS                  Default flags (e.g. "--quiet")"#,
                 "\x1b[1;33mUSAGE:\x1b[0m",
                 "\x1b[1;33mFLAGS:\x1b[0m",
                 "\x1b[1;33mENVIRONMENT:\x1b[0m"
        );
    }

    // The logic that uses the path you're passing in
    if let Some(p) = path {
        println!("\n  \x1b[1;32mCURRENT FILE:\x1b[0m");
        println!("    {}", p.display());
    } else {
        println!("\n  \x1b[1;31mCURRENT FILE:\x1b[0m\n    None (Use ALIAS_FILE to set)");
    }
    println!(); // Trailing newline for cleanliness
}

pub fn query_alias_file(name: &str, path: &Path, mode: OutputMode) -> Vec<String> {
    let mut results = Vec::new();
    let search = format!("{}=", name.to_lowercase());

    // Read the file (The Source of Truth)
    let content = std::fs::read_to_string(path).unwrap_or_default();

    let found = content
        .lines()
        .find(|line| line.to_lowercase().starts_with(&search));

    if let Some(line) = found {
        results.push(line.to_string());
    } else if !mode.is_quiet() {
        // Only push the "Not Known" message if we aren't in Silent/DataOnly mode
        results.push(format!("{} is not a known alias in the config file.", name));
    }

    results
}

pub fn dump_alias_file() -> Vec<(String, String)> {
    // 1. Get the path using your fallback logic
    let Some(path) = get_alias_path() else {
        return vec![]; // No file found, file half of mesh will be empty
    };

    // 2. Read and parse into (Name, Value) pairs
    let content = std::fs::read_to_string(path).unwrap_or_default();

    content.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with(';'))
        .filter_map(|line| {
            // Split once at '=' to preserve quotes/the "Beast"
            line.split_once('=').map(|(n, v)| (n.to_string(), v.to_string()))
        })
        .collect()
}

pub fn inject_env_options(args: &mut Vec<String>) {
    // 1. Check for ENV_ALIAS_OPTS (e.g., "--quiet")
    if let Ok(env_opts) = env::var(ENV_ALIAS_OPTS) {
        // Only inject if the user hasn't already provided options
        // Or simply append them to the start so CLI overrides can still happen
        for opt in env_opts.split_whitespace() {
            if !args.contains(&opt.to_string()) {
                // Insert after the executable name (index 0)
                args.insert(1, opt.to_string());
            }
        }
    }

    // 2. Check for ENV_ALIAS_FILE (The DB location)
    if let Ok(env_file) = env::var(ENV_ALIAS_FILE) {
        // If the user didn't specify --file manually in the CLI
        if !args.contains(&"--file".to_string()) && !args.contains(&"-f".to_string()) {
            args.push("--file".to_string());
            args.push(env_file);
        }
    }
}

pub fn calculate_new_file_state(current_content: &str, name: &str, value: &str) -> String {
    let search_target = format!("{}=", name.to_lowercase());

    // Filter out the old line if it exists
    let mut lines: Vec<String> = current_content
        .lines()
        .filter(|line| !line.to_lowercase().starts_with(&search_target))
        .map(|s| s.to_string())
        .collect();

    // Add the new line if it's not a deletion (empty value)
    if !value.is_empty() {
        lines.push(format!("{}={}", name, value));
    }

    let mut result = lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

pub fn mesh_logic(os_list: Vec<(String, String)>, file_list: Vec<(String, String)>) -> Vec<AliasEntryMesh> {
    let mut mesh_list: Vec<AliasEntryMesh> = os_list
        .into_iter()
        .map(|(n, v)| AliasEntryMesh {
            name: n,
            os_value: Some(v), // Even if v is "", it is Some("")
            file_value: None,
        })
        .collect();

    for (f_name, f_val) in file_list {
        if let Some(existing) = mesh_list.iter_mut().find(|e| e.name == f_name) {
            existing.file_value = Some(f_val);
        } else {
            mesh_list.push(AliasEntryMesh {
                name: f_name,
                os_value: None,
                file_value: Some(f_val),
            });
        }
    }
    mesh_list
}

pub fn display_audit(mesh_list: &[AliasEntryMesh]) {
    // 1. Calculate max length for the "Far Right" alignment
    let max_len = mesh_list.iter()
        .map(|e| {
            let val = e.os_value.as_deref().unwrap_or("<MISSING>");
            format!("{}={}", e.name, val).len()
        })
        .max()
        .unwrap_or(20);

    // 2. Iterate and print with "The Beast" detection
    for entry in mesh_list {
        let display_val = match &entry.os_value {
            Some(v) if v.is_empty() => "<EMPTY>",
            Some(v) => v,
            None => "<NOT IN OS>",
        };

        let line = format!("{}={}", entry.name, display_val);

        let o = if entry.os_value.is_some() { "O" } else { " " };
        let f = if entry.file_value.is_some() { "F" } else { " " };

        // Determine if we have a mismatch (The Beast or just different values)
        let mut alert = "";
        if let (Some(os), Some(fi)) = (&entry.os_value, &entry.file_value) {
            if os != fi {
                alert = " !! MISMATCH !!";
                if os.is_empty() { alert = " !! GHOST EMPTY !!"; }
            }
        }

        // The "Far Right" Print: Pad to max_len + 5
        println!("{:width$} [{}{}] {}", line, o, f, alert, width = max_len + 5);
    }
}

pub fn perform_audit(os_pairs: Vec<(String, String)>) {
    // 1. Get the File data using the healthy path logic
    let file_pairs = dump_alias_file();

    // 2. Mesh them
    let mesh = mesh_logic(os_pairs, file_pairs);

    // 3. Display with the Far-Right alignment
    display_audit(&mesh);
}

pub fn perform_triple_audit(
    win32_pairs: Vec<(String, String)>,
    mut wrap_pairs: Vec<(String, String)>,
    mut file_pairs: Vec<(String, String)>
) {
    let mut desync_detected = false;
    let max_len = win32_pairs.iter()
        .map(|(n, v)| format!("{}={}", n, v).len())
        .max()
        .unwrap_or(35);

    println!("--- 🔍 Triple Audit [W=Win32, D=Doskey, F=File] ---");

    // 1. Primary Loop: Win32 is the source of truth for "What is Active"
    for (name, w_val) in win32_pairs {
        let d_idx = wrap_pairs.iter().position(|(n, _)| n == &name);
        let f_idx = file_pairs.iter().position(|(n, _)| n == &name);

        let d_val = d_idx.map(|i| wrap_pairs.remove(i).1);
        let f_val = f_idx.map(|i| file_pairs.remove(i).1);

        let icons = format!("[W{}{}]",
                            if d_val.is_some() { "D" } else { " " },
                            if f_val.is_some() { "F" } else { " " }
        );

        let display_val = if w_val.is_empty() { "<EMPTY>" } else { &w_val };
        println!("{:width$} {}", format!("{}={}", name, display_val), icons, width = max_len + 5);

        // Discrepancy Detection
        if let Some(dv) = &d_val {
            if &w_val != dv {
                println!("  ⚠️  VALUE DESYNC: Doskey wrapper sees \"{}\"", dv);
                desync_detected = true;
            }
        }
        if let Some(fv) = &f_val {
            if &w_val != fv {
                println!("  ❌ FILE MISMATCH: File expects \"{}\"", fv);
                desync_detected = true;
            }
        }
    }

    // 2. Leftovers in Doskey (Phantom entries)
    for (name, d_val) in wrap_pairs {
        let f_idx = file_pairs.iter().position(|(n, _)| n == &name);
        let f_val = f_idx.map(|i| file_pairs.remove(i).1);
        println!("{:<width$} [ D{}] !! PHANTOM (In Doskey, not Win32) !!",
                 format!("{}={}", name, d_val),
                 if f_val.is_some() { "F" } else { " " },
                 width = max_len + 5);
        desync_detected = true;
    }

    // 3. Leftovers in File (Pending entries)
    for (name, f_val) in file_pairs {
        println!("{:<width$} [  F] !! PENDING (In File, not OS) !!",
                 format!("{}={}", name, f_val),
                 width = max_len + 5);
        desync_detected = true;
    }

    // --- The Reminder ---
    if desync_detected {
        println!("\n\x1b[1;33m💡 Tip: Out of sync? Run `alias --reload` to align RAM with your config file.\x1b[0m");
    }
}
