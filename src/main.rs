// src/main.rs

#[cfg(test)]
mod tests;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

// --- Macros ---
macro_rules! qprintln {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            println!($($arg)*);
        }
    };
}

// --- Constants (Zero Magic Zone) ---
const ENV_ALIAS_FILE: &str = "ALIAS_FILE";
const ENV_ALIAS_OPTS: &str = "ALIAS_OPTS";
const ENV_EDITOR: &str = "EDITOR";
const ENV_VISUAL: &str = "VISUAL";
const DEFAULT_ALIAS_FILENAME: &str = "aliases.doskey";
const FALLBACK_EDITOR: &str = "notepad";

#[derive(Debug, PartialEq)]
pub(crate) enum AliasAction {
    ShowAll,
    Query(String),
    Set { name: String, value: String },
    Reload,
    Clear,
    Edit(Option<String>),
    Which,
    Help,
    Setup,
    Invalid,
}

pub enum HelpMode { Short, Full }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();

    #[cfg(debug_assertions)]
    {
        eprintln!("--------------------------------------------------");
        eprintln!("DEBUG [Raw OS Input]: {:?}", args);
        eprintln!("--------------------------------------------------");
    }

    if let Ok(env_opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = env_opts.split_whitespace().map(String::from).collect();
        if !extra.is_empty() {
            args.splice(1..1, extra);
        }
    }

    let (action, quiet) = parse_alias_args(&args);

    let alias_path = match get_alias_path() {
        Some(path) => path,
        None => {
            return Err(format!(
                "❌ Error: No usable alias file found. Set %{}% or create '{}' in APPDATA.",
                ENV_ALIAS_FILE, DEFAULT_ALIAS_FILENAME
            ).into());
        }
    };

    match action {
        AliasAction::Clear => {
            qprintln!(quiet, "🧹 Clearing RAM macros...");
            clear_ram_macros()?;
            qprintln!(quiet, "✨ RAM is now empty.");
        },
        AliasAction::Reload => {
            qprintln!(quiet, "🔄 Syncing RAM with {}...", alias_path.display());
            reload_full(&alias_path)?;
            qprintln!(quiet, "✨ Reload complete.");
        },
        AliasAction::ShowAll => {
            Command::new("doskey").arg("/macros:all").status()?;
        }
        AliasAction::Query(term) => {
            query_alias(&term, &alias_path)?;
        }
        AliasAction::Set { name, value } => {
            if value.is_empty() {
                // Remove from RAM immediately
                Command::new("doskey").arg(format!("{}=", name)).status()?;
            }
            set_alias(&name, &value, &alias_path, quiet)?;
        }
        AliasAction::Edit(custom_editor) => {
            open_editor(&alias_path, custom_editor, quiet)?;
            reload_full(&alias_path)?;
            qprintln!(quiet, "✨ Aliases reloaded after edit.");
        }
        AliasAction::Which => run_diagnostics(&alias_path),
        AliasAction::Help => print_help(HelpMode::Full),
        AliasAction::Invalid => {
            eprintln!("❌ Invalid command.");
            print_help(HelpMode::Short);
        }
        AliasAction::Setup => {
            qprintln!(quiet, "🛠️  Setting up Windows AutoRun hook...");
            if let Err(e) = install_autorun(quiet) {
                eprintln!("❌ Setup failed: {}", e);
            } else {
                qprintln!(quiet, "✅ Success! Your aliases are now global.");
            }
        }
    }

    Ok(())
}

// --- Logic Functions ---

fn reload_doskey(path: &Path) -> io::Result<()> {
    Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()
        .map(|_| ())
}

pub(crate) fn get_alias_path() -> Option<PathBuf> {
    if let Ok(val) = env::var(ENV_ALIAS_FILE) {
        let p = PathBuf::from(val);
        return Some(if p.is_dir() { p.join(DEFAULT_ALIAS_FILENAME) } else { p });
    }
    let mut candidates = Vec::new();
    if let Ok(app) = env::var("APPDATA") {
        candidates.push(PathBuf::from(app).join("alias_tool").join(DEFAULT_ALIAS_FILENAME));
    }
    if let Ok(user) = env::var("USERPROFILE") {
        candidates.push(PathBuf::from(user).join(DEFAULT_ALIAS_FILENAME));
    }
    candidates.into_iter().find(|p| is_path_healthy(p))
}

fn is_path_healthy(p: &Path) -> bool {
    if p.exists() {
        return fs::metadata(p).map(|m| !m.permissions().readonly()).unwrap_or(false);
    }
    p.parent().map_or(false, |parent| parent.exists() && parent.is_dir())
}

