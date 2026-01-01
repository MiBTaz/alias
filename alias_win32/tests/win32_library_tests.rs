// alias_win32/tests/win32lib.rs

use alias_lib::*;
use alias_win32::*;

// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
#[cfg(test)]
#[path = "../../tests/alias_win32_wrapper.rs"]
mod win32_api;

