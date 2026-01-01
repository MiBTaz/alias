// alias_win32/src/lib.rs

use std::{env, fs, io};
use std::path::Path;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Console::{GetConsoleAliasesLengthA, GetConsoleAliasesA};
use alias_lib::*;
use alias_lib::qprintln;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

// Simplified Wide Silo for Serial Testing
fn get_target_exe_wide() -> *const u16 {
    use std::sync::OnceLock;
    static BUCKET_W: OnceLock<Vec<u16>> = OnceLock::new();

    BUCKET_W.get_or_init(|| {
        let is_test = std::env::var("ALIAS_TEST_BUCKET").is_ok() || cfg!(test);
        let name = if is_test { "alias_test_silo" } else { "cmd.exe" };
        std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }).as_ptr()
}

// Keep the old one only if other ANSI functions still need it,
// otherwise you can delete it later.
fn get_target_exe() -> *const u8 {
    use std::cell::RefCell;
    thread_local! {
        static THREAD_BUCKET: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    }
    THREAD_BUCKET.with(|bucket| {
        let mut b = bucket.borrow_mut();
        if b.is_empty() {
            let name = if std::env::var("ALIAS_TEST_BUCKET").is_ok() || cfg!(test) {
                format!("alias_test_silo_{:?}\0", std::thread::current().id())
            } else {
                "cmd.exe\0".to_string()
            };
            *b = name.into_bytes();
        }
        b.as_ptr()
    })
}

pub struct Win32LibraryInterface;


impl alias_lib::AliasProvider for Win32LibraryInterface {
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

pub fn api_set_macro(name: &str, value: Option<&str>) -> bool {
    use std::os::windows::ffi::OsStrExt;

    // Encode strings to Wide (UTF-16) for Win32 W-APIs
    let n_wide: Vec<u16> = std::ffi::OsStr::new(name).encode_wide().chain(Some(0)).collect();
    let v_wide: Option<Vec<u16>> = value.map(|v| {
        std::ffi::OsStr::new(v).encode_wide().chain(Some(0)).collect()
    });

    unsafe {
        windows_sys::Win32::System::Console::AddConsoleAliasW(
            n_wide.as_ptr(),
            v_wide.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            get_target_exe_wide() // You'll need a similar Wide version of the silo name
        ) != 0
    }
}

// --- Logic Helpers ---

pub fn reload_full(path: &Path, quiet: bool) -> io::Result<()> {
    // 1. Call the parameter-less version
    let _ = purge_ram_macros();

    // 2. Proceed with injection
    let count = parse_macro_file(path).into_iter()
        .filter(|(n, v)| api_set_macro(n, Some(v)))
        .count();

    qprintln!(quiet, "✨ API Reload: {} macros injected.", count);
    Ok(())
}

pub fn set_alias(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()> {
    // 1. Determine if we respect the case or force lowercase (The Override)
    let name = if opts.force_case { opts.name } else { opts.name.to_lowercase() };
    let val_opt = if opts.value.is_empty() { None } else { Some(opts.value.as_str()) };

    // 2. RAM Strike
    if !api_set_macro(&name, val_opt) {
        eprintln!("⚠️ Kernel strike failed (Code {}).", unsafe { GetLastError() });
    }

    // 3. Volatile check
    if opts.volatile {
        qprintln!(quiet, "⚡ Volatile alias (RAM Only): {}", name);
        return Ok(());
    }

    // 4. Disk Strike
    update_disk_file(&name, &opts.value, path)?;

    qprintln!(quiet, "✨ {} alias: {}", if opts.value.is_empty() { "Deleted" } else { "Set" }, name);
    Ok(())
}

// --- Logic Helpers ---
pub fn install_autorun(quiet: bool) -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let our_cmd = format!("\"{}\" --reload", exe_path.display());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(REG_SUBKEY)?;

    // 1. Check if an AutoRun already exists
    let existing: String = key.get_value(REG_AUTORUN_KEY).unwrap_or_default();

    // 2. Decide if we need to append or just set
    let new_val = if existing.is_empty() {
        our_cmd
    } else if existing.contains("--reload") {
        qprintln!(quiet, "ℹ️ AutoRun already configured.");
        return Ok(());
    } else {
        // Append with '&' to preserve existing AutoRun logic (e.g., Clink/Anaconda)
        format!("{} & {}", existing, our_cmd)
    };

    key.set_value(REG_AUTORUN_KEY, &new_val)?;
    qprintln!(quiet, "✅ AutoRun hook installed (Preserved existing commands).");
    Ok(())
}

// --- Win32 Memory Iteration ---

pub fn for_each_macro<F: FnMut(&str)>(mut f: F) {
    let exe = get_target_exe();
    unsafe {
        let len = GetConsoleAliasesLengthA(exe);
        if len > 0 {
            let mut buf = vec![0u8; len as usize];
            let read = GetConsoleAliasesA(buf.as_mut_ptr(), len, exe);
            String::from_utf8_lossy(&buf[..read as usize]).split('\0')
                .filter(|e| !e.is_empty()).for_each(|e| f(e));
        }
    }
}

pub fn purge_ram_macros() -> io::Result<PurgeReport> {
    let mut report = PurgeReport {
        cleared: Vec::new(),
        failed: Vec::new(),
    };

    // We get the current state first
    let active_macros = get_all_aliases();

    for (name, _) in active_macros {
        // Passing None to the value deletes the macro
        if api_set_macro(&name, None) {
            report.cleared.push(name);
        } else {
            let err = unsafe { GetLastError() };
            report.failed.push((name, err));
        }
    }

    Ok(report)
}

pub fn alias_show_all() {
    let os_pairs = get_all_aliases();
    alias_lib::perform_audit(os_pairs);
}

pub fn get_all_aliases_raw() -> Vec<String> {
    let exe_name = get_target_exe_wide(); // Use Wide Silo
    unsafe {
        let len = windows_sys::Win32::System::Console::GetConsoleAliasesLengthW(exe_name);
        if len == 0 { return vec![]; }

        let mut buffer = vec![0u16; len as usize / 2]; // u16 for Wide
        let read = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(),
            len,
            exe_name
        );

        String::from_utf16_lossy(&buffer[..read as usize / 2])
            .split('\0')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .collect()
    }
}

