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
use std::time::Duration;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;
pub use alias_lib::{REG_SUBKEY, REG_AUTORUN_KEY};
#[allow(unused_imports)]
#[cfg(debug_assertions)]
use function_name::named;

fn get_test_silo_name() -> String {
    if env::var("ALIAS_TEST_BUCKET").is_ok() || cfg!(test) {
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
    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        // 1. NO MORE TRIMMING. Pass the name EXACTLY as it is.
        let n_wide: Vec<u16> = std::ffi::OsStr::new(name)
            .encode_wide().chain(Some(0)).collect();

        let v_wide: Option<Vec<u16>> = value.map(|v| {
            // 2. NO MORE TRIMMING. The quote in "%i" is vital!
            let encoded: Vec<u16> = std::ffi::OsStr::new(v)
                .encode_wide().chain(Some(0)).collect();
            encoded
        });

        unsafe {
            let exe_ptr = get_target_exe_wide();
            let success = AddConsoleAliasW(
                n_wide.as_ptr(),
                v_wide.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
                exe_ptr
            ) != 0;

            if !success {
                let code = GetLastError();
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Win32 Kernel rejected alias (Error Code: {})", code)
                ));
            }

            Ok(true)
        }
    }
    fn raw_reload_from_file(_verbosity: &Verbosity, path: &Path) -> io::Result<()> {
        // We pass Verbosity::silent() to satisfy the signature
        let macros = parse_macro_file(path, &Verbosity::silent())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        for (n, v) in macros {
            Self::raw_set_macro(&n, Some(&v))?;
        }
        Ok(())
    }
    fn get_all_aliases(_verbosity: &Verbosity) -> io::Result<Vec<(String, String)>> {
        let exe_name = get_target_exe_wide();

        unsafe {
            let mut len_bytes = GetConsoleAliasesLengthW(exe_name);

            if len_bytes == 0 {
                let code = GetLastError();
                // 203 = ERROR_ENVVAR_NOT_FOUND
                if code == 203 || code == 0 {
                    // let msg = text!(verbosity, AliasIcon::Alert, "no macros found in Win32 RAM.");
                    // return Ok(vec![(msg, String::new())]);
                    return Ok(Vec::new());
                }
                return Err(io::Error::new(io::ErrorKind::Other, format!("Kernel failed length query (Code {})", code)));
            }

            // Add padding to initial length to minimize re-allocations
            len_bytes += 512;

            let mut buffer;
            let mut read_bytes;

            loop {
                // Allocate u16 buffer (2 bytes per element)
                let buffer_size_u16 = (len_bytes as usize + 1) / 2; // Round up
                buffer = vec![0u16; buffer_size_u16];
                read_bytes = GetConsoleAliasesW(buffer.as_mut_ptr(), len_bytes, exe_name);

                if read_bytes > 0 {
                    break; // We have the data
                }

                let code = GetLastError();
                // 111 = Buffer Overflow, 122 = Insufficient Buffer
                if code == 111 || code == 122 {
                    len_bytes *= 2;
                    if len_bytes > 1024 * 1024 { // 1MB Safety cap
                        return Err(io::Error::new(io::ErrorKind::Other, "Win32 Alias buffer exceeded 1MB limit"));
                    }
                    continue;
                }

                return Err(io::Error::new(io::ErrorKind::Other, format!("Kernel failed buffer read (Code {})", code)));
            }

            // Slice only the bytes actually read
            let u16_len = (read_bytes as usize + 1) / 2;
            let wide_slice = &buffer[..u16_len];

            let list: Vec<(String, String)> = String::from_utf16_lossy(wide_slice)
                .split('\0')
                .filter_map(parse_alias_line)
                .collect();
            Ok(list)
        }
    }
    fn write_autorun_registry(cmd: &str, verbosity: &Verbosity) -> io::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(REG_SUBKEY)?;

        // 1. Get existing AutoRun string (e.g., "clink inject --profile ... & alias --startup")
        let raw_existing: String = key.get_value(REG_AUTORUN_KEY).unwrap_or_default();

        // 2. Identify ourselves by the file stem (e.g., "alias") to be rugged
        let my_stem = env::current_exe()?
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("alias")
            .to_lowercase();

        // 3. Parse existing commands into a vector
        let mut entries: Vec<String> = raw_existing
            .split('&')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut first_match_found = false;

        // 4. The "Highlander" Deduplication Logic:
        // We iterate through and keep only the first instance of 'alias',
        // replacing its content with the new command and dropping subsequent duplicates.
        entries.retain_mut(|entry| {
            let entry_lower = entry.to_lowercase();
            // Strip quotes for a clean comparison against the stem
            let clean_entry = entry_lower.replace('\"', "");

            // Check if this entry belongs to us (alias) and looks like a setup command
            if clean_entry.contains(&my_stem) && (clean_entry.contains("--startup") || clean_entry.contains("--file")) {
                if !first_match_found {
                    *entry = cmd.to_string(); // SWAP existing entry with current command
                    first_match_found = true;
                    true // Keep the first one we found (now updated)
                } else {
                    false // KILL THE GHOSTS (discard any secondary alias entries)
                }
            } else {
                true // Leave other tools (like Clink or prompt mods) alone
            }
        });

        // 5. If we weren't in the registry at all, append ourselves to the end
        if !first_match_found {
            entries.push(cmd.to_string());
        }

        // 6. Reconstruct the command string and commit to the Registry
        let final_val = entries.join(" & ");
        key.set_value(REG_AUTORUN_KEY, &final_val)?;

        shout!(verbosity, AliasIcon::Success, "AutoRun synchronized (Deduplicated & Position Preserved).");
        Ok(())
    }
    fn read_autorun_registry() -> String {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        // We open the key and get the value, defaulting to empty string on any error
        hkcu.open_subkey(REG_SUBKEY)
            .and_then(|key| key.get_value(REG_AUTORUN_KEY))
            .unwrap_or_default()
    }
    fn purge_ram_macros(verbosity: &Verbosity) -> io::Result<PurgeReport> {
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
    fn reload_full( verbosity: &Verbosity, path: &Path, clear: bool) -> Result<(), Box<dyn std::error::Error>> {
        if clear { Self::purge_ram_macros(verbosity)?; }
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
    fn query_alias(name: &str, verbosity: &Verbosity) -> Vec<String> {
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
    fn set_alias(opts: SetOptions, path: &Path, verbosity: &Verbosity) -> io::Result<()> {
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
    fn run_diagnostics(path: &Path, verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        let report = DiagnosticReport {
            binary_path: env::current_exe().ok(),
            resolved_path: path.to_path_buf(),
            env_file: env::var(ENV_ALIAS_FILE).unwrap_or_else(|_| "NOT SET".to_string()),
            env_opts: env::var(ENV_ALIAS_OPTS).unwrap_or_else(|_| "NOT SET".to_string()),
            file_exists: path.exists(),
            is_readonly: path.metadata().map(|m| m.permissions().readonly()).unwrap_or(false),
            drive_responsive: matches!(is_drive_responsive(path, IO_RESPONSIVENESS_THRESHOLD), AccessResult::Ready),
            registry_status: check_registry_native(),
            api_status: Some(if Self::is_api_responsive(IO_RESPONSIVENESS_THRESHOLD) { "CONNECTED (Win32 API)".to_string() } else { "FAILED".to_string() }),
        };
        render_diagnostics(report, verbosity);
        Ok(())
    }
    fn alias_show_all(verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        let os_pairs = Self::get_all_aliases(verbosity)?;
        perform_audit(os_pairs, verbosity, &Self::provider_type())
    }
    fn provider_type() -> ProviderType {
        ProviderType::Win32
    }
    fn is_api_responsive(timeout: Duration) -> bool {
        timeout_guard(timeout, || {
            let name = get_test_silo_name() + "\0";
            unsafe { GetConsoleAliasesLengthA(name.as_ptr()) };
            true // If it didn't hang, it's responsive
        }).unwrap_or(false)
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


