// alias_hybrid/src/main.rs

use std::{env, io};
use std::path::Path;
use std::process::Command;
use windows_sys::Win32::System::Console::{AddConsoleAliasA, GetConsoleAliasesLengthA, GetConsoleAliasesA};


// The Handshake - Accesses the shared brain in lib.rs
use alias_lib::*;
use alias_wrapper::*;
use alias_win32::*;




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
            // Trigger the forensic purge
            match alias_win32::purge_ram_macros() {
                Ok(report) => {
                    if report.is_fully_clean() {
                        qprintln!(quiet, "🧹 RAM cleared ({} macros removed).", report.cleared.len());
                    } else {
                        eprintln!("⚠️ Partial Purge! {} cleared, {} failed.", report.cleared.len(), report.failed.len());
                        for (name, code) in report.failed {
                            eprintln!("   - [{}] Refused to die (Error Code: {})", name, code);
                        }
                    }
                },
                Err(e) => eprintln!("❌ Critical failure during purge: {}", e),
            }
        },
        AliasAction::Reload => reload_hybrid(&alias_path, quiet)?,
        AliasAction::ShowAll => {
            hybrid_show_all();
        },
        AliasAction::Query(term) => query_alias_hybrid(&term, &alias_path, mode),
        AliasAction::Edit(ed) => {
            open_editor(&alias_path, ed, quiet)?;
            reload_hybrid(&alias_path, quiet)?;
        }
        AliasAction::Set(opts) => {
            // Pass the entire options pack to the hybrid worker
            if let Err(e) = set_alias_hybrid(opts, &alias_path, quiet) {
                eprintln!("❌ Failed to set alias: {}", e);
            }
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
    // 1. Wipe everything first (using our new hybrid clear)
    let _ = clear_alias_hybrid(true);

    let definitions = parse_macro_file(path);
    let mut api_success_count = 0;

    // 2. Attempt API Injection
    for (name, value) in &definitions {
        if alias_win32::api_set_macro(name, Some(value)) {
            api_success_count += 1;
        }
    }

    // 3. Fallback: If the API missed any, let Doskey handle the whole file
    if api_success_count < definitions.len() {
        qprintln!(quiet, "⚠️ API missed {} macros. Running Doskey fallback...", definitions.len() - api_success_count);
        alias_wrapper::reload_doskey(path)?;
    } else {
        qprintln!(quiet, "✨ API Reload: {} macros synced.", definitions.len());
    }

    Ok(())
}

fn clear_alias_hybrid(quiet: bool) -> io::Result<()> {
    // 1. Primary Strike: Win32 API
    match alias_win32::purge_ram_macros() {
        Ok(report) => {
            if report.is_fully_clean() {
                qprintln!(quiet, "🧹 RAM cleared via API ({} macros removed).", report.cleared.len());
                return Ok(());
            } else {
                eprintln!("⚠️ API Partial Purge. Attempting Doskey fallback...");
            }
        },
        Err(_) => {
            eprintln!("⚠️ Win32 API unavailable. Falling back to Doskey...");
        }
    }

    // 2. Fallback Strike: Doskey Wrapper
    alias_wrapper::purge_ram_macros()?;
    qprintln!(quiet, "🧹 RAM cleared via Doskey Wrapper.");

    Ok(())
}

fn set_alias_hybrid(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()> {
    // 1. Prepare name/value based on flags
    let name = if opts.force_case { opts.name } else { opts.name.to_lowercase() };
    let value = opts.value.trim();
    let val_opt = if value.is_empty() { None } else { Some(value) };

    // 2. RAM Strike (Win32 API)
    let api_success = alias_win32::api_set_macro(&name, val_opt);

    // 3. RAM Fallback (Doskey)
    if !api_success {
        let _ = Command::new("doskey")
            .arg(format!("{}={}", name, value))
            .status();
    }

    // 4. Persistence Bypass (The "Volatile" Override)
    if opts.volatile {
        qprintln!(quiet, "⚡ Volatile alias (Session Only): {}", name);
        return Ok(()); // Stop here!
    }

    // 5. Disk Strike (Persistence)
    alias_lib::update_disk_file(&name, value, path)?;

    if value.is_empty() {
        qprintln!(quiet, "🗑️  Deleted: {}", name);
    } else {
        qprintln!(quiet, "✨ Set: {}={}", name, value);
    }

    Ok(())
}

// --- Minimal API Wrappers for the Hybrid ---

fn alias_show_everything() -> bool {
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


fn hybrid_show_all() {
    let w32 = alias_win32::get_all_aliases();
    let wrap = alias_wrapper::get_all_aliases();
    let file = alias_lib::dump_alias_file();

    perform_triple_audit(w32, wrap, file);
}
