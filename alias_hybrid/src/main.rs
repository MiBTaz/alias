use std::{env, io};
use std::path::Path;
use std::process::Command;


// The Handshake - Accesses the shared brain in lib.rs
use alias_lib::*;
use alias_wrapper::*;


// Win32 API Imports for the primary strike
use windows_sys::Win32::System::Console::{AddConsoleAliasA, GetConsoleAliasesLengthA, GetConsoleAliasesA};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();

    // 1. Prep & Parse
    inject_env_options(&mut args);
    let (action, quiet) = parse_alias_args(&args);
    let alias_path = get_alias_path().ok_or("❌ No alias file found.")?;
    let mode = OutputMode::set_quiet(quiet);

    // 2. The Hybrid Dispatcher
    match action {
        AliasAction::Clear => {
            qprintln!(quiet, "🧹 Clearing RAM...");
            if !api_purge_all_macros() {
                // FALLBACK: If API fails, use the legacy scraper
                let _ = legacy_clear_ram();
            }
        },
        AliasAction::Reload => reload_hybrid(&alias_path, quiet)?,
        AliasAction::ShowAll => {
            // Try API first for speed, fallback to doskey /macros
            if !api_show_all() {
                let _ = Command::new("doskey").arg("/macros:all").status();
            }
        },
        AliasAction::Query(term) => query_alias_hybrid(&term, &alias_path, mode),
        AliasAction::Set { name, value } => set_alias_hybrid(&name, &value, &alias_path, quiet)?,
        AliasAction::Edit(ed) => {
            open_editor(&alias_path, ed, quiet)?;
            reload_hybrid(&alias_path, quiet)?;
        }
        
        AliasAction::Which => alias_win32::run_diagnostics(&alias_path),
        AliasAction::Help => print_help(HelpMode::Full, Some(&alias_path)),
        AliasAction::Setup => {
            // Setup is a one-time thing; Win32 API is safest here to avoid reg.exe issues
            qprintln!(quiet, "🛠️ Installing AutoRun hook...");
            
            let cmd = get_autorun_command(&env::current_exe()?);
            // We'll just use a simple Command call here for the hybrid's setup
            let _ = Command::new("reg")
                .args(["add", REG_SUBKEY, "/v", REG_VALUE_NAME, "/t", "REG_EXPAND_SZ", "/d", &cmd, "/f"])
                .status();
        }
        AliasAction::Invalid => print_help(HelpMode::Short, Some(&alias_path)),
    }
    Ok(())
}

// --- Hybrid Logic: The "Best of Both Worlds" ---

fn reload_hybrid(path: &Path, quiet: bool) -> io::Result<()> {
    // Attempt API Reload (Fast)
    api_purge_all_macros();
    let definitions = parse_macro_file(path);
    let mut success = false;

    for (name, value) in &definitions {
        if api_set_macro(name, Some(value)) {
            success = true;
        }
    }

    // If API failed to set anything, use Doskey as the safety net
    if !success && !definitions.is_empty() {
        qprintln!(quiet, "⚠️ API Strike failed. Falling back to Doskey...");
        Command::new("doskey")
            .arg(format!("/macrofile={}", path.display()))
            .status()?;
    } else {
        qprintln!(quiet, "✨ API Reload: {} macros synced.", definitions.len());
    }
    Ok(())
}

fn set_alias_hybrid(name: &str, value: &str, path: &Path, quiet: bool) -> io::Result<()> {
    // 1. RAM Strike (Try API, ignore failure because we hit Disk next)
    let val_opt = if value.is_empty() { None } else { Some(value) };
    if !api_set_macro(name, val_opt) {
        // Fallback RAM strike via subprocess
        let _ = Command::new("doskey").arg(format!("{}={}", name, value)).status();
    }

    // 2. Disk Strike (Lib Logic - The source of truth)
    update_disk_file(name, value, path)?;

    qprintln!(quiet, "✨ {} alias: {}", if value.is_empty() { "Deleted" } else { "Set" }, name);
    Ok(())
}

// --- Minimal API Wrappers for the Hybrid ---

fn api_set_macro(name: &str, value: Option<&str>) -> bool {
    let n_c = format!("{}\0", name);
    let v_c = value.map(|v| format!("{}\0", v));
    unsafe {
        AddConsoleAliasA(
            n_c.as_ptr(),
            v_c.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            "cmd.exe\0".as_ptr()
        ) != 0
    }
}

fn api_purge_all_macros() -> bool {
    let mut success = false;
    for_each_macro(|e| {
        if let Some((n, _)) = e.split_once('=') {
            if api_set_macro(n, None) { success = true; }
        }
    });
    success
}

fn api_show_all() -> bool {
    let mut found = false;
    for_each_macro(|e| {
        if !found { println!("[cmd.exe]"); found = true; }
        println!("{}", e);
    });
    found
}

fn for_each_macro<F: FnMut(&str)>(mut f: F) {
    let exe = "cmd.exe\0".as_ptr();
    unsafe {
        let len = GetConsoleAliasesLengthA(exe);
        if len > 0 {
            let mut buf = vec![0u8; len as usize];
            let read = GetConsoleAliasesA(buf.as_mut_ptr(), len, exe);
            String::from_utf8_lossy(&buf[..read as usize])
                .split('\0')
                .filter(|e| !e.is_empty())
                .for_each(|e| f(e));
        }
    }
}

fn legacy_clear_ram() -> io::Result<()> {
    let output = Command::new("doskey").arg("/macros").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some((name, _)) = line.trim().split_once('=') {
            let _ = Command::new("doskey").arg(format!("{}=", name)).status();
        }
    }
    Ok(())
}

fn query_alias_hybrid(term: &str, path: &Path, mode: OutputMode) {
    // 1. Try Win32 first (DataOnly mode ensures it stays silent if not found)
    let mut output = alias_win32::query_alias(term, OutputMode::DataOnly);

    // 2. The Fallback Trigger: If Win32 found nothing, check the file
    if output.is_empty() {
        // Now use the user's actual requested mode (Normal or Silent)
        output = alias_wrapper::query_alias(term, OutputMode::DataOnly);
    }
    if output.is_empty() {
        output = query_alias_file(term, path, mode);
    }
    // 3. The Final Output: Print whatever the winner returned
    for line in output {
        println!("{}", line);
    }
}