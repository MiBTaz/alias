// alias_wrapper/src/lib.rs

use std::{env, fs, io};
use std::io::Read;
use std::path::Path;
use std::process::Command;

// 1. Import the constants explicitly
// 2. Import the macro using the crate name
use alias_lib::*;
use alias_lib::qprintln;

pub fn reload_doskey(path: &Path) -> io::Result<()> {
    Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()
        .map(|_| ())
}

pub fn set_alias(name: &str, value: &str, path: &Path, quiet: bool) -> io::Result<()> {
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

/// Perfroms a "Hard Sync": Wipes RAM then loads from disk
pub fn reload_full(path: &Path) -> io::Result<()> {
    clear_ram_macros()?;
    Command::new("doskey")
        .arg(format!("/macrofile={}", path.display()))
        .status()
        .map(|_| ())
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

pub fn clear_ram_macros() -> io::Result<()> {
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