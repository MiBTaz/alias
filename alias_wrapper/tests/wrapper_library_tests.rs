// alias_wrapper/tests/wrapper_library_tests.rs

// use alias_lib::*;
use alias_wrapper::WrapperLibraryInterface as P;

// Instead of 'mod wrapper_specialist', just include the path directly
// so the tests are registered to this file's root.
include!("../../tests/alias_win32_wrapper.rs");

// If alias_win32_wrapper.rs needs access to the imports above,
// make sure that file has 'use crate::*;' or 'use alias_win32::*;' at the top.
