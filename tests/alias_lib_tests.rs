// tests/alias_lib_tests.rs
use serial_test::serial;
use alias_lib::*;
use std::fs::File;
use std::time::Duration;
use tempfile::tempdir;
use tempfile::NamedTempFile; // Standard for file-based tests
use std::io::Write;
use std::fs;
use alias_lib::{parse_macro_file, Verbosity};

// shared code start
extern crate alias_lib;

#[path = "shared_test_utils.rs"]
mod test_suite_shared;
#[allow(unused_imports)]
use test_suite_shared::{MockProvider, MOCK_RAM, LAST_CALL, global_test_setup};

// shared code end

#[path = "state_restoration.rs"]
mod stateful;
#[cfg(test)]
#[ctor::ctor]
fn local_library_tests_init() {
    eprintln!("[PRE-FLIGHT] Warning: System state is starting.");
    // FORCE LINKAGE: This prevents the linker from tree-shaking the module
    // and silences the "unused" warnings by actually "using" them.
    let _ = stateful::has_backup();
    if stateful::is_stale() {
        // This path probably won't be hit, but the compiler doesn't know that.
        eprintln!("[PRE-FLIGHT] Warning: System state is stale.");
    }
    let _ = stateful::has_backup();
    stateful::pre_flight_inc();
    global_test_setup();
}
#[cfg(test)]
#[ctor::dtor]
fn alias_lib_tests_end() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}

macro_rules! trace {
    ($($arg:tt)*) => {
            eprintln!("[AL-TRACE] {}", format!($($arg)*));
    };
}

#[cfg(test)]
mod macro_tests {
    use super::*;
    #[test]
    fn test_voice_initialization_variants() {
        // Test Case 1: The "Direct Off" optimization
        let mute = voice!(Mute, Off, Off);
        assert_eq!(mute.level, VerbosityLevel::Mute);
        assert!(!mute.show_icons.is_on());
        assert_eq!(mute.display_tip, None);

        // Test Case 2: Explicit On
        let loud = voice!(Loud, ShowIcons::On, ShowTips::On);
        assert_eq!(loud.level, VerbosityLevel::Loud);
        assert!(loud.show_icons.is_on());
        // Since tips are 'On', display_tip MUST be Some
        assert!(loud.display_tip.is_some());

        // Test Case 3: Explicit Off via expr
        let silent = voice!(Silent, ShowIcons::Off, ShowTips::Off);
        assert_eq!(silent.display_tip, None);
    }
}

#[cfg(test)]
mod battery_1 {
    use super::*;
    pub fn is_valid_value(val: &str) -> bool {
        // Values are payloads. We only block what would physically
        // break the .doskey file format (Newlines).
        !val.contains('\n') && !val.contains('\r')
    }
    #[test]
    fn test_macro_formatting_expansion() {
        let v = Verbosity::normal(); // Icons are ON by default

        // Testing Case 1 of the macro: Icon + Format String + Args
        // This manually checks what the macro does under the hood
        let msg = format!("Hello {}", "World");
        let formatted = v.icon_format(AliasIcon::Success, &msg);

        // Verify the icon from the ICON_MATRIX is present
        // Success icon (Index 5) in "On" mode is "✨"
        assert!(formatted.contains("✨"));
        assert!(formatted.contains("Hello World"));
    }

    #[test]
    fn test_macro_resilience_to_shell_characters() {
        let v = Verbosity::loud();
        // Test if the macro handles raw shell commands as strings
        // If the macro expansion is wrong, this might fail to compile or crash
        v.shout(&format!("Command: {} {} & pause", v.get_icon_str(AliasIcon::Scream), "dir /s"));
        scream!(v, "Critical: %PATH% is corrupted!");
    }

    #[test]
    fn test_to_bool_conversion_logic() {
        // 1. Test literal matches (The "Fast Path")
        assert!(to_bool!(On));
        assert!(!to_bool!(Off));
        assert!(to_bool!(ShowIcons::On));

        // 2. Test the expr fallback (The "Brain")
        let icons = ShowIcons::On;
        assert!(to_bool!(icons));

        let tips = ShowTips::Off;
        assert!(!to_bool!(tips));
    }

    #[test]
    fn test_gatekeeper_edge_cases() {
        // Bad Names
        assert!(!is_valid_name("-invalid"));    // No leading hyphen
        assert!(!is_valid_name("alias&name")); // No shell operators in name
        assert!(!is_valid_name(""));           // No empty names
        assert!(!is_valid_name("1alias"));     // No leading digits (strict mode)

        // Valid but "Dangerous" Names (Unicode/Special)
        assert!(is_valid_name("alias_1"));     // Underscores ok
        assert!(is_valid_name("ñandú"));       // Unicode support check

        // Values (The "Doskey Impossible" Payload)
        // Values should allow almost anything EXCEPT naked newlines
        assert!(is_valid_value("echo %PATH% & pause"));
        assert!(is_valid_value("dir /s | find \"test\""));
    }

    #[test]
    fn test_voice_initialization_logic() {
        // 1. Test the "Direct Off" optimized branch
        let mute = voice!(Mute, Off, Off);
        assert_eq!(mute.level, VerbosityLevel::Mute);
        assert_eq!(mute.display_tip, None, "Direct Off should never have a tip");

        // 2. Test the General Case with 'On'
        // This verifies the 'match' logic inside the macro
        let loud = voice!(Loud, ShowIcons::On, ShowTips::On);
        assert!(loud.display_tip.is_some(), "Tips set to On must generate a string");

        // 3. Test the General Case with 'Off' (passed as expr)
        let silent_icons = ShowIcons::Off;
        let silent_tips = ShowTips::Off;
        let quiet = voice!(Normal, silent_icons, silent_tips);
        assert_eq!(quiet.display_tip, None, "Tips set to Off via expr should be None");
    }
}

#[cfg(test)]
mod thorough_macro_tests {
    use alias_lib::{voice, ShowIcons, ShowTips};

    #[test]
    fn test_voice_fast_path_identity() {
        // This ensures the specialized 'Off, Off' branch
        // matches the behavior of the general branch.
        let fast_mute = voice!(Mute, Off, Off);
        let general_mute = voice!(Mute, ShowIcons::Off, ShowTips::Off);

        assert_eq!(fast_mute.level, general_mute.level);
        assert_eq!(fast_mute.show_icons, general_mute.show_icons);
        assert_eq!(fast_mute.show_tips, general_mute.show_tips);
        assert_eq!(fast_mute.display_tip, None);
    }

    #[test]
    fn test_voice_tip_allocation_logic() {
        // Case: Tips ON
        // Verifies that the 'match' arm triggers the tip generator
        let v_on = voice!(Normal, ShowIcons::On, ShowTips::On);
        assert!(v_on.display_tip.is_some(), "Tips On should allocate a string");

        // Case: Tips OFF
        // Verifies the None variant
        let v_off = voice!(Normal, ShowIcons::On, ShowTips::Off);
        assert!(v_off.display_tip.is_none(), "Tips Off must be None");
    }

    #[test]
    fn test_voice_expression_hygiene() {
        // This tests if the macro can handle expressions (function calls)
        // as arguments without double-evaluating or failing to match.
        fn get_icons() -> ShowIcons { ShowIcons::On }

        let v = voice!(Normal, get_icons(), ShowTips::Off);
        assert!(v.show_icons.is_on());
    }

    #[test]
    fn test_voice_random_branch_stability() {
        // Since Random uses a dice roll (random_tip_show), we test
        // that it compiles and returns a valid Option<String> structure.
        let v_rand = voice!(Normal, ShowIcons::On, ShowTips::Random);

        // We don't care if it's Some or None (it's random),
        // we just care that the struct initialized without panicking.
        let _ = format!("{:?}", v_rand.display_tip);
    }
}

#[cfg(test)]
mod battery_2 {
    use alias_lib::{failure, Verbosity};

    #[test]
    fn test_failure_macro_os_error_extraction() {
        let v = Verbosity::normal();

        // 1. Test Pattern 1: Real OS Error (e.g., File Not Found)
        let raw_os_err = std::io::Error::from_raw_os_error(2); // ERROR_FILE_NOT_FOUND
        let alias_err = failure!(v, raw_os_err);

        assert_eq!(alias_err.code, 2);
        assert!(alias_err.message.contains("2")); // Message usually contains the code
    }

    #[test]
    fn test_failure_macro_custom_logic_error() {
        let v = Verbosity::normal();

        // 2. Test Pattern 2: Custom Code + Formatted Message
        // Simulating: (verbosity, ErrorCode, "fmt", args)
        let code = 42u8;
        let alias_err = failure!(v, code, "Validation failed for: {}", "test_alias");

        assert_eq!(alias_err.code, 42);
        assert!(alias_err.message.contains("Validation failed for: test_alias"));
        // Verify icon presence (Fail icon is usually ⚠️ or ❌)
        assert!(alias_err.message.contains("⚠️") || alias_err.message.contains("❌"));
    }

    #[test]
    fn test_failure_macro_fallback_code() {
        let v = Verbosity::normal();

        // Test that a generic error with no OS code defaults to 1
        let custom_err = std::io::Error::new(std::io::ErrorKind::Other, "oh no");
        let alias_err = failure!(v, custom_err);

        assert_eq!(alias_err.code, 1);
    }

    #[test]
    fn test_failure_macro_type_brittleness() {
        let v = Verbosity::normal();

        // Test Case: Does it handle numeric casting correctly?
        // Passing a u32 that exceeds u8 (256) should truncate or wrap
        // depending on 'as' behavior. You want to KNOW this happens.
        let code_32: u32 = 257;
        let err = failure!(v, code_32, "Truncation test");

        // In Rust, '257 as u8' is 1.
        assert_eq!(err.code, 1, "Verify truncation behavior for OS exit codes");
    }

    #[test]
    fn test_failure_macro_payload_safety() {
        let v = Verbosity::normal();

        // Test Case: Error without OS code (Custom IO Error)
        let custom = std::io::Error::new(std::io::ErrorKind::AddrInUse, "Port Busy");
        let err = failure!(v, custom);

        // If the macro breaks here, it's likely because of the .unwrap_or(1)
        assert_eq!(err.code, 1);
        assert!(err.message.contains("Port Busy"));
    }

