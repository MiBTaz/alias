// libs/stateful.rs
use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject, ReleaseMutex, INFINITE};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_ABANDONED, CloseHandle, HANDLE};
use winreg::enums::*;
use winreg::RegKey;
use std::ptr;

const BACKUP_KEY_PATH: &str = r"Software\AliasTool\Backup";

pub struct GlobalNamedMutex { handle: HANDLE }

impl GlobalNamedMutex {
    pub fn acquire() -> Self {
        let name: Vec<u16> = "Global\\AliasToolTestLock\0".encode_utf16().collect();
        unsafe {
            let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
            match WaitForSingleObject(handle, INFINITE) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Self { handle },
                _ => { CloseHandle(handle); panic!("Mutex fail"); }
            }
        }
    }
}
impl Drop for GlobalNamedMutex {
    fn drop(&mut self) { unsafe { ReleaseMutex(self.handle); CloseHandle(self.handle); } }
}

// --- STUBS FOR COMPILER (The parts you asked why were missing) ---
pub fn has_backup() -> bool {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(BACKUP_KEY_PATH).is_ok()
}

pub fn is_stale() -> bool { false }

// --- THE REBUILT JOY LOGIC ---
pub fn pre_flight_inc() {
    let _lock = GlobalNamedMutex::acquire();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(BACKUP_KEY_PATH).unwrap();
    let count: u32 = key.get_value("ActiveCount").unwrap_or(0);

    if count == 0 {
        // Capture AutoRun
        if let Ok(cp_key) = hkcu.open_subkey(r"Software\Microsoft\Command Processor") {
            let current: String = cp_key.get_value("AutoRun").unwrap_or_default();
            let _ = key.set_value("AutoRun", &current);
        }
        // Capture RAM
        let mut buffer = vec![0u16; 65536];
        unsafe {
            let res = windows_sys::Win32::System::Console::GetConsoleAliasesW(
                buffer.as_mut_ptr(), 131072, "cmd.exe\0".encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _
            );
            if res > 0 {
                let s = String::from_utf16_lossy(&buffer[..(res as usize / 2)]);
                let list: Vec<String> = s.split('\0').filter(|x| !x.is_empty() && x.contains('=')).map(|x| x.to_string()).collect();
                let _ = key.set_value("Aliases", &list);
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
            // Restore Global State
            if let Ok(old) = key.get_value::<String, _>("AutoRun") {
                if let Ok(rk) = hkcu.open_subkey_with_flags(r"Software\Microsoft\Command Processor", KEY_SET_VALUE) {
                    let _ = rk.set_value("AutoRun", &old);
                }
            }
            // Restore RAM
            if let Ok(aliases) = key.get_value::<Vec<String>, _>("Aliases") {
                for pair in aliases {
                    if let Some((k, v)) = pair.split_once('=') { raw_add_alias(k, v); }
                }
            }
            let _ = hkcu.delete_subkey_all(BACKUP_KEY_PATH);
        } else {
            let _ = key.set_value("ActiveCount", &(count - 1));
        }
    }
}

fn raw_add_alias(k: &str, v: &str) {
    unsafe {
        windows_sys::Win32::System::Console::AddConsoleAliasW(
            format!("{}\0", k).encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _,
            format!("{}\0", v).encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _,
            "cmd.exe\0".encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _
        );
    }
}
