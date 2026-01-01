// alias_hybrid/tests/win32_library_tests.rs

use alias_lib::*;
#[allow(unused_imports)]
use alias_win32::*;

// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
#[cfg(test)]
#[path = "../../tests/alias_win32_wrapper.rs"]
mod win32_api;