    #[test]
    fn test_failure_macro_formatting_stress() {
        let v = Verbosity::normal();

        // Testing if multiple arguments and special characters break the macro
        let err = failure!(v, 5, "Failed: {} | {} < {}", "A", "B", "C");

        assert_eq!(err.code, 5);
        // Ensure shell characters were preserved in the message
        assert!(err.message.contains("|"));
        assert!(err.message.contains("<"));
    }


}

#[cfg(test)]
mod failure_macro_integrity_tests {
    use std::io::{Error, ErrorKind};
    use alias_lib::{failure, Verbosity};

    #[test]
    fn test_ownership_persistence() {
        let v = Verbosity::normal();
        let err_one = Error::new(ErrorKind::Other, "First");
        let err_two = Error::new(ErrorKind::Other, "Second");

        // If the macro takes ownership of 'v', this second call will fail to compile.
        // This ensures the macro is "leak-proof" regarding your Verbosity state.
        let _ = failure!(v, err_one);
        let _ = failure!(v, err_two);
    }

    #[test]
    fn test_exit_code_truncation_safety() {
        let v = Verbosity::normal();

        // Windows exit codes are technically 32-bit, but Doskey/CMD
        // usually expects 0-255. We test if 'as u8' handles overflow
        // predictably (wrapping) rather than panicking.
        let large_code: u32 = 258; // 258 % 256 = 2
        let res = failure!(v, large_code, "Testing overflow");

        assert_eq!(res.code, 2);
    }

    #[test]
    fn test_trait_compatibility() {
        let v = Verbosity::normal();

        // Pattern 2 requires the error code to be castable to u8.
        // We test with different integer types to ensure the 'as u8'
        // is robust across the suite.
        let code_i32: i32 = 5;
        let code_usize: usize = 10;

        let res_a = failure!(v, code_i32, "i32 test");
        let res_b = failure!(v, code_usize, "usize test");

        assert_eq!(res_a.code, 5);
        assert_eq!(res_b.code, 10);
    }
}

#[cfg(test)]
mod battery_3 {
    use alias_lib::{failure, AliasError, ErrorCode, Verbosity};

    #[test]
    fn test_error_code_mapping_integrity() {
        // Verify that the repr(u8) values match our expectations
        assert_eq!(ErrorCode::Generic as u8, 1);
        assert_eq!(ErrorCode::Registry as u8, 5);
        assert_eq!(ErrorCode::MissingName as u8, 7);
    }

    #[test]
    fn test_alias_error_rugged_display() {
        let msg = "✨ Success turned ⚠️ Failure".to_string();
        let err = AliasError {
            message: msg.clone(),
            code: 6, // Access Denied
        };

        // Test Display implementation
        assert_eq!(format!("{}", err), msg);

        // Test Debug implementation (should include struct metadata)
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("AliasError"));
        assert!(debug_str.contains("code: 6"));
    }

    #[test]
    fn test_trait_object_safety() {
        let err = AliasError {
            message: "File missing".to_string(),
            code: 3,
        };

        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.to_string().contains("File missing"));
    }

    #[test]
    fn test_guaranteed_nonzero_exit() {
        let v = Verbosity::normal();
        // Even if we pass an OS error with no code, failure! must default to 1 (Generic)
        let custom_io_err = std::io::Error::new(std::io::ErrorKind::Other, "Generic failure");
        let err = failure!(v, custom_io_err);

        assert!(err.code > 0, "Errors must always return a non-zero exit code for shell automation.");
    }
}

#[cfg(test)]
mod task_queue_tests {
    use alias_lib::{AliasAction, SetOptions, Task, TaskQueue};

    #[test]
    fn test_queue_lifecycle_and_growth() {
        let mut q = TaskQueue::new();
        assert!(q.is_empty());

        // Test capacity jump: pushing 5 items (exceeding initial 4)
        for i in 0..5 {
            q.push(AliasAction::Set(SetOptions {
                name: format!("alias_{}", i),
                value: "dir".to_string(),
                volatile: false,
                force_case: false,
            }));
        }

        assert_eq!(q.len(), 5);
        assert!(!q.is_empty());

        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn test_into_iterator_consumption() {
        let mut q = TaskQueue::new();
        q.push(AliasAction::Clear);
        q.push(AliasAction::Clear);

        // IntoIterator should consume the queue
        let tasks: Vec<Task> = q.into_iter().collect();
        assert_eq!(tasks.len(), 2);

        // Note: q is now moved/consumed, preventing double-processing bugs
    }

    #[test]
    fn test_out_of_bounds_safety() {
        let q = TaskQueue::new();
        // Ensure the getter handles empty queues/high indices gracefully
        assert!(q.get(0).is_none());
        assert!(q.get(999).is_none());
    }
}

#[cfg(test)]
mod icon_system_tests {
    use alias_lib::{AliasIcon, ICON_MATRIX, ICON_TYPES};

    #[test]
    fn test_icon_matrix_bounds() {
        // Ensure the Matrix length exactly matches the Enum variant count
        // This catches the "I added an enum but forgot the matrix" bug
        assert_eq!(ICON_MATRIX.len(), ICON_TYPES,
                   "ICON_MATRIX size mismatch! Did you add an AliasIcon but forget to update the matrix?");
    }

    #[test]
    fn test_icon_indexing_integrity() {
        // Test a few key indices to ensure the repr(usize) is working as expected
        assert_eq!(AliasIcon::None as usize, 0);
        assert_eq!(AliasIcon::Success as usize, 5);
        assert_eq!(AliasIcon::Question as usize, 19);

        // Verify specifically that Success (5) points to the "✨" icon in Unicode mode
        assert_eq!(ICON_MATRIX[AliasIcon::Success as usize][1], "✨");

        assert_eq!(ICON_MATRIX[AliasIcon::Fail as usize][0], "X");
    }

    #[test]
    fn test_variant_count_logic() {
        // Ensure _VariantCount is actually at the end
        // If this isn't the last element, ICON_TYPES will be wrong
        let last_valid_icon = AliasIcon::Architect as usize;
        assert_eq!(last_valid_icon + 1, ICON_TYPES);
    }
}

#[cfg(test)]
mod verbosity_carrier_tests {
    use alias_lib::{AliasIcon, ShowTips, Verbosity, VerbosityLevel};


    #[test]
    fn test_verbosity_factory_states() {
        // Ensure the factory methods produce the correct "Logic Gates"
        let mute = Verbosity::mute();
        assert_eq!(mute.level, VerbosityLevel::Mute);
        assert!(!mute.show_icons.is_on());

        let loud = Verbosity::loud();
        assert!(loud.show_audit());
        assert!(loud.show_xmas_lights());
        assert!(loud.display_tip.is_some() || loud.show_tips == ShowTips::On);
    }

    #[test]
    fn test_icon_formatting_passthrough() {
        let silent = Verbosity::silent();
        let msg = "test message";

        // In Silent/Off mode, icon_format should be a NO-OP (Returning original string)
        assert_eq!(silent.icon_format(AliasIcon::Success, msg), msg);

        let normal = Verbosity::normal();
        // In Normal mode, it should contain the icon from the matrix
        let formatted = normal.icon_format(AliasIcon::Success, msg);
        assert!(formatted.contains(msg));
        assert!(formatted.len() > msg.len());
    }

    #[test]
    fn test_audit_block_alignment_logic() {
        let v_icons = Verbosity::normal(); // Icons On
        let v_no_icons = Verbosity::silent(); // Icons Off (effectively)

        // We verify the logic inside `property` and `align` regarding spacers
        // Icons take 2 spaces worth of width
        let icon_spacer = if v_icons.show_icons.is_on() { "  " } else { " " };
        assert_eq!(icon_spacer, "  ");

        let no_icon_spacer = if v_no_icons.show_icons.is_on() { "  " } else { " " };
        assert_eq!(no_icon_spacer, " ");
    }
}

#[cfg(test)]
mod const_integrity_tests {
    use alias_lib::{DEFAULT_ALIAS_FILENAME, ENV_ALIAS_FILE, ENV_ALIAS_OPTS, REG_SUBKEY};

    #[test]
    fn test_registry_path_validity() {
        // Ensure REG_SUBKEY does not start or end with a backslash.
        // winreg/Windows API usually expects the subkey relative to the hive.
        assert!(!REG_SUBKEY.starts_with('\\'), "Registry subkeys should be relative, not absolute.");
        assert!(!REG_SUBKEY.ends_with('\\'), "Trailing backslashes can break registry key lookups.");

        // Ensure it contains the expected spaces for the Command Processor
        assert!(REG_SUBKEY.contains("Command Processor"));
    }

    #[test]
    fn test_environment_key_naming_convention() {
        // Rust's env::var is case-sensitive. Standard practice is UPPERCASE.
        // This ensures a refactor doesn't accidentally lowercase a lookup key.
        assert_eq!(ENV_ALIAS_FILE, "ALIAS_FILE");
        assert_eq!(ENV_ALIAS_OPTS, "ALIAS_OPTS");
    }

    #[test]
    fn test_file_extension_validity() {
        // Doskey files require specific formatting.
        // We ensure the default filename at least has an extension.
        assert!(DEFAULT_ALIAS_FILENAME.ends_with(".doskey") || DEFAULT_ALIAS_FILENAME.ends_with(".txt"));
    }
}

#[cfg(test)]
mod show_feature_tests {
    use alias_lib::{DisplayTip, ShowFeature, ShowIcons, ShowTips};

    #[test]
    fn test_show_feature_logic_gates() {
        let on = ShowFeature::On;
        let off = ShowFeature::Off;

        // Test the 'is_on' helper
        assert!(on.is_on());
        assert!(!off.is_on());

        // Test the 'Not' operator implementation
        // This is how you'll handle toggle flags in the CLI
        assert_eq!(!on, ShowFeature::Off);
        assert_eq!(!off, ShowFeature::On);
        assert_eq!(!!on, ShowFeature::On);
    }

    #[test]
    fn test_repr_values() {
        // Since we defined On=1 and Off=0, we ensure
        // they cast to the correct integers for C-style API calls
        // or registry storage.
        assert_eq!(ShowFeature::On as usize, 1);
        assert_eq!(ShowFeature::Off as usize, 0);
    }

    #[test]
    fn test_comparisons() {
        // Because of PartialOrd/Ord, On (1) should be "greater" than Off (0)
        // This is useful for sorting or filtering audit results.
        assert!(ShowFeature::On > ShowFeature::Off);
    }

