// alias_hybrid/tests/wrapper_library_tests.rs

use alias_lib::*;
#[allow(unused_imports)]
use alias_wrapper::*;


// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
#[cfg(test)]
#[path = "../../tests/alias_win32_wrapper.rs"]
mod wrapper_api;

// If alias_win32_wrapper.rs needs access to the imports above,
// make sure that file has 'use crate::*;' or 'use alias_win32::*;' at the top.
