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
    if std::env::var("PROFILE").unwrap_or_default() == "debug" {
        // Respect the Mutex used by the tests
        let _lock = stateful::GlobalNamedMutex::acquire();

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        // Only overwrite if no tests are currently active
        let current_count: u32 = hkcu.open_subkey(BACKUP_KEY_PATH)
            .and_then(|k| k.get_value("ActiveCount")).unwrap_or(0);

        if current_count == 0 {
            if let Ok((key, _)) = hkcu.create_subkey(BACKUP_KEY_PATH) {
                // 1. CAPTURE Golden Image (Vec<String> for MULTI_SZ)
                let aliases = raw_get_all_aliases_as_strings();
                let _ = key.set_value("Aliases", &aliases);

                // 2. CAPTURE AutoRun
                let run_path = r"Software\Microsoft\Command Processor";
                let autorun: String = hkcu.open_subkey(run_path)
                    .and_then(|k| k.get_value("AutoRun")).unwrap_or_default();
                let _ = key.set_value("AutoRun", &autorun);

                // 3. CLEAN ROOM setup
                stateful::clear_all_macros(); // Targeted delete
                if let Ok(k) = hkcu.open_subkey_with_flags(run_path, winreg::enums::KEY_SET_VALUE) {
                    let _ = k.set_value("AutoRun", &"");
                }

                // Initialized and ready for tests
                let _ = key.set_value("ActiveCount", &0u32);
            }
        }
    }
}

// Helper for build.rs to get strings exactly how winreg wants them
fn raw_get_all_aliases_as_strings() -> Vec<String> {
    let mut buffer = vec![0u16; 65536];
    unsafe {
        let res = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(), 131072, "cmd.exe\0".encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _
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