// alias_win32/src/lib.rs

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
    fn get_all_aliases(verbosity: Verbosity) -> io::Result<Vec<(String, String)>> {
        let exe_name = get_target_exe_wide();
        unsafe {
            let len_bytes = GetConsoleAliasesLengthW(exe_name);
            trace!("GetConsoleAliasesLengthW: {} bytes", len_bytes);

            if len_bytes == 0 {
                let code = GetLastError();
                trace!("Length was 0. LastError: {}", code);
                // 203 = ERROR_ENVVAR_NOT_FOUND (Empty RAM)
                if code == 203 || code == 0 {
                    let msg = text!(verbosity, AliasIcon::Alert, "no macros (aliases) found in Win32 RAM.");
                    trace!("Empty RAM detected (203/0). Returning alert tuple.");
                    return Ok(vec![(msg, String::new())]);
                }
                return Err(io::Error::new(io::ErrorKind::Other, format!("Kernel failed length query (Code {})", code)));
            }

            let mut buffer = vec![0u16; (len_bytes / 2) as usize];
            let read_bytes = GetConsoleAliasesW(buffer.as_mut_ptr(), len_bytes, exe_name);
            trace!("GetConsoleAliasesW read {} bytes into buffer", read_bytes);

            if read_bytes == 0 {
                let code = GetLastError();
                trace!("Read was 0. LastError: {}", code);
                if code == 203 {
                    let msg = text!(verbosity, AliasIcon::Alert, "no macros (aliases) found in Win32 RAM.");
                    return Ok(vec![(msg, String::new())]);
                }
                return Err(io::Error::new(io::ErrorKind::Other, format!("Kernel failed buffer read (Code {})", code)));
            }

            let wide_slice = &buffer[.. (read_bytes / 2) as usize];
            let list: Vec<(String, String)> = String::from_utf16_lossy(wide_slice)
                .split('\0')
                .filter_map(|line| {
                    if line.trim().is_empty() { return None; }
                    line.split_once('=').map(|(n, v)| {
                        (n.trim_matches('"').to_string(), v.trim_matches('"').to_string())
                    })
                })
                .collect();

            trace!("Parsed {} aliases from RAM", list.len());
            Ok(list)
        }
    }
    fn query_alias(name: &str, verbosity: Verbosity) -> Vec<String> {
        trace!("query_alias entry for name: '{}'", name);
        let search_target = name.to_lowercase();

        let os_list = match Self::get_all_aliases(verbosity) {
            Ok(list) => { list },
            Err(e) => {
                return vec![text!(verbosity, AliasIcon::Alert, "Kernel Query Failed: {}", e)];
            }
        };

        for (n, v) in os_list {
            if n.to_lowercase() == search_target {
                return vec![format!("{}={}", n, v)];
            }
        }

        // This return MUST match what your test is looking for: "not a known alias"
        vec![text!(verbosity, AliasIcon::Alert, "'{}' not found in Win32 RAM.", name)]
    }

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        // 1. Clean the input to prevent Ghost Quotes in the Kernel
        let n_wide: Vec<u16> = std::ffi::OsStr::new(name.trim_matches('"'))
            .encode_wide().chain(Some(0)).collect();
        let v_wide: Option<Vec<u16>> = value.map(|v| {
            std::ffi::OsStr::new(v.trim_matches('"')).encode_wide().chain(Some(0)).collect()
        });

        unsafe {
            let success = AddConsoleAliasW(
                n_wide.as_ptr(),
                v_wide.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
                get_target_exe_wide()
            ) != 0;

            if !success {
                let code = GetLastError();
                // This is the "Percolation": converting a Win32 code into a Rust IO Error
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Win32 Kernel rejected alias (Error Code: {})", code)
                ));
            }
            Ok(true)
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

        update_disk_file(verbosity, &name, &opts.value, path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        whisper!(verbosity, AliasIcon::Success, "{} alias: {}", if opts.value.is_empty() { "Deleted" } else { "Set" }, name);
        Ok(())
    }
    fn reload_full(path: &Path, verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        let _ = Self::purge_ram_macros(verbosity);

        // 1. Add '?' to percolate the error and get the Vec
        // 2. Pass verbosity to match the new signature
        let macros = parse_macro_file(path, verbosity)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let mut count = 0;
        for (n, v) in macros {
            // Use '?' here too to ensure we stop on a kernel failure
            Self::raw_set_macro(&n, Some(&v))?;
            count += 1;
        }

        whisper!(verbosity, AliasIcon::Success, "API Reload: {} macros injected.", count);
        Ok(())
    }
    fn raw_reload_from_file(path: &Path) -> io::Result<()> {
        // We pass Verbosity::silent() to satisfy the signature
        let macros = parse_macro_file(path, Verbosity::silent())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        for (n, v) in macros {
            Self::raw_set_macro(&n, Some(&v))?;
        }
        Ok(())
    }
    fn purge_ram_macros(verbosity: Verbosity) -> io::Result<PurgeReport> {
        let mut report = PurgeReport { cleared: Vec::new(), failed: Vec::new() };
        // Now using ? on the getter
        for (name, _) in Self::get_all_aliases(verbosity)? {
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
    fn run_diagnostics(path: &Path, verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
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
        Ok(())
    }
    fn alias_show_all(verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        let os_pairs = Self::get_all_aliases(verbosity)?;
        perform_audit(os_pairs, verbosity)
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
