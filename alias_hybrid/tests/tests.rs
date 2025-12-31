// alias_hybrid/tests/tests.rs

// 1. Pull in the public API of the library
#[allow(unused_imports)]
use alias_lib::*;
#[allow(unused_imports)]
use alias_win32::*;
#[allow(unused_imports)]
use alias_wrapper::*;

/// ---------------------------------------------------------
/// LOCAL TESTS (Specific to this crate)
/// ---------------------------------------------------------
#[test]
fn test_local_logic() {
    // No-op: Placeholder for future local tests
}


/// ---------------------------------------------------------
/// SHARED TESTS (The "Pragma" style include)
/// ---------------------------------------------------------
/// ---------------------------------------------------------
/// 1. THE BRAIN (Logic & Parsing)
/// ---------------------------------------------------------
#[cfg(test)]
#[path = "../../tests/alias_lib_tests.rs"]
mod shared_lib;

/// ---------------------------------------------------------
/// 2. THE WIN32 SPECIALIST
/// ---------------------------------------------------------
#[cfg(test)]
mod win32_specialist {
    #[allow(unused_imports)]
    use alias_win32::*;
    #[allow(unused_imports)]
    use alias_lib::*;

    #[path = "../../../tests/alias_win32_wrapper.rs"]
    mod shared_muscle {
        #[allow(unused_imports)]
        use super::*; // 2. CHILD REACHES UP TO GRAB THE TOOL!
    }
}

/// ---------------------------------------------------------
/// 3. THE WRAPPER SPECIALIST
/// ---------------------------------------------------------
#[cfg(test)]
mod wrapper_specialist {
    #[allow(unused_imports)]
    use alias_wrapper::*; // 1. Parent has the tool...
    #[allow(unused_imports)]
    use alias_lib::*;

    #[path = "../../../tests/alias_win32_wrapper.rs"]
    mod shared_muscle {
        #[allow(unused_imports)]
        use super::*;
    }
}
