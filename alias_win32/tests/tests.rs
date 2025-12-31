// alias_win32/tests/tests.rs

// 1. Pull in the public API of the library
#[allow(unused_imports)]
use alias_lib::*;
#[allow(unused_imports)]
use alias_win32::*;

/// ---------------------------------------------------------
/// 1. LOCAL TESTS (Specific to this crate)
/// ---------------------------------------------------------
#[test]
fn test_local_logic() {
    // No-op: Placeholder for future local tests
}

/// ---------------------------------------------------------
/// SHARED TESTS (The "Pragma" style include)
/// ---------------------------------------------------------
/// ---------------------------------------------------------
/// 2. THE BRAIN (Logic & Parsing)
/// ---------------------------------------------------------
#[cfg(test)]
#[path = "../../tests/alias_lib_tests.rs"]
mod shared_lib;

/// ---------------------------------------------------------
/// 3. THE WRAPPER SPECIALIST (Muscle Test 2)
/// ---------------------------------------------------------
#[cfg(test)]
mod wrapper_specialist {
    // 1. Bring the muscle into this room
    #[allow(unused_imports)]
    use alias_lib::*;
    #[allow(unused_imports)]
    use alias_win32::*;

    // 2. Point to the shared test file
    #[path = "../../tests/alias_win32_wrapper.rs"]
    mod shared_muscle {
        // 3. THIS IS THE KEY: Reach up and grab 'set_alias' from the parent room
        #[allow(unused_imports)]
        use super::*;
    }
}