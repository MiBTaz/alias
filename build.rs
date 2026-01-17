// build.rs
use std::process::Command;

#[path = "tests/state_restoration.rs"]
mod stateful;

fn main() {
    // Trigger re-run only if files change
    // println!("cargo:rerun-if-changed=.git/HEAD");
    main_vars();
    main_state();
}
fn main_vars() {
    let output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().ok();
    let git_hash = output
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    println!("cargo:rustc-env=BUILD_REVISION={}", git_hash);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rerun-if-changed=build.rs");
}
fn main_state() {
    // Only manage system state during debug/test builds
    if std::env::var("PROFILE").unwrap_or_default() == "debug" {
        let _lock = stateful::GlobalNamedMutex::acquire();

        // 1. Cleanup Stale Backups
        if stateful::has_backup() && stateful::is_stale() {
            println!("cargo:warning=⚠️ Stale backup detected. Restoring and cleaning...");
            // You would call your restore logic here
            let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let _ = hkcu.delete_subkey_all(r"Software\AliasTool\Backup");
        }

        // 2. Perform Initial Backup
        if !stateful::has_backup() {
            println!("cargo:warning=🔒 Creating System Backup for Test Run...");

            let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            if let Ok((key, _)) = hkcu.create_subkey(r"Software\AliasTool\Backup") {
                // A. Backup AutoRun (Raw winreg)
                let run_key = hkcu.open_subkey(r"Software\Microsoft\Command Processor").ok();
                let autorun: String = run_key.and_then(|k| k.get_value("AutoRun").ok()).unwrap_or_default();
                let _ = key.set_value("AutoRun", &autorun);

                // B. Clear System AutoRun so tests don't trigger it
                if let Ok(k) = hkcu.open_subkey_with_flags(r"Software\Microsoft\Command Processor", winreg::enums::KEY_SET_VALUE) {
                    let _ = k.set_value("AutoRun", &"");
                }

                // C. Backup RAM Aliases (Using our raw helper)
                let aliases = stateful::raw_get_all_aliases();
                let _ = stateful::write_multi_sz(r"Software\AliasTool\Backup", "Aliases", aliases);

                // D. Nuke RAM Aliases for a clean test environment
                stateful::raw_nuke_aliases();

                // E. Initialize Semaphore
                let _ = key.set_value("ActiveCount", &0u32);
            }
        }
    }
}

pub fn raw_get_all_aliases() -> Vec<(String, String)> {
    // Calling the Win32 API directly to get current RAM aliases
    // This is a simplified version of what's in your win32_lib
    let mut buffer = [0u16; 32768];
    let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();

    unsafe {
        let len = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(),
            buffer.len() as u32 * 2,
            exe_name.as_ptr() as *mut _
        );
        if len == 0 { return vec![]; }

        String::from_utf16_lossy(&buffer[..len as usize / 2])
            .split('\0')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

pub fn raw_nuke_aliases() {
    let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
    unsafe {
        // Passing null as the source buffer nukes all aliases for that target
        windows_sys::Win32::System::Console::AddConsoleAliasW(
            ptr::null_mut(),
            ptr::null_mut(),
            exe_name.as_ptr() as *mut _
        );
    }
}