fn set_alias(name: &str, value: &str, path: &Path, quiet: bool) -> io::Result<()> {
    let content = fs::read_to_string(path).unwrap_or_default();
    // Trim the name just in case a space sneaked through the parser
    let clean_name = name.trim();
    let search = format!("{}=", clean_name.to_lowercase());

    let mut lines: Vec<String> = content.lines()
        .filter(|l| !l.to_lowercase().starts_with(&search))
        .map(|l| l.to_string())
        .collect();

    if !value.is_empty() {
        lines.push(format!("{}={}", name, value));
    }

    fs::write(path, lines.join("\n") + "\n")?;
    reload_doskey(path)?;

    if value.is_empty() {
        qprintln!(quiet, "🗑️  Deleted alias: {}", name);
    } else {
        qprintln!(quiet, "✨ Set alias: {}={}", name, value);
    }
    Ok(())
}

fn open_editor(path: &Path, override_editor: Option<String>, quiet: bool) -> io::Result<()> {
    let editor = override_editor
        .or_else(|| env::var(ENV_VISUAL).ok())
        .or_else(|| env::var(ENV_EDITOR).ok())
        .unwrap_or_else(|| FALLBACK_EDITOR.to_string());

    qprintln!(quiet, "🚀 Launching {}...", editor);
    if Command::new(&editor).arg(path).status().is_err() {
        Command::new(FALLBACK_EDITOR).arg(path).status()?;
    }
    Ok(())
}

fn print_help(mode: HelpMode) {
    if let HelpMode::Full = mode {
        println!("🚀 Alias Tool v1.0 (Rust Edition)");
        println!("\nUsage: alias [FLAGS] [COMMAND]");
    } else {
        println!("\nUsage: alias --help for full details");
    }

    println!("\nCore Commands:");
    println!("  alias                 Show all active macros in current RAM");
    println!("  alias <name>=<val>    Set/update alias (e.g., alias ls=dir /w)");
    println!("  alias <name> <val>    Set/update alias using space separation");
    println!("  alias <name>=         Delete alias from both RAM and disk");
    println!("  alias <name>          Search for a specific alias definition");

    if let HelpMode::Full = mode {
        println!("\nEnvironment Management:");
        println!("  --reload              🔄 Hard Sync: Wipes RAM and reloads from disk");
        println!("  --clear               🧹 Panic Button: Wipes all macros from current RAM");
        println!("  --edalias[=editor]    📝 Open alias file (Auto-reloads on save)");
        println!("  --edaliasas[=editor]  📝 Synonym for --edalias)");
        println!("  --which               🔍 Run system & path diagnostics");

        println!("\nGlobal Flags:");
        println!("  --quiet               🤫 Suppress success messages (useful for scripts)");
        println!("  --help, -h            ❓ Show this menu");

        println!("\nConfiguration:");
        let current_path = get_alias_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "Not Found".into());
        println!("  File Loc:  {}", current_path);
        println!("  Editor:    {} (Override with %{}% or %{}%)", FALLBACK_EDITOR, ENV_VISUAL, ENV_EDITOR);
    } else {
        println!("\n💡 Tip: Use 'alias --reload' if your shell gets out of sync with your file.");
    }
}

fn run_diagnostics(path: &Path) {
    println!("--- 🛠️  Alias Tool Diagnostics ---");
    if let Ok(p) = env::current_exe() { println!("Binary Loc:    {}", p.display()); }

    // Cleaned up Env Var display (no % signs)
    let env_file = env::var(ENV_ALIAS_FILE).unwrap_or_else(|_| "NOT SET".into());
    let env_opts = env::var(ENV_ALIAS_OPTS).unwrap_or_else(|_| "NOT SET".into());

    println!("Env Var:       {} = \"{}\"", ENV_ALIAS_FILE, env_file);
    println!("Env Var:       {} = \"{}\"", ENV_ALIAS_OPTS, env_opts);
    println!("Resolved Path: {}", path.display());

    match path.metadata() {
        Ok(m) => {
            println!("File Status:   EXISTS {}", if m.permissions().readonly() { "(READ-ONLY ⚠️)" } else { "(WRITABLE ✅)" });
            // Simple check to see if the drive is alive
            if let Ok(mut f) = fs::File::open(path) {
                let mut buf = [0; 1];
                let _ = f.read(&mut buf);
                println!("Drive Status:  RESPONSIVE ⚡");
            }
        }
        Err(_) => println!("File Status:   MISSING OR INACCESSIBLE ❌"),
    }

    println!("\nRegistry Check (AutoRun):");
    let reg = Command::new("reg").args(["query", "HKCU\\Software\\Microsoft\\Command Processor", "/v", "AutoRun"]).output();
    if let Ok(out) = reg {
        let s = String::from_utf8_lossy(&out.stdout);
        // Checking if the current resolved path is actually in the AutoRun string
        if s.contains(&path.to_string_lossy().into_owned()) || s.contains("alias") {
            println!("  Status:      SYNCED ✅");
        } else {
            println!("  Status:      MISMATCH/NOT FOUND ⚠️");
        }
    }
}