    #[test]
    fn test_show_tips_state_transitions() {
        let t_on = ShowTips::On;
        let t_off = ShowTips::Off;
        let t_rand = ShowTips::Random;

        // Test helper methods
        assert!(t_on.is_on());
        assert!(!t_off.is_on());
        assert!(!t_rand.is_on()); // Random is NOT "Explicitly On"

        assert!(t_rand.random());
        assert!(!t_on.random());

        // Identity check
        assert_ne!(t_on, t_rand);
        assert_ne!(t_off, t_rand);
    }

    #[test]
    fn test_feature_alias_behavior() {
        // Ensuring the type aliases behave like ShowFeature
        let icons: ShowIcons = ShowIcons::On;
        let tips: DisplayTip = DisplayTip::Off;

        // Test boolean coercion (using your is_on helper from ShowFeature)
        assert!(icons.is_on());
        assert!(!tips.is_on());

        // Test the Not operator on the alias
        let toggled_icons = !icons;
        assert_eq!(toggled_icons, ShowIcons::Off);
    }
}

#[cfg(test)]
mod set_options_tests {
    use alias_lib::SetOptions;

    #[test]
    fn test_set_options_structural_integrity() {
        let name = "test_alias".to_string();
        let value = "dir /w".to_string();

        let opts = SetOptions {
            name: name.clone(),
            value: value.clone(),
            volatile: false,
            force_case: true,
        };

        // Ensure cloning preserves the exact intent
        let cloned_opts = opts.clone();
        assert_eq!(cloned_opts.name, name);
        assert_eq!(cloned_opts.value, value);
        assert!(!cloned_opts.volatile);
        assert!(cloned_opts.force_case);
    }

    #[test]
    fn test_set_options_equality() {
        // This is critical for deduplication in the TaskQueue
        let opt_a = SetOptions {
            name: "ls".to_string(),
            value: "dir".to_string(),
            volatile: true,
            force_case: false,
        };

        let opt_b = SetOptions {
            name: "ls".to_string(),
            value: "dir".to_string(),
            volatile: true,
            force_case: false,
        };

        assert_eq!(opt_a, opt_b);
    }
}

#[cfg(test)]
mod registry_status_tests {
    use alias_lib::RegistryStatus;

    #[test]
    fn test_status_equality_and_matching() {
        let status = RegistryStatus::Synced;

        // Verify we can match on the variants
        match status {
            RegistryStatus::Synced => assert!(true),
            _ => panic!("Should have been Synced"),
        }

        let mismatch = RegistryStatus::Mismatch("dir /w".to_string());
        if let RegistryStatus::Mismatch(val) = mismatch {
            assert_eq!(val, "dir /w");
        } else {
            panic!("Should have been a Mismatch");
        }
    }

    #[test]
    fn test_mismatch_payload_integrity() {
        // Testing that the Mismatch variant can hold complex "Doskey Impossible" strings
        let dangerous_payload = "echo %PATH% & pause".to_string();
        let status = RegistryStatus::Mismatch(dangerous_payload.clone());

        if let RegistryStatus::Mismatch(inner) = status {
            assert_eq!(inner, dangerous_payload);
        }
    }
}

#[cfg(test)]
mod help_mode_tests {
    use alias_lib::HelpMode;

    #[test]
    fn test_help_mode_variants() {
        let short = HelpMode::Short;
        let full = HelpMode::Full;

        // Verify they are distinct
        // (Manual check since you didn't derive PartialEq,
        // though you likely should for the rewrite!)
        match short {
            HelpMode::Short => assert!(true),
            _ => panic!("Expected Short"),
        }
        match full {
            HelpMode::Full => assert!(true),
            _ => panic!("Expected Full"),
        }
    }

    #[test]
    fn test_help_mode_copy_semantics() {
        // Because it's Copy, we should be able to pass it
        // around without worrying about ownership.
        let mode = HelpMode::Short;
        let _mode2 = mode;
        let _mode3 = mode; // No move error here
    }
}

#[cfg(test)]
mod battery_4 {
    use std::path::PathBuf;
    use alias_lib::{AliasEntryMesh, DiagnosticReport, PurgeReport, RegistryStatus};

    #[test]
    fn test_purge_report_ruggedness() {
        let mut report = PurgeReport::default();

        // Test success tracking
        report.cleared.push("ls".to_string());
        assert!(!report.cleared.is_empty());
        assert!(report.is_fully_clean()); // No failures yet

        // Test failure tracking with Win32 Error Codes
        report.failed.push(("git".to_string(), 5)); // 5 = Access Denied

        assert_eq!(report.failed.len(), 1);
        assert!(!report.is_fully_clean(), "Should flag as not clean if a failure exists");
    }

    #[test]
    fn test_diagnostic_report_defaults() {
        let report = DiagnosticReport {
            binary_path: None,
            resolved_path: PathBuf::from("C:\\bin\\alias.exe"),
            env_file: "aliases.doskey".to_string(),
            env_opts: "-u".to_string(),
            file_exists: false,
            is_readonly: true,
            drive_responsive: true,
            registry_status: RegistryStatus::NotFound,
            api_status: Some("SPAWNER".to_string()),
        };

        assert!(!report.file_exists);
        assert_eq!(report.registry_status.is_synced(), false);
    }

    #[test]
    fn test_mesh_definition_logic() {
        // Test Case: Pure Empty (should be impossible in practice, but ruggedized)
        let empty_mesh = AliasEntryMesh {
            name: "test".to_string(),
            os_value: None,
            file_value: None,
        };
        assert!(empty_mesh.is_empty_definition());

        // Test Case: Ghost Alias
        let ghost = AliasEntryMesh {
            name: "ghost".to_string(),
            os_value: Some("dir".to_string()),
            file_value: None,
        };
        assert!(!ghost.is_empty_definition());
    }
}

#[cfg(test)]
mod alias_action_tests {
    use alias_lib::{AliasAction, SetOptions};

    #[test]
    fn test_display_fidelity_set() {
        let opts = SetOptions {
            name: "ls".to_string(),
            value: "dir".to_string(),
            volatile: false,
            force_case: false,
        };
        let action = AliasAction::Set(opts);

        // Ensure the display string correctly identifies the intent
        assert_eq!(format!("{}", action.error()), "Error setting alias: ls");
    }

    #[test]
    fn test_display_fidelity_edit() {
        // Case A: Custom Editor
        let action_custom = AliasAction::Edit(Some("code.exe".to_string()));
        assert!(format!("{}", action_custom.error()).contains("code.exe"));

        // Case B: Default Editor
        let action_default = AliasAction::Edit(None);
        assert!(format!("{}", action_default.error()).contains("default editor"));
    }

    #[test]
    fn test_enum_clonability_for_queue() {
        // Since TaskQueue holds a Vec<Task>, AliasAction must be efficiently clonable
        let original = AliasAction::Query("test".to_string());
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }
}

#[cfg(test)]
mod alias_provider_logic_tests {
    use super::*;
    use alias_lib::{AliasProvider, Verbosity};

