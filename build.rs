// V:\Projects\alias\build.rs

use std::process::Command;
use std::fs;
use std::path::Path;
use std::env;
use std::ptr;

#[path = "tests/state_restoration.rs"]
mod stateful;
#[path = "versioning.rs"]
mod versioning;

fn main() {
    // 2. Core Logic: Env Vars for UI
    main_vars();

    // 3. Core Logic: Registry Protection
    main_state();
}
fn main_vars() {
    let output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().ok();
    let git_hash = output
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Use a basic timestamp if chrono isn't available in build-dependencies,
    // but here we assume you have it as per your previous code.
    let build_date = if cfg!(windows) {
        let output = Command::new("powershell").args(["-Command", "Get-Date -Format 'yyyy-MM-dd HH:mm'"]).output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    };

    println!("cargo:rustc-env=BUILD_REVISION={}", git_hash);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rerun-if-changed=build.rs");
}

fn main_state() {
    if std::env::var("PROFILE").unwrap_or_default() == "debug" {
        let _lock = stateful::GlobalNamedMutex::acquire();

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let current_count: u32 = hkcu.open_subkey(BACKUP_KEY_PATH)
            .and_then(|k| k.get_value("ActiveCount")).unwrap_or(0);

        if current_count == 0 {
            if let Ok((key, _)) = hkcu.create_subkey(BACKUP_KEY_PATH) {
                let aliases = raw_get_all_aliases_as_strings();
                let _ = key.set_value("Aliases", &aliases);

                let run_path = r"Software\Microsoft\Command Processor";
                let autorun: String = hkcu.open_subkey(run_path)
                    .and_then(|k| k.get_value("AutoRun")).unwrap_or_default();
                let _ = key.set_value("AutoRun", &autorun);

                stateful::clear_all_macros();
                if let Ok(k) = hkcu.open_subkey_with_flags(run_path, winreg::enums::KEY_SET_VALUE) {
                    let _ = k.set_value("AutoRun", &"");
                }

                let _ = key.set_value("ActiveCount", &0u32);
            }
        }
    }
}

// --- WIN32 HELPERS ---

fn raw_get_all_aliases_as_strings() -> Vec<String> {
    let mut buffer = vec![0u16; 65536];
    unsafe {
        let res = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(),
            131072,
            "cmd.exe\0".encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _
        );
        if res == 0 { return vec![]; }
        let char_count = (res as usize + 1) / 2;
        String::from_utf16_lossy(&buffer[..char_count])
            .split('\0')
            .filter(|s| !s.is_empty() && s.contains('='))
            .map(|s| s.to_string())
            .collect()
    }
}