pub(crate) fn parse_alias_args(args: &[String]) -> (AliasAction, bool) {
    let quiet = args.iter().any(|arg| arg.to_lowercase() == "--quiet");
    let f_args: Vec<String> = args.iter()
        .filter(|arg| arg.to_lowercase() != "--quiet")
        .cloned()
        .collect();

    if f_args.len() < 2 { return (AliasAction::ShowAll, quiet); }

    let first = f_args[1].to_lowercase();

    // -- 1. Explicit Commands & Known Short Flags --
    if first == "--help" || first == "-h" || first == "/?" { return (AliasAction::Help, quiet); }
    if first == "--which" { return (AliasAction::Which, quiet); }
    if first == "--reload" { return (AliasAction::Reload, quiet); }
    if first == "--clear"  { return (AliasAction::Clear, quiet) };
    if first == "--setup"  { return (AliasAction::Setup, quiet) };


    // -- 2. Editor Variants --
    if first.starts_with("--edalias") {
        // This covers --edalias, --edaliases, and anything starting with them
        if first == "--edalias" || first == "--edaliases" {
            return (AliasAction::Edit(None), quiet);
        }
        if let Some(pos) = first.find('=') {
            let ed = &first[pos + 1..];
            let editor = if ed.is_empty() { None } else { Some(ed.to_string()) };
            return (AliasAction::Edit(editor), quiet);
        }
    }

    // -- 3. Safety Trap: Reject ALL other dashes --
    // This prevents -f, -r, or --anything-else from being used as names.
    if first.starts_with("-") { return (AliasAction::Invalid, quiet); }

    match f_args.len() {
        2 => {
            if f_args[1].contains('=') {
                let parts: Vec<&str> = f_args[1].splitn(2, '=').collect();
                let name = parts[0].trim().to_string();
                if name.is_empty() { return (AliasAction::Invalid, quiet); }
                (AliasAction::Set { name, value: parts[1].trim().to_string() }, quiet)
            } else {
                (AliasAction::Query(f_args[1].clone()), quiet)
            }
        }
        _ => {
            let input = f_args[1..].join(" ");
            let parts: Vec<&str> = if input.contains('=') {
                input.splitn(2, '=').collect()
            } else {
                input.splitn(2, ' ').collect()
            };

            if parts.len() == 2 {
                let name = parts[0].trim().to_string();
                if name.is_empty() { return (AliasAction::Invalid, quiet); }
                (AliasAction::Set { name, value: parts[1].trim().to_string() }, quiet)
            } else {
                (AliasAction::Invalid, quiet)
            }
        }
    }
}

fn query_alias(term: &str, path: &Path) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let search = format!("{}=", term.to_lowercase());
    match content.lines().find(|l| l.to_lowercase().starts_with(&search)) {
        Some(line) => println!("{}", line),
        None => println!("Alias \"{}\" not found.", term),
    }
    Ok(())
}

/// Perfroms a "Hard Sync": Wipes RAM then loads from disk
fn reload_full(path: &Path) -> io::Result<()> {
    clear_ram_macros()?;
    Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()
        .map(|_| ())
}

/// Hooks the tool into the CMD AutoRun registry key
fn install_autorun(quiet: bool) -> io::Result<()> {
    let exe_path = env::current_exe()?;

    // We use --reload so every new shell is fresh
    let command = format!("\"{}\" --reload", exe_path.display());

    qprintln!(quiet, "🔗 Target: {}", command);

    let status = Command::new("reg")
        .args([
            "add",
            "HKCU\\Software\\Microsoft\\Command Processor",
            "/v", "AutoRun",
            "/t", "REG_EXPAND_SZ",
            "/d", &command,
            "/f"
        ])
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Registry update failed. Check permissions."
        ));
    }

    Ok(())
}

fn clear_ram_macros() -> io::Result<()> {
    let output = Command::new("doskey").arg("/macros").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') { continue; }

        if let Some(pos) = line.find('=') {
            let name = &line[..pos];

            // DIRECT CALL: No 'cmd /c', no extra shell layer.
            // This is likely how it was working when it worked.
            let _ = Command::new("doskey")
                .arg(format!("{}=", name))
                .status();
        }
    }
    Ok(())
}