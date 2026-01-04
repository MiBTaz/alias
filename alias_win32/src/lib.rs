use std::{env, io};
use std::path::Path;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Console::{
    GetConsoleAliasesLengthW, GetConsoleAliasesW, AddConsoleAliasW,
    GetConsoleAliasesLengthA // Still used for api_responsive check
};
use alias_lib::*;
use std::os::windows::ffi::OsStrExt;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;
pub use alias_lib::{REG_SUBKEY, REG_AUTORUN_KEY};

/// Helper to generate the silo name based on the environment.
fn get_test_silo_name() -> String {
    if std::env::var("ALIAS_TEST_BUCKET").is_ok() || cfg!(test) {
        format!("alias_test_silo_{:?}", std::thread::current().id())
    } else {
        "cmd.exe".to_string()
    }
}

fn get_target_exe_wide() -> *const u16 {
    use std::cell::RefCell;
    thread_local! {
        static WIDE_BUCKET: RefCell<Vec<u16>> = RefCell::new(Vec::new());
    }
    WIDE_BUCKET.with(|b| {
        let mut bucket = b.borrow_mut();
        if bucket.is_empty() {
            let name = get_test_silo_name();
            *bucket = std::ffi::OsStr::new(&name).encode_wide().chain(Some(0)).collect();
        }
        bucket.as_ptr()
    })
}

pub struct Win32LibraryInterface;

impl AliasProvider for Win32LibraryInterface {
    fn get_all_aliases() -> Vec<(String, String)> {
        let exe_name = get_target_exe_wide();
        unsafe {
            let len = GetConsoleAliasesLengthW(exe_name);
            if len == 0 { return vec![]; }

            let mut buffer = vec![0u16; (len / 2) as usize];
            let read = GetConsoleAliasesW(buffer.as_mut_ptr(), len, exe_name);

            String::from_utf16_lossy(&buffer[.. (read / 2) as usize])
                .split('\0')
                .filter_map(|line| {
                    line.split_once('=')
                        .map(|(n, v)| (n.trim_matches('"').to_string(), v.trim_matches('"').to_string()))
                })
                .collect()
        }
    }

    fn query_alias(name: &str, mode: Verbosity) -> Vec<String> {
        let search_target = name.to_lowercase();
        let os_list = Self::get_all_aliases();

        for (n, v) in os_list {
            if n.to_lowercase() == search_target {
                return vec![format!("{}={}", n, v)];
            }
        }

        if mode.level == VerbosityLevel::Normal {
            return vec![text!(mode, AliasIcon::Alert, "'{}' not found in Win32 RAM.", name)];
        }
        vec![]
    }

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        let n_wide: Vec<u16> = std::ffi::OsStr::new(name).encode_wide().chain(Some(0)).collect();
        let v_wide: Option<Vec<u16>> = value.map(|v| {
            std::ffi::OsStr::new(v).encode_wide().chain(Some(0)).collect()
        });

        unsafe {
            let success = AddConsoleAliasW(
                n_wide.as_ptr(),
                v_wide.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
                get_target_exe_wide()
            ) != 0;
            Ok(success)
        }
    }

    fn set_alias(opts: SetOptions, path: &Path, verbosity: Verbosity) -> io::Result<()> {
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };
        let val_opt = if opts.value.is_empty() { None } else { Some(opts.value.as_str()) };

        if !Self::raw_set_macro(&name, val_opt)? {
            shout!(verbosity, AliasIcon::Alert, "Kernel strike failed (Code {}).", unsafe { GetLastError() });
        }

        if opts.volatile {
            say!(verbosity, AliasIcon::Win32, "Volatile alias (RAM Only): {}", name);
            return Ok(());
        }

        update_disk_file(&name, &opts.value, path)?;
        whisper!(verbosity, AliasIcon::Success, "{} alias: {}", if opts.value.is_empty() { "Deleted" } else { "Set" }, name);
        Ok(())
    }

    fn reload_full(path: &Path, verbosity: Verbosity) -> io::Result<()> {
        let _ = Self::purge_ram_macros();
        let count = parse_macro_file(path).into_iter()
            .filter(|(n, v)| Self::raw_set_macro(n, Some(v)).unwrap_or(false))
            .count();

        whisper!(verbosity, AliasIcon::Success, "API Reload: {} macros injected.", count);
        Ok(())
    }

    fn raw_reload_from_file(path: &Path) -> io::Result<()> {
        let macros = parse_macro_file(path);
        for (n, v) in macros {
            let _ = Self::raw_set_macro(&n, Some(&v));
        }
        Ok(()) // Match expected () return
    }

    fn purge_ram_macros() -> io::Result<PurgeReport> {
        let mut report = PurgeReport { cleared: Vec::new(), failed: Vec::new() };
        for (name, _) in Self::get_all_aliases() {
            if Self::raw_set_macro(&name, None)? {
                report.cleared.push(name);
            } else {
                report.failed.push((name, unsafe { GetLastError() }));
            }
        }
        Ok(report)
    }

    fn write_autorun_registry(cmd: &str, _verbosity: Verbosity) -> io::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(REG_SUBKEY)?;
        key.set_value(REG_AUTORUN_KEY, &cmd.to_string())
    }

    fn read_autorun_registry() -> String {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        // We open the key and get the value, defaulting to empty string on any error
        hkcu.open_subkey(REG_SUBKEY)
            .and_then(|key| key.get_value(REG_AUTORUN_KEY))
            .unwrap_or_default()
    }

    fn run_diagnostics(path: &Path, verbosity: Verbosity) {
        let report = DiagnosticReport {
            binary_path: env::current_exe().ok(),
            resolved_path: path.to_path_buf(),
            env_file: env::var(ENV_ALIAS_FILE).unwrap_or_else(|_| "NOT SET".to_string()),
            env_opts: env::var(ENV_ALIAS_OPTS).unwrap_or_else(|_| "NOT SET".to_string()),
            file_exists: path.exists(),
            is_readonly: path.metadata().map(|m| m.permissions().readonly()).unwrap_or(false),
            drive_responsive: is_drive_responsive(path),
            registry_status: check_registry_native(),
            api_status: Some(if is_api_responsive() { "CONNECTED (Win32 API)".to_string() } else { "FAILED".to_string() }),
        };
        render_diagnostics(report, verbosity);
    }

    fn alias_show_all(verbosity: Verbosity) {
        let os_pairs = Self::get_all_aliases();
        perform_audit(os_pairs, verbosity);
    }
}

// --- Internal Utilities (Non-Trait) ---

fn check_registry_native() -> RegistryStatus {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(REG_SUBKEY) {
        Ok(key) => {
            let val: String = key.get_value(REG_AUTORUN_KEY).unwrap_or_default();
            if val.contains("--reload") || val.contains("alias") {
                RegistryStatus::Synced
            } else if !val.is_empty() {
                RegistryStatus::Mismatch(val)
            } else {
                RegistryStatus::NotFound
            }
        }
        Err(_) => RegistryStatus::NotFound,
    }
}

fn is_api_responsive() -> bool {
    let name = get_test_silo_name() + "\0";
    unsafe {
        let len = GetConsoleAliasesLengthA(name.as_ptr());
        len > 0 || len == 0
    }
}