    #[test]
    fn test_purge_logic_completeness() {

        let v = Verbosity::mute();
        // 1. Setup Mock RAM
        {
            let mut ram = MOCK_RAM.lock().unwrap();
            ram.push(("ls".into(), "dir".into()));
            ram.push(("gs".into(), "git status".into()));
        }

        // 2. Run the Trait's default purge logic
        let report = MockProvider::purge_ram_macros(&v).unwrap();

        // 3. Verify
        assert_eq!(report.cleared.len(), 2);
        assert!(MOCK_RAM.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod battery_5 {
    use std::path::PathBuf;

    #[test]
    fn test_autorun_command_construction() {
        // We want to verify that constructing the command handles spaces correctly.
        let exe_path = PathBuf::from("C:\\Program Files\\Alias\\alias.exe");
        let alias_file = PathBuf::from("C:\\Users\\Guest\\Documents\\aliases.doskey");

        // Simulate the construction logic inside install_autorun
        let startup_args = format!("--file \"{}\" --startup", alias_file.display());
        let our_cmd = format!("\"{}\" {}", exe_path.display(), startup_args);

        // This is the "Ruggedness" check: The command must be valid for CMD.exe
        assert!(our_cmd.starts_with("\"C:\\"), "Executable must be quoted for paths with spaces");
        assert!(our_cmd.contains("--file \"C:\\"), "File argument must be quoted");
    }

    #[test]
    fn test_reload_logic_comment_filtering() {
        let mock_content = "
        ; This is a comment
        ls=dir

        gs=git status ; inline comments are handled by the engine
    ";

        // Logic from your reload_full implementation:
        let count = mock_content.lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with(';'))
            .count();

        assert_eq!(count, 2, "Should only count 'ls' and 'gs', ignoring comments and whitespace");
    }
}

#[cfg(test)]
mod directory_logic_tests {
    use alias_lib::get_alias_exe;

    #[test]
    fn test_alias_binary_and_directory_resolution() {
        // 1. Test the EXE resolution (The Anchor)
        let exe_result = get_alias_exe();
        assert!(exe_result.is_ok(), "Should always resolve the current exe path");

        let exe_path = exe_result.unwrap();
        assert!(exe_path.is_absolute(), "The binary path must be absolute for Registry reliability");
        assert!(exe_path.extension().is_some(), "The binary should have an extension (likely .exe)");

        // 2. Test the Directory resolution (The Parent)
        // We "unwrap the dir" manually from the exe path we just verified
        let dir_path = exe_path.parent()
            .expect("The running executable must have a parent directory");

        assert!(dir_path.is_absolute(), "The resolved directory must stay absolute");

        // 3. Physical Validation (if running in a real filesystem)
        if exe_path.exists() {
            assert!(exe_path.is_file(), "get_alias_exe must point to a file");
            assert!(dir_path.is_dir(), "The parent of the exe must be a directory");

            // Ensure the file is actually inside that directory
            assert!(exe_path.starts_with(dir_path), "The binary must reside within the resolved directory");
        }
    }
    #[test]
    fn test_parent_fallback_logic() {
        // Simulating the 'unwrap_or(&p)' logic
        // If a path has no parent (like a root drive), it should return itself
        let root_path = std::path::PathBuf::from("C:\\");
        let resolved = root_path.parent().unwrap_or(&root_path).to_path_buf();

        assert_eq!(resolved, root_path, "In root directories, the path should fall back to itself");
    }
}

#[cfg(test)]
mod validation_tests {
    use alias_lib::is_valid_name_loose;

    #[test]
    fn test_valid_name_loose_positives() {
        assert!(is_valid_name_loose("ls"));
        assert!(is_valid_name_loose("7zip"));
        assert!(is_valid_name_loose("Update-All")); // Hyphens in the middle are okay
    }

    #[test]
    fn test_valid_name_loose_negatives() {
        // 1. Empty string check (The unwrap() guard)
        assert!(!is_valid_name_loose(""));

        // 2. Shell Redirection starts (High Risk)
        assert!(!is_valid_name_loose(">output"));
        assert!(!is_valid_name_loose("&run"));
        assert!(!is_valid_name_loose("|pipe"));

        // 3. Leading whitespace
        assert!(!is_valid_name_loose(" alias"));
    }

    #[test]
    fn test_valid_name_loose_stress_unicode() {
        // Does it handle non-ASCII alphanumeric characters correctly?
        // Rust's is_alphabetic() is Unicode-aware.
        assert!(is_valid_name_loose("λ")); // Greek Lambda
        assert!(is_valid_name_loose("ñ")); // Spanish n
    }
}

#[cfg(test)]
mod validation_permissive_tests {
    use alias_lib::is_valid_name_permissive;

    #[test]
    fn test_permissive_positives() {
        assert!(is_valid_name_permissive("ls"));
        assert!(is_valid_name_permissive("update_all")); // Underscores in middle
        assert!(is_valid_name_permissive("_init"));      // Underscores at start
    }

    #[test]
    fn test_permissive_whitespace_block() {
        // These are the "Silent Killers" of Doskey
        assert!(!is_valid_name_permissive("ls "));    // Trailing space
        assert!(!is_valid_name_permissive(" ls"));    // Leading space
        assert!(!is_valid_name_permissive("git st")); // Middle space
    }

    #[test]
    fn test_permissive_digit_logic() {
        // Logic Check: Does your code allow leading digits?
        // 'first.is_alphabetic() || first == '_'' returns false for '7zip'
        // This makes the middle man STRICTER than 'loose' for numbers.
        assert!(!is_valid_name_permissive("7zip"), "Permissive should block leading digits");
    }

    #[test]
    fn test_permissive_trim_integrity() {
        // 'name.trim() != name' catches tabs and newlines too
        assert!(!is_valid_name_permissive("ls\t"));
        assert!(!is_valid_name_permissive("ls\n"));
    }
}

#[cfg(test)]
mod validation_gatekeeper_tests {
    use alias_lib::is_valid_name;

    #[test]
    fn test_gatekeeper_blacklist() {
        // These should all fail immediately at the Gatekeeper level
        assert!(!is_valid_name("alias&"), "Failed to block shell 'and' operator");
        assert!(!is_valid_name("path:"), "Failed to block colon (potential drive/stream)");
        assert!(!is_valid_name("quoted\""), "Failed to block quotes");
        assert!(!is_valid_name("sub(shell)"), "Failed to block parentheses");
        assert!(!is_valid_name("escape^"), "Failed to block CMD caret escape");
    }

    #[test]
    fn test_gatekeeper_first_char_logic() {
        // Gatekeeper blocks digits at the start, unlike 'Loose'
        assert!(!is_valid_name("7zip"), "Gatekeeper should block leading digits");

        // Gatekeeper allows these to pass to Permissive
        assert!(is_valid_name("ls"));
        assert!(is_valid_name("_internal"));
    }

    #[test]
    fn test_gatekeeper_unicode_safety() {
        // Since first.is_alphabetic() is used, Kanji/International chars pass
        // to the next layers.
        assert!(is_valid_name("命令")); // 'Command' in Japanese
    }
}

#[cfg(test)]
#[serial]
mod path_resolution_tests {
    use alias_lib::{get_alias_path, DEFAULT_ALIAS_FILENAME, ENV_ALIAS_FILE};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_env_override_panic() {
        // Mocking the environment variable
        let test_path = "C:\\custom\\path.doskey";
        unsafe {
            std::env::set_var(ENV_ALIAS_FILE, test_path);
        }

        let resolved = get_alias_path("").unwrap();
        assert_ne!(resolved.to_str().unwrap(), test_path);
        unsafe {
            std::env::remove_var(ENV_ALIAS_FILE);
        }
    }

    #[test]
    #[serial]
    fn test_env_override_priority() {
        // Use a path that can actually exist
        let temp_dir = std::env::temp_dir().join("alias_test_override");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("path.doskey");

        unsafe {
            std::env::set_var(ENV_ALIAS_FILE, test_path.to_str().unwrap());
        }

        let resolved = get_alias_path("").unwrap();
        trace!("Resolved={:?}", resolved);
        assert_eq!(resolved, test_path);
        unsafe {
            std::env::remove_var(ENV_ALIAS_FILE);
        }
        let _ = std::fs::remove_dir_all(&temp_dir); // Cleanup
    }

    #[test]
    #[serial]
    fn test_env_directory_auto_join_one() {
        // If the env var points to a directory, it should append the filename
        let temp_dir = std::env::temp_dir();
        unsafe {
            std::env::set_var(ENV_ALIAS_FILE, temp_dir.to_str().unwrap());
        }

        let _resolved = get_alias_path("").unwrap();
        trace!("Resolved={:?}", _resolved);
        assert!(_resolved.ends_with(DEFAULT_ALIAS_FILENAME));
        unsafe {
            std::env::remove_var(ENV_ALIAS_FILE);
        }
    }

    #[test]
    #[serial]
    fn test_env_directory_auto_join() {
        // If the env var points to a directory, it should append the filename
        let temp_dir = std::env::temp_dir();
        let expected = temp_dir.join(DEFAULT_ALIAS_FILENAME); // Define expected path

        unsafe { std::env::set_var(ENV_ALIAS_FILE, temp_dir.to_str().unwrap()); }

        let res = get_alias_path("").unwrap(); // Capture the result

        // --- THE FIX: Assertions to use the variables ---
        assert_eq!(res, expected, "Should have joined ALIAS_FILE directory with default filename");
        assert!(res.is_absolute(), "Resolved path must be absolute");

        unsafe { std::env::remove_var(ENV_ALIAS_FILE); }
    }

    #[test]
    #[serial]
    fn test_fallback_search_logic() {
        // Ensure that if ENV is missing, we at least get a path back
        // provided APPDATA or USERPROFILE exists on the test machine.
        let path = get_alias_path("");
        if let Some(p) = path {
            assert!(p.is_absolute());
            assert!(p.to_str().unwrap().contains("alias_tool"));
        }
    }
}

#[cfg(test)]
mod tip_engine_tests {
    use alias_lib::{get_random_tip, TIPS_ARRAY};

    #[test]
    fn test_tip_selection_bounds() {
        // Run it 100 times to ensure the time-based seed never produces
        // an index out of the array's bounds.
        for _ in 0..100 {
            let tip = get_random_tip();
            assert!(!tip.is_empty());
        }
    }

    #[test]
    fn test_tip_content_safety() {
        // Ensure no tips contain raw newlines that would break
        // the Verbosity::tip() single-line formatting.
        for tip in TIPS_ARRAY {
            assert!(!tip.contains('\n'), "Tip contains a newline: {}", tip);
            assert!(!tip.contains('\r'), "Tip contains a carriage return: {}", tip);
        }
    }

    #[test]
    fn test_randomness_spread() {
        // Verify that consecutive calls (with a tiny sleep) can produce different tips
        let _tip1 = get_random_tip();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let _tip2 = get_random_tip();

        // This isn't a guarantee, but over 1000 trials, they shouldn't all be the same
        // (Ensures the seed is actually moving)
    }
}

#[cfg(test)]
mod battery_6 {
    use alias_lib::random_num_bounded;

    #[test]
    fn test_random_num_bounded_zero_limit() {
        // The "malloc" check: limit 0 should return 0 safely, not panic.
        let result = random_num_bounded(0);
        assert_eq!(result, 0, "Limit of 0 must return 0 index without panicking (Modulo Zero Guard)");
    }

    #[test]
    fn test_random_num_bounded_unit_limit() {
        // The "Identity" check: limit 1 should always return 0.
        for _ in 0..100 {
            assert_eq!(random_num_bounded(1), 0, "Limit of 1 must always return index 0");
        }
    }

    #[test]
    fn test_random_num_bounded_range_safety() {
        let limit = 40;
        for _ in 0..1000 {
            let result = random_num_bounded(limit);
            // Mathematically: 0 <= result < limit
            assert!(result < limit, "Result {} must be within bounds of limit {}", result, limit);
        }
    }

    #[test]
    fn test_entropy_distribution() {
        // While not a perfect PRNG test, we want to ensure it's not returning
        // the same number every single time (proving the XOR jitter works).
        let limit = 1000000;
        let first = random_num_bounded(limit);

        // We might need a tiny sleep to ensure time moves,
        // but even if it doesn't, ASLR/PID should help.
        let second = random_num_bounded(limit);

        // This is a soft check: in 1000000 options, they shouldn't collide
        // immediately if entropy is functioning.
        assert_ne!(first, second, "Successive calls should ideally produce different seeds");
    }
}


#[cfg(test)]
mod general_random_tests {
    use alias_lib::random_num_bounded;

    #[test]
    fn test_maximum_usize_limit() {
        // If limit is usize::MAX, we are modding by a massive number.
        // On 64-bit, that's 18,446,744,073,709,551,615.
        // final_seed is u128, so this math is safe from overflow.
        let limit = usize::MAX;
        let result = random_num_bounded(limit);

        assert!(result < limit, "Even at usize::MAX, the result must be valid");
    }

    #[test]
    fn test_the_one_limit_edge_case() {
        // As we discussed, math says X % 1 is always 0.
        // This test ensures the function doesn't try to do something "clever"
        // that breaks the mathematical law.
        assert_eq!(random_num_bounded(1), 0);
    }
}

#[cfg(test)]
mod battery_7 {
    use alias_lib::get_random_tip;

    #[test]
    fn test_get_random_tip_is_not_empty() {
        let tip = get_random_tip();
        // A blank tip is a failure of the "Steel" logic
        assert!(!tip.is_empty(), "Tip should never be an empty string");
    }

    #[test]
    fn test_tip_distribution() {
        // Run it 100 times and store the results
        // We want to see at least SOME variety
        let mut results = std::collections::HashSet::new();
        for _ in 0..50 {
            results.insert(get_random_tip());
        }

        // If we have 40 tips, and we pull 50 times,
        // we should definitely have more than 1 unique tip.
        assert!(results.len() > 1, "Entropy failure: got the same tip 50 times in a row");
    }

    #[test]
    fn test_no_panic_on_rapid_calls() {
        // Stress test the stack entropy/time jitter
        for _ in 0..1000 {
            let _ = get_random_tip();
            // If this doesn't panic, the modulo math is solid
        }
    }
}

#[cfg(test)]
mod tip_tests {
    use alias_lib::random_tip_show;

    #[test]
    fn test_random_tip_show_frequency_bounds() {
        let mut hits = 0;
        let iterations = 2000;

        for _ in 0..iterations {
            if random_tip_show().is_some() {
                hits += 1;
            }
        }

        // 10% of 2000 is 200.
        // We allow a "Rugged" variance of +/- 5% of total iterations (100).
        // If hits are between 100 and 300, the entropy is healthy.
        assert!(hits > 100 && hits < 300,
                "Tip frequency out of range: got {} hits out of {}", hits, iterations);
    }

    #[test]
    fn test_random_tip_show_integrity() {
        // Ensure that when it DOES return Some, the string isn't empty
        for _ in 0..500 {
            if let Some(tip) = random_tip_show() {
                assert!(!tip.is_empty(), "Random show returned an empty string tip");
            }
        }
    }
}

#[cfg(test)]
mod battery_8 {
    use alias_lib::{mesh_logic, render_diagnostics, DiagnosticReport, RegistryStatus, Verbosity};

    #[test]
    fn test_render_diagnostics_resilience() {
        use std::path::PathBuf;

        // Test Case: The "Total Failure" Scenario
        let report_fail = DiagnosticReport {
            binary_path: None,
            resolved_path: PathBuf::from("Z:\\missing.doskey"),
            env_file: "NOT_SET".into(),
            env_opts: "NOT_SET".into(),
            file_exists: false,
            is_readonly: false,
            drive_responsive: false,
            registry_status: RegistryStatus::NotFound,
            api_status: Some("FAILED".into()),
        };

        // Test Case: The "Healthy" Scenario
        let report_ok = DiagnosticReport {
            binary_path: Some(PathBuf::from("C:\\bin\\alias.exe")),
            resolved_path: PathBuf::from("C:\\user\\.aliases"),
            env_file: "C:\\user\\.aliases".into(),
            env_opts: "--quiet".into(),
            file_exists: true,
            is_readonly: false,
            drive_responsive: true,
            registry_status: RegistryStatus::Synced,
            api_status: Some("CONNECTED (Win32 API)".into()),
        };

        // We use Verbosity::silent() to ensure the test doesn't clutter the CI output,
        // but it still executes every line of your rendering logic.
        let v = Verbosity::silent();

        // Act & Assert (No Panic)
        render_diagnostics(report_fail, &v);
        render_diagnostics(report_ok, &v);
    }

    #[test]
    fn test_render_diagnostics_output_logic() {
        use std::sync::{Arc, Mutex};

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let spy = Arc::clone(&buffer);

        // Create a block to limit the lifetime of 'v'
        {
            let v = Verbosity {
                level: alias_lib::VerbosityLevel::Loud,
                show_icons: alias_lib::ShowIcons::On,
                show_tips: alias_lib::ShowTips::Off,
                display_tip: None,
                in_startup: false,
                in_setup: false,
                writer: Some(buffer), // buffer is moved into v here
            };

            let report = alias_lib::DiagnosticReport {
                binary_path: Some(std::path::PathBuf::from("alias.exe")),
                resolved_path: std::path::PathBuf::from("C:\\test.doskey"),
                env_file: "TEST_FILE".into(),
                env_opts: "--temp".into(),
                file_exists: true,
                is_readonly: false,
                drive_responsive: true,
                registry_status: alias_lib::RegistryStatus::Synced,
                api_status: Some("CONNECTED".into()),
            };

            alias_lib::render_diagnostics(report, &v);
        } // <--- 'v' is dropped here! This is the magic moment.

        // Now lock the spy
        let lock = spy.lock().unwrap();
        let output = String::from_utf8_lossy(&*lock);

        assert!(output.contains("WRITABLE"), "Output was: {}", output);
        assert!(output.contains("RESPONSIVE"));
        assert!(output.contains("SYNCED"));
        assert!(output.contains("--temp"));
    }

    #[test]
    fn test_mesh_order_preservation() {
        let os = vec![("a".into(), "val".into()), ("z".into(), "val".into())];
        let file = vec![("z".into(), "val".into()), ("a".into(), "val".into())]; // Z is first here

        let result = mesh_logic(os, file);

        assert_eq!(result[0].name, "z"); // Z should stay first
        assert_eq!(result[1].name, "a");
    }

    #[test]
    fn test_mesh_consistency_and_order() {
        let os = vec![
            ("git".into(), "git status".into()),   // In both
            ("temp".into(), "ls -la".into()),      // Only in RAM (--temp)
        ];
        let file = vec![
            ("ls".into(), "ls --color".into()),    // Only in File
            ("git".into(), "git status".into()),   // In both
        ];

        let result = mesh_logic(os, file);

        // Assert Consistency: Total unique aliases should be 3
        assert_eq!(result.len(), 3);

        // Assert Order: File order first, then RAM ghosts
        assert_eq!(result[0].name, "ls");   // From File (First)
        assert_eq!(result[1].name, "git");  // From File (Second)
        assert_eq!(result[2].name, "temp"); // From RAM (Appended)

        // Assert Values
        assert!(result[0].os_value.is_none());
        assert!(result[2].file_value.is_none());
    }
}

#[cfg(test)]
mod battery_9 {
    use std::fs;
    use tempfile::tempdir;
    use alias_lib::{is_path_healthy, MAX_ALIAS_FILE_SIZE};
    // Requires tempfile crate for "Steel" testing

    #[test]
    fn test_is_path_healthy() {
        let dir = tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("aliases.doskey");
        let folder_path = dir.path().join("subfolder");

        // 1. Test: Path does not exist
        assert!(!is_path_healthy(&file_path,MAX_ALIAS_FILE_SIZE), "Missing file should be unhealthy");

        // 2. Test: Path is a valid file
        fs::write(&file_path, "g=git status").expect("Failed to write test file");
        assert!(is_path_healthy(&file_path,MAX_ALIAS_FILE_SIZE), "Existing file should be healthy");

        // 3. Test: Path is a directory (The "Fake-out" scenario)
        fs::create_dir(&folder_path).expect("Failed to create test dir");
        assert!(!is_path_healthy(&folder_path,MAX_ALIAS_FILE_SIZE), "A directory should be unhealthy");
    }
}

#[cfg(test)]
mod battery_10 {
    use super::*;

    #[test]
    fn test_update_disk_file_atomic() {
        let dir = tempdir().expect("dir");
        let file_path = dir.path().join("aliases.doskey");

        // Write initial state
        fs::write(&file_path, "g=git status").unwrap();

        // Perform an update
        let v = Verbosity::silent();
        update_disk_file(&v, "g", "git st", &file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("g=git st"), "Update should be reflected");
    }

    #[test]
    fn test_editor_resolution() {
        // We test the logic that picks the string,
        // rather than the Command::new itself.
        let override_ed = Some("code".to_string());
        let ed = override_ed
            .or_else(|| std::env::var("VISUAL").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "notepad".to_string());

        assert_eq!(ed, "code");

        let no_override: Option<String> = None;
        // Mocking env var behavior in a controlled scope
        let ed_fallback = no_override.unwrap_or_else(|| "notepad".to_string());
        assert_eq!(ed_fallback, "notepad");
    }

    #[test]
    fn test_triple_audit_orphan_corruption() {
        let win32 = vec![];
        let doskey = vec![];
        let file = vec![("bad name".into(), "val".into())];

        // This should fall all the way to the Pending loop and still flag "CORRUPT"
        perform_triple_audit(&Verbosity::silent(), win32, doskey, file, &MockProvider::provider_type());
    }
}
#[cfg(test)]
mod battery_11 {
    use super::*;

    #[test]
    fn test_parse_macro_file_integrity() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = "
; This is a comment
g=git status
multi=git log --format=%s=%h
  trim_key  =  keep_value_spaces
bad name=should_fail
";
        write!(tmp_file, "{}", content).unwrap();

        let verbosity = Verbosity::silent();
        let result = parse_macro_file(tmp_file.path(), &verbosity).unwrap();

        // 1. Check count: Should be 3 (g, multi, trim_key)
        // 'bad name' fails is_valid_name, ';' is a comment
        assert_eq!(result.len(), 3);

        // 2. Check First-Equals-Sign rule (The "multi" entry)
        assert_eq!(result[1].0, "multi");
        assert_eq!(result[1].1, "git log --format=%s=%h");

        // 3. Check Selective Trimming
        assert_eq!(result[2].0, "trim_key");
        // Note: The line.trim() in your code handles the outer edges,
        // but the value's internal leading spaces should remain if they weren't trimmed.
        assert_eq!(result[2].1, "  keep_value_spaces");
    }

    #[test]
    fn test_parse_non_existent_file() {
        let path = std::path::Path::new("missing_file_xyz.doskey");
        let result = parse_macro_file(path, &Verbosity::silent());

        // ALIGNMENT: We expect Ok(empty), not an Err.
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}

#[cfg(test)]
mod battery_12 {
    use super::*;

    #[test]
    fn test_display_audit_scenarios() {
        // Scenario 1: Perfect Match
        let mesh_ok = vec![AliasEntryMesh {
            name: "g".into(),
            os_value: Some("git status".into()),
            file_value: Some("git status".into()),
        }];

        // Scenario 2: Value Desync
        let mesh_desync = vec![AliasEntryMesh {
            name: "g".into(),
            os_value: Some("git status".into()),
            file_value: Some("git st".into()), // Different
        }];

        // Scenario 3: Corrupt Name
        let mesh_corrupt = vec![AliasEntryMesh {
            name: "bad name".into(), // Spaces are illegal
            os_value: Some("val".into()),
            file_value: Some("val".into()),
        }];

        // In your test, you can run these through display_audit.
        // Since it's a 'void' return, you are verifying it doesn't panic
        // and handles the Option types correctly.
        display_audit(&mesh_ok, &Verbosity::silent(), &MockProvider::provider_type());
        display_audit(&mesh_desync, &Verbosity::silent(), &MockProvider::provider_type());
        display_audit(&mesh_corrupt, &Verbosity::silent(), &MockProvider::provider_type());
    }
}

#[cfg(test)]
mod battery_13 {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use alias_lib::Verbosity;

    #[test]
    fn test_dump_alias_file_logic() {
        let mut tmp_file = NamedTempFile::new().expect("Failed to create temp file");
        let content = "
; Header Comment
g=git status
  spaced_key  =  spaced_value
invalid_line_no_equals
";
        write!(tmp_file, "{}", content).expect("Failed to write to temp file");

        // Note: In a real test environment, you'd need to mock get_alias_path()
        // or ensure the env var ALIAS_FILE points to tmp_file.path().

        let _verbosity = Verbosity::silent();

        // Let's test the inner iterator logic directly if dump_alias_file
        // is hard-coded to a path, or ensure the path exists:
        let lines = content.lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with(';'))
            .filter_map(|line| line.split_once('=').map(|(n, v)| (n.to_string(), v.to_string())))
            .collect::<Vec<_>>();

        // 1. Should ignore the comment and the empty lines
        // 2. Should ignore 'invalid_line_no_equals' (no split_once match)
        assert_eq!(lines.len(), 2);

        // 3. Verify 'spaced_key' preservation
        // Your logic uses line.starts_with(';') but NOT line.trim().starts_with(';')
        // This is a "Hazard": If a user has a space before a semicolon, it won't be ignored!
        assert_eq!(lines[0].0, "g");
        assert_eq!(lines[1].0, "  spaced_key  ");
    }
}

#[cfg(test)]
mod battery_14 {
    use tempfile::NamedTempFile;
    use std::io::Write;
    use alias_lib::{query_alias_file, Verbosity};

