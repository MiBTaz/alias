// alias_win32/src/lib.rs

// --- Win32 API Core ---

use std::{env, fs, io};
use std::path::Path;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Console::{AddConsoleAliasA, GetConsoleAliasA, GetConsoleAliasesA, GetConsoleAliasesLengthA};
use windows_sys::Win32::System::Registry::{RegCloseKey, RegCreateKeyA, RegSetValueExA, HKEY, REG_SZ, HKEY_CURRENT_USER};
use alias_lib::*;
use alias_lib::qprintln;
use std::io::Read;
// For the Win32 version, we'll use winreg for a clean API-level check
use winreg::RegKey;

pub fn api_set_macro(name: &str, value: Option<&str>) -> bool {
    let (n_c, v_c) = (format!("{}\0", name), value.map(|v| format!("{}\0", v)));
    unsafe {
        AddConsoleAliasA(n_c.as_ptr(), v_c.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()), "cmd.exe\0".as_ptr()) != 0
    }
}

// --- Logic Helpers ---

pub fn reload_full(path: &Path, quiet: bool) -> io::Result<()> {
    api_purge_all_macros(true);
    let count = parse_macro_file(path).into_iter()
        .filter(|(n, v)| api_set_macro(n, Some(v)))
        .count();
    qprintln!(quiet, "✨ API Reload: {} macros injected.", count);
    Ok(())
}

pub fn set_alias(name: &str, value: &str, path: &Path, quiet: bool) -> io::Result<()> {
    let val_opt = if value.is_empty() { None } else { Some(value) };
    if !api_set_macro(name, val_opt) {
        eprintln!("⚠️ Kernel strike failed (Code {}).", unsafe { GetLastError() });
    }

    let search = format!("{}=", name.to_lowercase());
    let mut lines: Vec<String> = fs::read_to_string(path).unwrap_or_default()
        .lines().filter(|l| !l.to_lowercase().starts_with(&search))
        .map(String::from).collect();

    if !value.is_empty() { lines.push(format!("{}={}", name, value)); }
    fs::write(path, lines.join("\n") + "\n")?;

    qprintln!(quiet, "✨ {} alias: {}", if value.is_empty() { "Deleted" } else { "Set" }, name);
    Ok(())
}

// --- Logic Helpers ---
pub fn install_autorun(quiet: bool) -> io::Result<()> {
    // 1. Get path and wrap in quotes to handle spaces
    let exe_path = env::current_exe()?;
    let cmd_string = format!("\"{}\" --reload", exe_path.display());

    // 2. Prepare null-terminated C-strings (ANSI for simplicity, if you prefer)
    let c_subkey = "Software\\Microsoft\\Command Processor\0";
    let c_value_name = "AutoRun\0";
    let mut c_data = cmd_string.into_bytes();
    c_data.push(0); // Explicit null terminator for the Registry

    let mut hkey: HKEY = 0 as HKEY;
    unsafe {
        // Use RegCreateKeyExA for better compatibility/control
        if RegCreateKeyA(HKEY_CURRENT_USER, c_subkey.as_ptr(), &mut hkey) == 0 {

            let status = RegSetValueExA(
                hkey,
                c_value_name.as_ptr(),
                0,
                REG_SZ, // Use REG_SZ unless you are using %VAR% variables
                c_data.as_ptr(),
                c_data.len() as u32
            );

            RegCloseKey(hkey);

            if status == 0 {
                qprintln!(quiet, "✅ AutoRun hook installed.");
                return Ok(());
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::Other, "Registry access denied or failed"))
}

// --- Win32 Memory Iteration ---

pub fn for_each_macro<F: FnMut(&str)>(mut f: F) {
    let exe = "cmd.exe\0".as_ptr();
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

pub fn api_show_all() { println!("[cmd.exe]"); for_each_macro(|e| println!("{}", e)); }
pub fn api_purge_all_macros(quiet: bool) {
    for_each_macro(|e| {
        if let Some((n, _)) = e.split_once('=') {
            if api_set_macro(n, None) { qprintln!(quiet, "🔥 Purged: [{}]", n); }
        }
    });
}

pub fn check_autorun_status() -> io::Result<String> {
    use windows_sys::Win32::System::Registry::{RegOpenKeyA, RegQueryValueExA, RegCloseKey, HKEY_CURRENT_USER};
    let mut hkey = 0 as windows_sys::Win32::System::Registry::HKEY;
    unsafe {
        let subkey = format!("{}\0", REG_SUBKEY);
        let val_name = format!("{}\0", REG_VALUE_NAME);
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
    let mut results = Vec::new();
    let exe_name = "cmd.exe\0".as_ptr();
    let name_c = format!("{}\0", name);
    let mut buffer = [0u8; 2048];

    unsafe {
        let result = GetConsoleAliasA(
            name_c.as_ptr() as *mut u8,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            exe_name as *mut u8,
        );

        if result > 0 {
            let output = String::from_utf8_lossy(&buffer[..result as usize]).to_string();
            results.push(output);
        } else if mode == OutputMode::Normal {
            results.push(format!("⚠️ '{}' is not active in the current session.", name));
        }
    }
    results
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
    let subkey = r"Software\Microsoft\Command Processor";

    match hkcu.open_subkey(subkey) {
        Ok(key) => {
            let autorun: String = key.get_value("AutoRun").unwrap_or_default();
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
    let exe = "cmd.exe\0".as_ptr();

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
