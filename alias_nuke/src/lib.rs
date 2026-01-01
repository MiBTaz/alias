use std::{ptr};
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Console::AddConsoleAliasW;

/// The "FU Windows" Kernel Wipe.
/// This is a library function, not a binary.
pub fn kernel_wipe_macros() {
    let exe = "cmd.exe".encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    unsafe {
        // Direct Kernel hit: NULL Source/Target = TOTAL PURGE
        AddConsoleAliasW(ptr::null(), ptr::null(), exe.as_ptr());
    }
    let active_macros = get_all_aliases();

    for (name, _) in active_macros {
        if !api_set_macro(&name, None) {
            let err = unsafe { GetLastError() };
            eprintln!("{}", err)
        }
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

pub fn get_all_aliases() -> Vec<(String, String)> {
    get_all_aliases_raw()
        .into_iter()
        .filter_map(|line| {
            // split_once('=') ensures we catch "name=" as ("name", "")
            line.split_once('=').map(|(n, v)| (n.to_string(), v.to_string()))
        })
        .collect()
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