    #[test]
    fn test_query_alias_file_precision() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let content = "
;ls=dir_old
ls=dir_new
  g=git status
";
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path();
        let v = Verbosity::silent();

        // 1. Test: Ignore comments
        // If query finds ';ls=', that's a fail. It should find 'ls='
        let res = query_alias_file("ls", path, &v).unwrap();
        assert!(res[0].contains("dir_new"));
        assert!(!res[0].starts_with(';'));

        // 2. Test: Case Insensitivity
        let res_caps = query_alias_file("LS", path, &v).unwrap();
        assert!(res_caps[0].contains("dir_new"));

        // 3. Test: Missing Alias
        let res_none = query_alias_file("xyz", path, &v).unwrap();
        // In silent mode, results should be empty based on your logic
        assert!(res_none.is_empty());
    }
}

#[cfg(test)]
mod battery_15 {
    use alias_lib::run;
    use super::*;

    #[test]
    fn test_run_hydration_logic() {
        global_test_setup();
        // Test 1: Startup only
        let args = vec!["alias.exe".into(), "--startup".into()];
        let result = run::<MockProvider>(args);
        assert!(result.is_ok(), "Startup should succeed even with empty queue");

        // Test 2: ENV Injection
        // We can't easily mock env::var in a standard test,
        // but we can test the splice logic if it were moved to a helper.
    }

