// alias_wrapper/src/lib.rs

use std::{env, fs, io};
use std::io::Read;
use std::path::Path;
use std::process::Command;

// 1. Import the constants explicitly
// 2. Import the macro using the crate name
use alias_lib::*;
use alias_lib::qprintln;

pub struct WrapperLibraryInterface;

impl alias_lib::AliasProvider for WrapperLibraryInterface {
    fn purge_ram_macros() -> io::Result<PurgeReport> {
        purge_ram_macros()
    }
    fn reload_full(path: &Path, quiet: bool) -> io::Result<()> {
        reload_full(path, quiet)
    }
    fn query_alias(name: &str, mode: OutputMode) -> Vec<String> {
        query_alias(name, mode)
    }
    fn set_alias(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()> {
        set_alias(opts, path, quiet)
    }
    fn run_diagnostics(path: &Path) {
        run_diagnostics(path)
    }
    fn alias_show_all() {
        alias_show_all()
    }
    fn install_autorun(quiet: bool) -> io::Result<()> {
        install_autorun(quiet)
    }
}

pub fn reload_doskey(path: &Path) -> io::Result<()> {
    Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()
        .map(|_| ())
}

pub fn set_alias(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()> {
    // 1. Respect the flags from the struct
    let name = if opts.force_case { opts.name } else { opts.name.to_lowercase() };

    // 2. The Disk Strike (Skip if --temp)
    if !opts.volatile {
        // Use your existing logic to update the .doskey file
        update_disk_file(&name, &opts.value, path)?;
    }

    // 3. The RAM Strike (Fallback to Doskey.exe)
    // We pass the name and value to doskey so it's active immediately
    let status = std::process::Command::new("doskey")
        .arg(format!("{}={}", name, opts.value))
        .status()?;

    if status.success() {
        let tag = if opts.volatile { "(volatile)" } else { "(saved)" };
        qprintln!(quiet, "✨ Wrapper set {}: {}={}", tag, name, opts.value);
    }

    Ok(())
}

/// Perfroms a "Hard Sync": Wipes RAM then loads from disk
pub fn reload_full(path: &Path, quiet: bool) -> io::Result<()> {
    // 1. Clear the current session
    purge_ram_macros()?;

    // 2. Count the macros in the file (assuming 1 macro per line)
    let content = std::fs::read_to_string(path)?;
    let count = content.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with(';')) // Ignore empty and comments
        .count();

    // 3. Execute the Doskey reload
    let status = Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()?;

    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Doskey failed to load file"));
    }

    // 4. Now 'count' exists!
    qprintln!(quiet, "✨ Wrapper Reload: {} macros injected.", count);
    Ok(())
}
/// Hooks the tool into the CMD AutoRun registry key
pub fn install_autorun(quiet: bool) -> io::Result<()> {
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

pub fn query_alias(name: &str, mode: OutputMode) -> Vec<String> {
    let mut results = Vec::new();
    let search = format!("{}=", name.to_lowercase());

    // 1. System Strike (No API - strictly spawning doskey)
    let output = std::process::Command::new("doskey")
        .args(["/macros:cmd.exe"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let found = stdout
                .lines()
                .find(|line| line.to_lowercase().starts_with(&search));

            if let Some(line) = found {
                results.push(line.to_string());
            } else if mode == OutputMode::Normal {
                results.push(format!("⚠️ '{}' not found via doskey query.", name));
            }
        }
        _ => {
            if mode == OutputMode::Normal {
                results.push("❌ Error: Wrapper failed to execute doskey.exe".to_string());
            }
        }
    }
    results
}

pub fn run_diagnostics(path: &Path) {
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

pub fn get_autorun_command(alias_path: &Path) -> String {
    // 1. Get the path to 'this' executable (the hybrid tool)
    let current_exe = env::current_exe()
        .unwrap_or_else(|_| "alias".into());

    // 2. Format it for the Registry.
    // We use /K (keep open) for cmd or just ensure it runs silently.
    // The "doskey /macrofile=" part is the fallback,
    // but we want our tool to handle it:

    format!(
        "\"{}\" --reload --file \"{}\"",
        current_exe.display(),
        alias_path.display()
    )
}

pub fn get_all_aliases() -> Vec<(String, String)> {
    let output = std::process::Command::new("doskey")
        .arg("/macros")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    output.lines()
        .filter_map(|line| {
            // split_once preserves the "Beast" (everything after the first '=')
            line.split_once('=').map(|(n, v)| (n.trim().to_string(), v.to_string()))
        })
        .collect()
}

pub fn alias_show_all() {
    // 1. Get the wrapper-specific data
    let os_pairs = get_all_aliases();

    // 2. Hand off to the WRUM engine in alias_lib
    // This will find the file, mesh them, and print the icons
    alias_lib::perform_audit(os_pairs);
}

// In alias_wrapper/src/lib.rs

pub fn purge_ram_macros() -> io::Result<PurgeReport> {
    let mut report = PurgeReport { cleared: Vec::new(), failed: Vec::new() };

    // 1. Snapshot Before
    let before = get_all_aliases();

    // 2. Perform the Purge
    for (name, _) in &before {
        let status = Command::new("doskey")
            .arg(format!("{}=", name))
            .status()?;

        if status.success() {
            report.cleared.push(name.clone());
        }
    }

    // 3. Snapshot After to find the "Unkillable" ones
    let after = get_all_aliases();
    for (name, _) in after {
        // If it's still there, it failed to delete (moved from cleared to failed)
        if let Some(pos) = report.cleared.iter().position(|x| x == &name) {
            report.cleared.remove(pos);
            report.failed.push((name, 0)); // 0 as a placeholder for Win32 Error Code
        }
    }

    Ok(report)
}