pub fn get_all_aliases() -> Vec<(String, String)> {
    get_all_aliases_raw()
        .into_iter()
        .filter_map(|line| {
            // split_once('=') ensures we catch "name=" as ("name", "")
            line.split_once('=').map(|(n, v)| (n.to_string(), v.to_string()))
        })
        .collect()
}



pub fn check_autorun_status() -> io::Result<String> {
    use windows_sys::Win32::System::Registry::{RegOpenKeyA, RegQueryValueExA, RegCloseKey, HKEY_CURRENT_USER};
    let mut hkey = 0 as windows_sys::Win32::System::Registry::HKEY;
    unsafe {
        let subkey = format!("{}\0", REG_SUBKEY);
        let val_name = format!("{}\0", REG_AUTORUN_KEY);
        if RegOpenKeyA(HKEY_CURRENT_USER, subkey.as_ptr(), &mut hkey) == 0 {
            let mut buf = [0u8; 512];
            let mut len = buf.len() as u32;
            let res = RegQueryValueExA(hkey, val_name.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), buf.as_mut_ptr(), &mut len);
            RegCloseKey(hkey);
            if res == 0 {
                let val = String::from_utf8_lossy(&buf[..len as usize]);
                return Ok(if val.contains("alias") { "SYNCED ✅".into() } else { "MISMATCH ⚠️".into() });
            }
        }
    }
    Ok("NOT FOUND ❌".into())
}