    #[test]
    fn test_empty_args_defaults_to_show_all() {
        let args = vec!["alias.exe".into()];
        // If queue is empty, Step 5 should push ShowAll.
        // We verify this by ensuring the runner doesn't just exit early.
        let result = run::<MockProvider>(args);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod battery_16 {
    use serial_test::serial;
    use alias_lib::run;
    use super::*;

    // A fake provider to track what the 'run' function tries to do
    #[test]
    fn test_run_startup_short_circuit() {
        // Scenario: alias --startup (and nothing else)
        // Expected: Should run Reload and return Ok without pushing ShowAll
        let args = vec!["alias.exe".to_string(), "--startup".to_string()];

        let result = run::<MockProvider>(args);
        assert!(result.is_ok(), "Startup flow should exit cleanly");
    }

    #[test]
    fn test_run_default_to_show_all() {
        // Scenario: alias (no args)
        // Expected: Queue is empty, should default to ShowAll
        let args = vec!["alias.exe".to_string()];

        let result = run::<MockProvider>(args);
        assert!(result.is_ok(), "Empty args should trigger ShowAll fallback");
    }

    #[test]
    #[serial]
    fn test_run_with_malformed_env_opts() {
        // Even if ALIAS_OPTS is garbage, run should survive.
        unsafe {
            std::env::set_var("ALIAS_OPTS", "--invalid-flag --quiet");
        }
        let args = vec!["alias.exe".to_string(), "g=git status".to_string()];

        let result = run::<MockProvider>(args);
        assert!(result.is_ok(), "Run should filter bad env opts and continue");
        unsafe {
            std::env::remove_var("ALIAS_OPTS");
        }
    }
}

#[cfg(test)]
mod battery_17 {
    use alias_lib::{dispatch, AliasAction, Task, Verbosity};
    use super::*;

    #[test]
    fn test_dispatch_unalias_formatting() {
        let file_path = std::path::PathBuf::from("test.doskey"); // Use PathBuf for Task
        let verbosity = Verbosity::silent();

        // The raw action remains the same
        let action = AliasAction::Unalias(SetOptions::volatile("  g=something  ".to_string(), false ));

        // 1. Wrap them into the Task struct
        let task = Task {
            action,
            path: file_path, // Task owns this path now
        };

        // 2. Dispatch now takes exactly TWO arguments: (Task, &Verbosity)
        assert!(dispatch::<MockProvider>(task, &verbosity).is_ok());

        // 3. Verify the MockProvider captured the cleaned result
        let captured = LAST_CALL.lock().unwrap().take().expect("Mock should have been called");
        assert_eq!(captured.name, "g=something");
        assert_eq!(captured.value, "");
    }

    #[test]
    fn test_dispatch_invalid_name_error() {
        let file_path = std::path::PathBuf::from("test.doskey");

        // An empty or whitespace-only string should fail validation
        let action = AliasAction::Unalias(SetOptions::volatile("   ".to_string(), false));

        // 1. Wrap it in a Task
        let task = Task {
            action,
            path: file_path,
        };

        // 2. Pass Task and &Verbosity
        let result = dispatch::<MockProvider>(task, &Verbosity::silent());

        // 3. Verify it failed
        assert!(result.is_err(), "Should return an error for empty names");
    }
}
#[cfg(test)]
mod battery_18 {
    use alias_lib::{parse_arguments, AliasAction};

    #[test]
    fn test_parse_complex_intent() {
        // Input: alias --temp --file my.txt g = git status
        let args = vec![
            "alias.exe".into(),
            "--temp".into(),
            "--file".into(), "my.txt".into(),
            "g".into(), "=".into(), "git".into(), "status".into()
        ];

        // FIX 1: Correct order (TaskQueue, Verbosity)
        let (mut queue, _voice) = parse_arguments(&args);

        // FIX 2: Pull the first task out to inspect it
        let mut task = queue.pull().expect("Should have parsed at least one task");

        // FIX 3: Check the path on the TASK, not as a standalone variable
        assert_eq!(task.path.to_str().unwrap(), "my.txt");

        task = queue.pull().expect("Should have parsed at least one task");

        // FIX 4: Check the action inside that same task
        if let AliasAction::Set(opts) = task.action {
            assert_eq!(opts.name, "g");
            assert_eq!(opts.value, "git status");
            assert!(opts.volatile); // --temp was passed
        } else {
            panic!("Expected Set action, found {:?}", task.action);
        }
    }

    #[test]
    fn test_parse_query_fallback() {
        // Input: alias --quiet my_alias
        let args = vec!["alias.exe".into(), "--quiet".into(), "my_alias".into()];

        // FIX: Swap the names so 'queue' is the TaskQueue
        let (queue, _voice) = parse_arguments(&args);

        // Now 'queue' is a TaskQueue, so .get(0) exists!
        match &queue.get(0).expect("Queue should have 1 task").action {
            AliasAction::Query(name) => assert_eq!(name, "my_alias"),
            _ => panic!("Expected Query action"),
        }
    }
}

#[cfg(test)]
mod battery_19 {
    use alias_lib::{get_alias_path, parse_arguments, AliasAction, VerbosityLevel};

    #[test]
    fn test_parser_pivot_with_ugly_spacing() {
        // Input: alias --temp   g   =   "git status"
        let args = vec![
            "alias".into(), "--temp".into(),
            "g".into(), "=".into(), "\"git status\"".into()
        ];

        // FIX: queue must be first to receive the TaskQueue
        let (queue, _) = parse_arguments(&args);

        // FIX: Access the internal tasks vector
        if let AliasAction::Set(opts) = &queue.tasks[0].action {
            assert_eq!(opts.name, "g");
            assert_eq!(opts.value, "\"git status\"");
            assert!(opts.volatile);
        } else {
            panic!("Failed to pivot on spaced-out assignment");
        }
    }

    #[test]
    fn test_parser_ignores_garbage_flags() {
        // Input: alias --not-a-real-flag --quiet g=ls
        let args = vec!["alias".into(), "--not-a-real-flag".into(), "--quiet".into(), "g=ls".into()];

        // 1. Unpack correctly: (TaskQueue, Verbosity)
        let (mut queue, voice) = parse_arguments(&args);

        // 2. Verify the Verbosity (Voice)
        assert_eq!(voice.level, VerbosityLevel::Silent);

        // 3. Use .pull() to get the Task.
        // This avoids the "cannot index" error if TaskQueue doesn't implement Index.
        // the bad flag
        let garbage = queue.pull().expect("Should have the invalid flag task");
        assert!(matches!(garbage.action, AliasAction::Invalid), "First task should be Invalid");
        // and the set
        let task = queue.pull().expect("Should have recovered from bad flag to process the payload");

        if let AliasAction::Set(opts) = task.action {
            assert_eq!(opts.name, "g");
            assert_eq!(opts.value, "ls");
        } else {
            panic!("Expected a Set action, but got: {:?}", task.action);
        }
    }
    #[test]
    fn test_parser_illegal_name_detection() {
        // Input: alias "bad name"=value
        let args = vec!["alias".into(), "bad name=value".into()];

        // FIX: Swap the order so 'queue' is the TaskQueue (the first element)
        // and 'voice' (or _) is the Verbosity (the second element).
        let (queue, _voice) = parse_arguments(&args);

        // Now 'queue' refers to the TaskQueue, which has the .tasks field
        // or an iterator implementation.
        assert!(queue.tasks.iter().any(|t| matches!(t.action, AliasAction::Invalid)));
    }
    #[test]
    fn test_parser_file_flag_missing_path() {
        let args = vec!["alias".into(), "--file".into()];
        let (mut queue, _voice) = parse_arguments(&args);

        // 1. The queue IS NOT empty. It contains the record of the failure.
        let task = queue.pull().expect("Should have a task describing the failure");

        // 2. The REALITY: The action is Invalid.
        assert!(
            matches!(task.action, AliasAction::Invalid),
            "Reality Check: Naked --file must result in an Invalid action"
        );

        // 3. The FALLBACK: The queue context itself should have reverted
        // to whatever the environment/system considers 'Home'.
        let default_pathbuf = get_alias_path("").unwrap();
        let expected_default = default_pathbuf.to_string_lossy();

        assert_eq!(
            queue.getpath(),
            expected_default,
            "Queue should fall back to default system path when flag parsing fails"
        );
    }
}
#[cfg(test)]
mod battery_20 {
    use alias_lib::parse_alias_line;

    #[test]
    fn test_parse_alias_line_comprehensive() {
        let cases = vec![
            // 1. Simple case (Still works)
            ("rust=cargo $*", Some(("rust", "cargo $*"))),

            // 2. Quoted LHS (FIXED: Now we EXPECT the quotes to stay)
            ("\"my alias\"=echo hello", Some(("\"my alias\"", "echo hello"))),

            // 3. The "XCD" (Still works, and now we know WHY it works)
            ("xcd=cd /d \"%i\"", Some(("xcd", "cd /d \"%i\""))),

            // 4. Kanji (Still works)
            ("エイリアス=echo kanji", Some(("エイリアス", "echo kanji"))),

            // 5. Whitespace (Still works because we still .trim() whitespace, just not quotes)
            ("  clean \u{00A0}=  value with space  ", Some(("clean", "value with space"))),

            // ...

            // 10. Unbalanced LHS quote (FIXED: Expect the literal quote)
            ("\"unbalanced=value", Some(("\"unbalanced", "value"))),
        ];
        for (input, expected) in cases {
            let result = parse_alias_line(input);

            match (result, expected) {
                (Some((n, v)), Some((en, ev))) => {
                    assert_eq!(n, en, "Name mismatch on input: {}", input);
                    assert_eq!(v, ev, "Value mismatch on input: {}", input);
                }
                (None, None) => {} // Correctly failed
                (r, e) => panic!("Failed test case '{}'. Got {:?}, expected {:?}", input, r, e),
            }
        }
    }
}

#[cfg(test)]
mod intelligence_tests {
    //        use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use alias_lib::{ext_with_dot, identify_binary, is_script_extension, peek_pe_metadata, BinarySubsystem, Verbosity};

    #[test]
    fn test_ext_with_dot_normalization() {
        assert_eq!(ext_with_dot(std::ffi::OsStr::new("exe")), ".exe");
        assert_eq!(ext_with_dot(std::ffi::OsStr::new(".BAT")), ".bat"); // Case & Dot check
        assert_eq!(ext_with_dot(std::ffi::OsStr::new("")), ".");
    }

    #[test]
    fn test_is_script_extension() {
        assert!(is_script_extension(".bat"));
        assert!(is_script_extension(".cmd"));
        assert!(is_script_extension(".vbs"));
        assert!(!is_script_extension(".exe")); // Binary, not script
        assert!(!is_script_extension(".txt")); // Text, not executable script
    }

    #[test]
    fn test_identify_binary_script_triage() {
        let dir = tempdir().unwrap();
        let script_path = dir.path().join("test.bat");
        File::create(&script_path).unwrap();

        let verbosity = Verbosity::mute();
        let profile = identify_binary(&verbosity, &script_path).unwrap();

        assert!(matches!(profile.subsystem, BinarySubsystem::Script));
        assert_eq!(profile.exe, script_path);
    }

    #[test]
    fn test_peek_pe_metadata_invalid_files() {
        let dir = tempdir().unwrap();
        let txt_path = dir.path().join("fake.exe");
        let mut f = File::create(&txt_path).unwrap();
        f.write_all(b"Not an MZ header").unwrap();

        // Should fail because it doesn't start with "MZ"
        let result = peek_pe_metadata(&txt_path);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod intent_tests {
    //        use super::*;
    use std::env;
    use alias_lib::{find_executable, get_editor_preference, Verbosity};

    #[test]
    fn test_get_editor_preference_override() {
        let verbosity = Verbosity::mute();
        let override_cmd = Some("code --wait".to_string());

        let profile = get_editor_preference(&verbosity, &override_cmd);

        // Should split arguments correctly
        assert_eq!(profile.args[0], "code");
        assert_eq!(profile.args[1], "--wait");
    }

    #[test]
    fn test_get_editor_preference_env_priority() {
        let verbosity = Verbosity::mute();

        // Set environment
        unsafe {
            env::set_var("VISUAL", "vim");
            env::set_var("EDITOR", "nano");
        }

        let profile = get_editor_preference(&verbosity, &None);

        // VISUAL has priority over EDITOR
        assert_eq!(profile.args[0], "vim");

        unsafe {
            env::remove_var("VISUAL");
        }
        let profile_2 = get_editor_preference(&verbosity, &None);
        assert_eq!(profile_2.args[0], "nano");
    }

    #[test]
    fn test_find_executable_path_logic() {
        // This test assumes 'notepad.exe' exists in C:\Windows\System32
        // which is standard for Windows environments.
        let result = find_executable("notepad");
        assert!(result.is_some());
        assert!(result.unwrap().to_string_lossy().to_lowercase().contains("notepad.exe"));
    }
}

#[cfg(test)]
mod integration_tests {
    //        use super::*;
    use std::path::PathBuf;
    use alias_lib::{open_editor, BinaryProfile, BinarySubsystem, Verbosity};

    #[test]
    fn test_open_editor_inaccessible_target() {
        let verbosity = Verbosity::mute();
        let non_existent = PathBuf::from("Z:\\this\\does\\not\\exist.txt");

        let result = open_editor(&non_existent, None, &verbosity);

        // Should return an error because the file doesn't exist
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Target file inaccessible.");
    }

    #[test]
    fn test_binary_profile_fallback() {
        let profile = BinaryProfile::fallback("dummy.exe");
        assert_eq!(profile.exe, PathBuf::from("dummy.exe"));
        assert!(matches!(profile.subsystem, BinarySubsystem::Cui));
        assert_eq!(profile.is_32bit, false);
    }
}

#[cfg(test)]
mod intent_and_symmetry_tests {
    use std::str::FromStr;
    use alias_lib::{dispatch, is_valid_name, parse_arguments, AliasAction, Verbosity};
    use super::*;

    #[test]
    fn test_alias_action_symmetry_round_trip() {
        let cases = vec![
            AliasAction::Icons,
            AliasAction::NoIcons,
            AliasAction::Tips,
            AliasAction::NoTips,
            AliasAction::Help,
            AliasAction::Reload,
            AliasAction::Setup,
            AliasAction::Clear,
            AliasAction::Which,
            AliasAction::File,
            AliasAction::Quiet,
            AliasAction::Temp,
            AliasAction::Case,
            AliasAction::NoCase,
            AliasAction::Startup,
            AliasAction::ShowAll,
            AliasAction::Edit(Some("my_alias".to_string())),
            AliasAction::Edit(None),
            AliasAction::Unalias(SetOptions::volatile("target".to_string(), false)),
            AliasAction::Remove(SetOptions::involatile("old_cmd".to_string(), false)),
            AliasAction::Unalias(SetOptions::volatile("find_me".to_string(), false)),
            AliasAction::Toggle(Box::new(AliasAction::Query("icons".to_string())), true),
        ];

        for action in cases {
            let serialized = action.to_string();
            let deserialized = AliasAction::from_str(&serialized).expect("Round trip failed to parse");
            assert_eq!(action, deserialized, "Symmetry break: {:?} != {:?}", action, deserialized);
        }
    }

    #[test]
    fn test_semantic_intent_categorization() {
        let test_matrix = vec![
            // Input string      // Expected Variant type
            ("--help", "Help"),
            ("--HELP", "Help"), // Case-insensitivity check
            ("--no-icons", "NoIcons"),
            ("--unalias", "Unalias"),
            ("my-alias", "Query"), // Standard name
            ("complex_name", "Query"),
            ("--unknown-flag", "Invalid"),
            ("--no-help", "Invalid"), // Negating a non-toggleable flag
        ];

        for (input, expected_type) in test_matrix {
            let intent = AliasAction::intent(input);
            match expected_type {
                "Help" => assert!(matches!(intent, AliasAction::Help)),
                "NoIcons" => assert!(matches!(intent, AliasAction::NoIcons)),
                "Unalias" => assert!(matches!(intent, AliasAction::Unalias(_))),
                "Query" => assert!(matches!(intent, AliasAction::Query(_))),
                "Invalid" => assert!(matches!(intent, AliasAction::Invalid)),
                _ => panic!("Unknown expected type"),
            }
        }
    }

    #[test]
    fn test_alias_validation_and_toggle_edges() {
        // 1. Internal Toggle Parsing (The colon-split logic)
        let toggle_str = "__internal_toggle=feature_x:true";
        let action = AliasAction::from_str(toggle_str).unwrap();
        if let AliasAction::Toggle(inner, state) = action {
            assert_eq!(state, true);
            if let AliasAction::Query(name) = *inner {
                assert_eq!(name, "feature_x");
            } else {
                panic!("Toggle inner should be a Query");
            }
        }

        // 2. Bad Toggle formats
        assert!(matches!(AliasAction::from_str("__internal_toggle=no_colon"), Ok(AliasAction::Invalid)));

        // 3. Name Validation (Gatekeeper)
        assert!(is_valid_name("valid_name"));
        assert!(is_valid_name("name-with-hyphen"));
        assert!(!is_valid_name(""), "Empty name should be invalid");
        assert!(!is_valid_name("name with spaces"), "Spaces should be invalid");
        assert!(!is_valid_name("=leading_eq"), "Equals sign is a reserved delimiter");
    }

    #[test]
    fn test_structural_traits() {
        let a = AliasAction::Query("test".to_string());
        let b = a.clone();
        assert_eq!(a, b); // Partial_Eq check

        // Ensure nested Boxes are also cloned correctly
        let t1 = AliasAction::Toggle(Box::new(a), true);
        let t2 = t1.clone();
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_alias_action_symmetry_matrix() {
        let scenarios = vec![
            // Simple flags
            AliasAction::Help,
            AliasAction::Setup,
            AliasAction::Reload,
            // Parameterized
            AliasAction::Unalias(SetOptions::volatile("test_cmd".to_string(), false)),
            AliasAction::Remove(SetOptions::involatile("garbage_collect".to_string(), false)),
            AliasAction::Edit(Some("vim_profile".to_string())),
            AliasAction::Edit(None),
            // Internal/Logic
            AliasAction::Toggle(Box::new(AliasAction::Query("voice".to_string())), true),
            AliasAction::Toggle(Box::new(AliasAction::Query("icons".to_string())), false),
            AliasAction::Query("search_term".to_string()),
        ];

        for action in scenarios {
            let serialized = action.to_string();
            let deserialized = AliasAction::from_str(&serialized)
                .expect(&format!("Failed to parse serialized action: {}", serialized));

            assert_eq!(action, deserialized, "Mismatch after round-trip! \nOriginal: {:?} \nSerialized: {} \nResult: {:?}", action, serialized, deserialized);
        }
    }

    #[test]
    fn test_intent_categorization_and_negation_safety() {
        let matrix = vec![
            // Input string            // Expected Variant
            ("--icons",                AliasAction::Icons),
            ("--no-icons",             AliasAction::NoIcons),
            ("--tips",                 AliasAction::Tips),
            ("--no-tips",              AliasAction::NoTips),
            // Verification: Do not strip "no-" from valid command names
            ("no-limit",               AliasAction::Query("no-limit".to_string())),
            ("--no-help",              AliasAction::Invalid), // Help can't be negated
            ("--unalias",              AliasAction::Unalias(SetOptions::volatile(String::new(), false))),
        ];

        for (input, expected) in matrix {
            let result = AliasAction::intent(input);
            // We check discriminant match since data inside Query/Unalias might vary
            assert_eq!(std::mem::discriminant(&result), std::mem::discriminant(&expected),
                       "Intent mismatch for input: {}", input);
        }
    }

    #[test]
    fn test_internal_toggle_parsing_integrity() {
        let valid_toggle = "__internal_toggle=icons:true";
        let action = AliasAction::from_str(valid_toggle).unwrap();

        if let AliasAction::Toggle(inner, state) = action {
            assert!(state == true);
            if let AliasAction::Query(name) = *inner {
                assert_eq!(name, "icons");
            } else {
                panic!("Toggle inner should have been a Query variant");
            }
        } else {
            panic!("Failed to parse as Toggle variant");
        }

        // Negative case: Missing colon
        let bad_toggle = "__internal_toggle=icons_true";
        assert!(matches!(AliasAction::from_str(bad_toggle), Ok(AliasAction::Invalid)));
    }

    #[test]
    fn test_alias_name_gatekeeper_edges() {
        let valid = ["git-commit", "build_22", "test123", "normal"];
        let invalid = ["CON", "PRN", "AUX", "name with space", "cmd=val", ""];

        for name in valid {
            assert!(is_valid_name(name), "Should be valid: {}", name);
        }

        for name in invalid {
            assert!(!is_valid_name(name), "Should be invalid: {}", name);
        }
    }

    #[test]
    fn t76_case_persistence_dispatch() {
        // Scenario: alias --case g=ls
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--case".into(), "g=ls".into()]);
        let task = q.pull().expect("Should have one task");

        // Dispatch the task to our Mock
        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        let r = LAST_CALL.lock().unwrap().take().expect("Capture failed");
        assert_eq!(r.name, "g");
        assert!(r.force_case); // This verifies the rename is functional
    }
}

#[cfg(test)]
mod integrity_tests {
    use super::*;

    #[test]
    fn test_integrity_missing_file_on_live_drive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.doskey");

        // Should be Missing, not Unresponsive
        let state = check_path_integrity(&path);
        assert!(matches!(state, PathIntegrity::Missing));

        // Tool A should allow it (for creation/empty read)
        assert!(is_file_accessible(&path));
    }

    #[serial]
    #[test]
    fn test_integrity_healthy_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("healthy.doskey");
        File::create(&path).unwrap();

        let state = check_path_integrity(&path);
        assert!(matches!(state, PathIntegrity::Healthy));
        assert!(is_file_accessible(&path));
    }

    #[test]
    fn test_integrity_locked_file() {
        use std::os::windows::fs::OpenOptionsExt; // Needed for .share_mode()

        let dir = tempdir().unwrap();
        let path = dir.path().join("locked.txt");

        // Create the file
        std::fs::File::create(&path).unwrap();

        // Open with share_mode(0) -> Deny Read, Deny Write, Deny Delete
        // This creates a hard Sharing Violation for any subsequent access
        let _blocker = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&path)
            .unwrap();

        let state = check_path_integrity(&path);

        // Now is_file_accessible will hit Err(32) and return false
        assert!(matches!(state, PathIntegrity::Unresponsive));
    }

    #[test]
    fn test_timeout_guard_triggers_properly() {
        let timeout = Duration::from_millis(10);
        let result = timeout_guard(timeout, || {
            std::thread::sleep(Duration::from_millis(100));
            true
        });

        assert!(result.is_none(), "Guard should have timed out");
    }

    #[test]
    #[serial] // Use serial_test to avoid env collision between threads
    fn test_path_resolution_cycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let real_file = temp_dir.path().join("my.aliases");
        std::fs::File::create(&real_file).unwrap();

        // --- Scenario 1: Explicit CLI Override ---
        let res = get_alias_path(real_file.to_str().unwrap());
        assert_eq!(res, Some(real_file.clone()), "CLI override should win");

        // --- Scenario 2: Environment Variable (File) ---
        unsafe {
            std::env::set_var("ALIAS_FILE", real_file.to_str().unwrap());
        }
        let res = get_alias_path("");
        assert_eq!(res, Some(real_file.clone()), "ENV var file should win over defaults");

        // --- Scenario 3: Environment Variable (Dir) ---
        // Should join with DEFAULT_ALIAS_FILENAME
        unsafe {
            std::env::set_var("ALIAS_FILE", temp_dir.path().to_str().unwrap());
        }
        let res = get_alias_path("");
        let expected = temp_dir.path().join("aliases.doskey"); // Assuming this is your default
        // Note: This might fail if the file doesn't exist yet and is_viable_path checks existence
        assert_eq!(res, Some(expected), "Should join ALIAS_FILE directory with default filename");
        assert!(res.is_some(), "Should resolve even if file doesn't exist yet");

        // --- Scenario 4: The Empty String (The Discovery) ---
        unsafe {
            std::env::remove_var("ALIAS_FILE");
        }
        let res = get_alias_path("");
        // This will test your APPDATA/USERPROFILE fallback
        // If this is None in your audit, it means none of the standard paths have a viable parent
        assert!(res.is_some(), "Standard OS discovery should return a default path when ENV is empty");
    }
}


#[cfg(test)]
mod tips_test {
    use super::*;

