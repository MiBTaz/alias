// tests/state_restoration.rs
use windows_sys::Win32::System::Threading::{
    CreateMutexW,
    WaitForSingleObject,
    ReleaseMutex,
    INFINITE,
};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_ABANDONED, CloseHandle, HANDLE};
use winreg::enums::*;
use winreg::RegKey;
use std::ptr;

const BACKUP_KEY_PATH: &str = r"Software\AliasTool\Backup";

pub struct GlobalNamedMutex {
    handle: HANDLE,
}

impl GlobalNamedMutex {
    pub fn acquire() -> Self {
        let name: Vec<u16> = "Global\\AliasToolTestLock\0".encode_utf16().collect();
        unsafe {
            let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
            if handle == 0 as _ { panic!("Failed to create Global Mutex"); }

            match WaitForSingleObject(handle, INFINITE) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Self { handle },
                _ => {
                    CloseHandle(handle);
                    panic!("Failed to acquire Global Mutex");
                }
            }
        }
    }
}

impl Drop for GlobalNamedMutex {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub fn pre_flight_inc() {
    let _lock = GlobalNamedMutex::acquire();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(BACKUP_KEY_PATH).unwrap();

    let count: u32 = key.get_value("ActiveCount").unwrap_or(0);

    if count == 0 {
        // 1. Capture AutoRun
        let cp_path = r"Software\Microsoft\Command Processor";
        if let Ok(cp_key) = hkcu.open_subkey(cp_path) {
            let current_autorun: String = cp_key.get_value("AutoRun").unwrap_or_default();
            let _ = key.set_value("AutoRun", &current_autorun);
        }

        // 2. Capture RAM Aliases
        let mut buffer = vec![0u16; 65536];
        let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
        unsafe {
            let result = windows_sys::Win32::System::Console::GetConsoleAliasesW(
                buffer.as_mut_ptr(),
                (buffer.len() * 2) as u32,
                exe_name.as_ptr() as *mut u16,
            );
            if result > 0 {
                let raw_string = String::from_utf16_lossy(&buffer[..(result as usize / 2)]);
                let alias_list: Vec<String> = raw_string
                    .split('\0')
                    .filter(|s| !s.is_empty() && s.contains('='))
                    .map(|s| s.to_string())
                    .collect();

                if !alias_list.is_empty() {
                    let _ = key.set_value("Aliases", &alias_list);
                }
            }
        }
    }
    let _ = key.set_value("ActiveCount", &(count + 1));
}

pub fn post_flight_dec() {
    let _lock = GlobalNamedMutex::acquire();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    if let Ok(key) = hkcu.open_subkey_with_flags(BACKUP_KEY_PATH, KEY_ALL_ACCESS) {
        let count: u32 = key.get_value("ActiveCount").unwrap_or(0);

        if count <= 1 {
            // RELOAD: Uses MULTI_SZ strings ("key=value")
            if let Ok(backup_aliases) = key.get_value::<Vec<String>, _>("Aliases") {
                clear_all_macros(); // Targeted scrub

                for entry in backup_aliases {
                    if let Some((k, v)) = entry.split_once('=') {
                        raw_add_alias(k, v);
                    }
                }
            }

            // RESTORE AutoRun
            if let Ok(old_autorun) = key.get_value::<String, _>("AutoRun") {
                if let Ok(rk) = hkcu.open_subkey_with_flags(r"Software\Microsoft\Command Processor", KEY_SET_VALUE) {
                    let _ = rk.set_value("AutoRun", &old_autorun);
                }
            }

            // Anchor remains, count goes to zero
            let _ = key.set_value("ActiveCount", &0u32);
        } else {
            let _ = key.set_value("ActiveCount", &(count - 1));
        }
    }
}
fn clear_all_macros() {
    let mut buffer = vec![0u16; 65536];
    let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
    unsafe {
        let result = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(),
            131072,
            exe_name.as_ptr() as *mut u16,
        );
        if result > 0 {
            let s = String::from_utf16_lossy(&buffer[..((result as usize + 1) / 2)]);
            for pair in s.split('\0').filter(|x| !x.is_empty() && x.contains('=')) {
                if let Some((k, _)) = pair.split_once('=') {
                    // In Win32, AddConsoleAlias with NULL value deletes the alias
                    windows_sys::Win32::System::Console::AddConsoleAliasW(
                        format!("{}\0", k).encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _,
                        ptr::null_mut(), // THIS CLEARS IT
                        exe_name.as_ptr() as *mut _
                    );
                }
            }
        }
    }
}

fn raw_add_alias(name: &str, value: &str) {
    let name_w: Vec<u16> = format!("{}\0", name).encode_utf16().collect();
    let value_w: Vec<u16> = format!("{}\0", value).encode_utf16().collect();
    let exe_w: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
    unsafe {
        windows_sys::Win32::System::Console::AddConsoleAliasW(
            name_w.as_ptr() as *mut _,
            value_w.as_ptr() as *mut _,
            exe_w.as_ptr() as *mut _
        );
    }
}

pub fn has_backup() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\AliasTool\Backup")
        .is_ok()
}

pub fn is_stale() -> bool {
    false
}