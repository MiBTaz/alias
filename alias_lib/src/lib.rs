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
    Set { name: String, value: String },
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

pub fn parse_alias_args(args: &[String]) -> (AliasAction, bool) {
    let quiet = args.iter().any(|a| a.to_lowercase() == "--quiet");
    let f_args: Vec<String> = args.iter()
        .filter(|a| a.to_lowercase() != "--quiet")
        .cloned().collect();

    // 1. Minimum args check (just the exe name)
    if f_args.len() < 2 { return (AliasAction::ShowAll, quiet); }
    let first = f_args[1].to_lowercase();

    // 2. Resolve flags/commands via helper
    if let Some(action) = resolve_command(&first) {
        return (action, quiet);
    }

    // 3. Handle Editor override
    if first.starts_with("--edalias") {
        let ed = first.split_once('=')
            .map(|(_, e)| e.trim().to_string())
            .filter(|s| !s.is_empty()); // Ensures "" becomes None
        return (AliasAction::Edit(ed), quiet);
    }

    // 4. Block other flags
    if first.starts_with('-') { return (AliasAction::Invalid, quiet); }

    // 5. Split Logic (Set vs Query)
    let input = f_args[1..].join(" ");
    let delim = if input.contains('=') { '=' } else { ' ' };

    if let Some((n, v)) = input.split_once(delim) {
        let name = n.trim();
        if name.is_empty() {
            (AliasAction::Invalid, quiet)
        } else {
            (AliasAction::Set { name: name.into(), value: v.trim().into() }, quiet)
        }
    } else if f_args.len() == 2 {
        // Only a query if it's exactly one word
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
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn update_disk_file(name: &str, value: &str, path: &Path) -> io::Result<()> {
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
