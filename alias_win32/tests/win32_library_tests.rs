// alias_win32/tests/win32_library_tests.rs

use std::path::Path;
#[allow(unused_imports)]
use alias_lib::*;
use alias_win32::Win32LibraryInterface as P;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
include!("../../tests/alias_win32_wrapper.rs");

// If alias_win32_wrapper.rs needs access to the imports above,
// make sure that file has 'use crate::*;' or 'use alias_win32::*;' at the top.
