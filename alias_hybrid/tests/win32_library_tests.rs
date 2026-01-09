// alias_hybrid/tests/win32_library_tests.rs

#[allow(unused_imports)]
use alias_lib::*;

use alias_win32::Win32LibraryInterface as P;

// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
include!("../../tests/alias_win32_wrapper.rs");

// If alias_win32_wrapper.rs needs access to the imports above,
// make sure that file has 'use crate::*;' or 'use alias_win32::*;' at the top.