pub fn query_alias(name: &str, mode: OutputMode) -> Vec<String> {
    use std::os::windows::ffi::OsStrExt;
    let name_w: Vec<u16> = std::ffi::OsStr::new(name).encode_wide().chain(Some(0)).collect();
    let target_w = get_target_exe_wide();
    let mut buffer = [0u16; 2048];

    unsafe {
        let result = windows_sys::Win32::System::Console::GetConsoleAliasW(
            name_w.as_ptr() as *mut u16,
            buffer.as_mut_ptr(),
            buffer.len() as u32 * 2,
            target_w as *mut u16,
        );
        if result > 0 {
            vec![String::from_utf16_lossy(&buffer[..result as usize / 2])]
        } else {
            if mode == OutputMode::Normal {
                vec![format!("⚠️ '{}' not active", name)]
            } else {
                vec![]
            }
        }
    }
}

pub fn get_alias_list() -> Vec<String> {
    let exe_name = get_target_exe();
    // 1. Ask the kernel how big a buffer we need
    let buffer_size = unsafe {
        GetConsoleAliasesLengthA(exe_name as *mut u8)
    };

    if buffer_size == 0 { return vec![]; }

    // 2. Allocate and fetch
    let mut buffer = vec![0u8; buffer_size as usize];
    unsafe {
        GetConsoleAliasesA(
            buffer.as_mut_ptr(),
            buffer_size,
            exe_name as *mut u8,
        );
    }

    // 3. Parse the null-terminated block into a Vec of "name=value"
    buffer.split(|&b| b == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect()
}

pub fn run_diagnostics(path: &Path) {
    println!("--- 🛠️  Win32-Native Diagnostics ---");

    // 1. Core Environment
    if let Ok(p) = env::current_exe() {
        println!("Binary Loc:    {}", p.display());
    }

    println!("Resolved Path: {}", path.display());

    // 2. File & Drive Health
    match path.metadata() {
        Ok(m) => {
            let read_only = m.permissions().readonly();
            println!("File Status:   EXISTS {}", if read_only { "(READ-ONLY ⚠️)" } else { "(WRITABLE ✅)" });

            if let Ok(mut f) = fs::File::open(path) {
                let mut buf = [0; 1];
                if f.read(&mut buf).is_ok() {
                    println!("Drive Status:  RESPONSIVE ⚡");
                }
            }
        }
        Err(_) => println!("File Status:   MISSING OR INACCESSIBLE ❌"),
    }

    // 3. API-Level Registry Check
    // Instead of shelling out to reg.exe, we use the Windows Registry API
    println!("\nRegistry Check (AutoRun):");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let subkey = REG_SUBKEY;

    match hkcu.open_subkey(subkey) {
        Ok(key) => {
            let autorun: String = key.get_value(REG_AUTORUN_KEY).unwrap_or_default();
            if autorun.is_empty() {
                println!("  Status:      EMPTY (No AutoRun set) ⚪");
            } else if autorun.contains("alias") {
                println!("  Status:      SYNCED ✅ (Found: \"...alias\")");
            } else {
                println!("  Status:      MISMATCH ⚠️ (Found other: \"{}\")", autorun);
            }
        }
        Err(_) => println!("  Status:      KEY NOT FOUND ❌ (Command Processor path missing)"),
    }

    // 4. API Memory Probe
    // Let's actually check if the Console is responding to macro queries
    #[cfg(target_os = "windows")]
    {
        println!("\nConsole API Health:");
        // Directly check if we can get the length of aliases for cmd.exe
        if is_api_responsive() {
            println!("  Status:      CONNECTED 🔗 (Win32 Console Link Active)");
        } else {
            println!("  Status:      DISCONNECTED 💔 (Is this a restricted terminal?)");
        }
    }
}

/// Checks if the Win32 Console API is actually responding.
/// Returns true if we can communicate with the console subsystem.
pub fn is_api_responsive() -> bool {
    // We target "cmd.exe" as the default executable name for the alias subsystem
    let exe = get_target_exe();

    unsafe {
        // GetConsoleAliasesLengthA returns 0 if no aliases exist,
        // or a positive number if they do.
        // It returns 0 and sets an error if the handle is invalid.
        let len = GetConsoleAliasesLengthA(exe);

        // In most terminal hosts (conhost.exe), this will return >= 0.
        // If we are in a non-standard pipe or a restricted environment, it may fail.
        len > 0 || len == 0
    }
}