    #[test]
    fn test_verbosity_random_tips_statistical_distribution() {
        let iterations = 1000;
        let mut tip_count = 0;

        for _ in 0..iterations {
            let v = Verbosity::normal();

            // Check if a tip was populated
            if v.display_tip.is_some() {
                tip_count += 1;

                // Safety check: ensure it didn't just return an empty string
                assert!(!v.display_tip.unwrap().is_empty(), "Tip should not be empty if present");
            }
        }

        // 10% of 1000 is 100.
        // We use a generous margin (e.g., 50-150) to account for RNG variance
        // while still catching hard-coded "Always On" or "Always Off" bugs.
        println!("Statistical Tip Count: {}/{}", tip_count, iterations);

        assert!(tip_count > 50, "Tip frequency too low: {}/{}", tip_count, iterations);
        assert!(tip_count < 150, "Tip frequency too high: {}/{}", tip_count, iterations);
    }

    #[test]
    fn test_verbosity_presets_logic() {
        // Silent should NEVER have a tip
        let silent = Verbosity::silent();
        assert!(silent.display_tip.is_none(), "Silent mode must never show tips");

        // Loud should ALWAYS have a tip (based on your Loud preset logic)
        // If random_tip_show() itself has the 10% logic, this might still be None
        // unless you force random_tip_show to check the ShowTips enum.
        let loud = Verbosity::loud();
        if loud.show_tips.is_on() {
            // Verification logic here depends on if random_tip_show()
            // respects the 'On' override or stays at 10%.
        }
    }

    #[test]
    fn test_verbosity_normal_tip_distribution() {
        let iterations = 1000;
        let mut tip_count = 0;

        for _ in 0..iterations {
            // Construct a new "Normal" verbosity which rolls for a tip
            let v = Verbosity::normal();

            if v.display_tip.is_some() {
                tip_count += 1;
                // Ruggedness check: ensure it's not a blank string
                assert!(!v.display_tip.unwrap().is_empty(), "Tip should contain text");
            }
        }

        // Checking for 10% distribution (target 100)
        // We allow a margin (50-150) to prevent flakiness while catching 0% or 100% bugs.
        println!("Statistical Tip Count: {}/{}", tip_count, iterations);
        assert!(tip_count >= 50, "Tip frequency too low: {}/{}", tip_count, iterations);
        assert!(tip_count <= 150, "Tip frequency too high: {}/{}", tip_count, iterations);
    }

    #[test]
    fn test_verbosity_preset_hard_limits() {
        // Silent/Mute should NEVER show tips, regardless of RNG
        assert!(Verbosity::silent().display_tip.is_none(), "Silent mode must be tip-free");
        assert!(Verbosity::mute().display_tip.is_none(), "Mute mode must be tip-free");

        // Loud mode should be reliable for debugging
        let loud = Verbosity::loud();
        assert_eq!(loud.show_tips, ShowTips::On, "Loud mode should have tips ON");
    }

    #[test]
    fn test_verbosity_tip_distribution_monte_carlo() {
        let iterations = 1000;
        let mut tip_count = 0;

        for _ in 0..iterations {
            let v = Verbosity::normal();
            if v.display_tip.is_some() {
                tip_count += 1;
                // Ensure tip isn't just an empty string
                assert!(!v.display_tip.unwrap().is_empty());
            }
        }

        // Checking for ~10% (Target 100).
        // Margin of 50-150 prevents "flaky" failures while catching logical errors.
        println!("Statistical Tip Count: {}/{}", tip_count, iterations);
        assert!(tip_count >= 50 && tip_count <= 150,
                "Tip distribution outside 10% tolerance: got {}/{}", tip_count, iterations);
    }
}

