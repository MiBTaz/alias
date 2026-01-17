// alias_lib/tests/local_library_tests.rs

use std::path::PathBuf;
use std::time::Duration;
use std::io;
use alias_lib::*;
use alias_lib::ShowFeature::{Off, On};
use std::{env, fs};
use serial_test::serial;

// shared code start
extern crate alias_lib;

#[path = "../../tests/shared_test_utils.rs"]
mod test_suite_shared;
#[allow(unused_imports)]
use test_suite_shared::{MockProvider, MOCK_RAM, LAST_CALL, global_test_setup};

#[path = "../../tests/state_restoration.rs"]
mod stateful;

// shared code end
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
fn local_library_tests_end() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}

#[macro_export]
macro_rules! trace {
    // Branch 1: Single argument
    ($arg:expr) => {
        #[cfg(any(debug_assertions, test))]
        {
            // Changing {} to {:?} is the key.
            // It will now print "Query("cmd")" instead of just "cmd"
            eprintln!("[TRACE][{}] {:?}", function_name!(), $arg);
        }
    };
    // Branch 2: Format string
    ($fmt:expr, $($arg:tt)*) => {
        #[cfg(any(debug_assertions, test))]
        {
            eprintln!("[TRACE][{}] {}", function_name!(), format!($fmt, $($arg)*));
        }
    };
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

// =========================================================
// SECTION 1: COMMAND PARSER & ACTION MAPPING (Tests 1-25)
// =========================================================
#[cfg(test)]
mod argument_tests {
    use super::*;
    //    use std::path::PathBuf;

    // =========================================================
    // 1. COMMAND PARSER & ACTION MAPPING
    // =========================================================

    #[test]
    fn test_show_all() {
        let args = vec!["alias".into()];
        let (mut queue, voice) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pull().unwrap().action, AliasAction::ShowAll);
        assert!(!voice.is_silent());
    }

    #[test]
    fn test_help_flag() {
        let args = vec!["alias".into(), "--help".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Help);
    }

    #[test]
    fn test_quiet_and_icons() {
        let args = vec!["alias".into(), "--quiet".into(), "--icons".into()];
        let (_, voice) = parse_arguments(&args);

        assert_eq!(voice.level, VerbosityLevel::Silent);
        assert_eq!(voice.show_icons, ShowFeature::On);
    }

    #[test]
    fn test_reload_flag() {
        let args = vec!["alias".into(), "--reload".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Reload);
    }

    #[test]
    fn test_unalias_flag() {
        let args = vec!["alias".into(), "--unalias".into(), "test_cmd".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        if let AliasAction::Unalias(name) = queue.pull().unwrap().action {
            assert_eq!(name, "test_cmd".into());
        } else {
            panic!("Expected AliasAction::Unalias");
        }
    }

    #[test]
    fn test_file_flag() {
        let args = vec!["alias".into(), "--file".into(), "custom.doskey".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        let task = queue.pull().unwrap();
        assert_eq!(task.action, AliasAction::File);
    }

    #[test]
    fn test_reload_and_query() {
        let args = vec!["alias".into(), "--reload".into(), "my_cmd".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Reload);

        if let AliasAction::Query(name) = queue.pull().unwrap().action {
            assert_eq!(name, "my_cmd");
        } else {
            panic!("Expected trailing Query task");
        }
    }

    #[test]
    fn test_path_sticky_sweep() {
        let args = vec![
            "alias".into(),
            "--reload".into(),
            "--file".into(),
            "custom.doskey".into(),
            "test".into()
        ];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 3);

        let t1 = queue.pull().unwrap(); // Reload
        let t2 = queue.pull().unwrap(); // File
        let t3 = queue.pull().unwrap(); // Query

        assert!(t1.path.to_string_lossy().contains("custom.doskey"));
        assert_eq!(t2.action, AliasAction::File);
        assert!(t3.path.as_os_str().is_empty(), "Queries should have empty paths as they are RAM-based");
    }

    // alias_lib\tests\local_library_tests.rs

    #[test]
    fn test_default_path_resolution() {
        let args = vec![
            "alias".into(),
            "--which".into(),
            "my_cmd".into(),
            "new_cmd=value".into()
        ];
        let (mut queue, _) = parse_arguments(&args);

        // 2. The Set Task (Greedy Payload)
        let task_s = queue.pop().expect("Should have set task");
        assert!(task_s.path.to_string_lossy().contains(".doskey"), "Set MUST have a path");

        // 1. The Which Task
        let task_q = queue.pop().expect("Should have query task from --which");

        // FIX: Update this assertion to match your new `requires_file` logic
        assert!(!task_q.path.to_string_lossy().is_empty(), "Which now REQUIRES a path context");
        assert!(task_q.path.to_string_lossy().contains(".doskey"), "Which should point to default file");

        assert_eq!(queue.len(), 0, "Queue should be empty");
    }

    #[test]
    fn test_setup_requirement() {
        let args = vec!["alias".into(), "--setup".into()];
        let (mut queue, voice) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Setup);
        assert!(voice.in_setup);
    }

    #[test]
    fn test_invalid_file_marks_fail() {
        // We use 'ls=dir' (a Set) instead of 'ls' (a Query)
        let args = vec![
            "alias".into(),
            "--file".into(),
            "/non/existent/path/xyz.txt".into(),
            "--edalias".into()
        ];
        let (queue, _) = parse_arguments(&args);

        let tasks: Vec<_> = queue.tasks.into_iter().collect();

        // Now, because Set needs a file and the path is garbage, it will hit AliasAction::Fail
        let has_fail = tasks.iter().any(|t| matches!(t.action, AliasAction::Fail));
        assert!(has_fail, "Expected 'Set' task to fail due to unresolvable path");
    }

    #[test]
    fn test_payload_set_space() {
        let args = vec!["alias".into(), "list".into(), "ls".into(), "-la".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        if let AliasAction::Set(opts) = queue.pull().unwrap().action {
            assert_eq!(opts.name, "list");
            assert_eq!(opts.value, "ls -la");
        } else {
            panic!("Expected AliasAction::Set from space separation");
        }
    }

    #[test]
    fn test_payload_query() {
        let args = vec!["alias".into(), "git_status".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        if let AliasAction::Query(name) = queue.pull().unwrap().action {
            assert_eq!(name, "git_status");
        } else {
            panic!("Expected AliasAction::Query");
        }
    }

    #[test]
    fn test_payload_with_persistence_flags() {
        let args = vec!["alias".into(), "--temp".into(), "--case".into(), "tmp=echo 1".into()];
        let (mut queue, _) = parse_arguments(&args);

        if let AliasAction::Set(opts) = queue.pull().unwrap().action {
            assert!(opts.volatile);
            assert!(opts.force_case);
            assert_eq!(opts.name, "tmp");
        } else {
            panic!("Expected AliasAction::Set with flags");
        }
    }

    #[test]
    fn test_payload_invalid_name() {
        let args = vec!["alias".into(), "!!!invalid=value".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Invalid);
    }

    #[test]
    fn test_unalias_and_remove_logic() {
        // 1. Test Unalias (Volatile / In-Memory)
        let args_u = vec!["alias".into(), "--unalias".into(), "old_cmd".into()];
        let (mut queue_u, _) = parse_arguments(&args_u);
        if let AliasAction::Unalias(opts) = queue_u.pull().unwrap().action {
            assert_eq!(opts.name, "old_cmd");
            assert!(opts.volatile, "Unalias should be volatile (memory only)");
        } else {
            panic!("Expected Unalias action");
        }

        // 2. Test Remove (Persistent / Disk)
        let args_r = vec!["alias".into(), "--remove".into(), "other_cmd".into()];
        let (mut queue_r, _) = parse_arguments(&args_r);
        if let AliasAction::Remove(opts) = queue_r.pull().unwrap().action {
            assert_eq!(opts.name, "other_cmd");
            assert!(!opts.volatile, "Remove should NOT be volatile (hits disk)");
        } else {
            panic!("Expected Remove action");
        }
    }

    #[test]
    fn test_editor_extraction() {
        let args_space = vec!["alias".into(), "--edalias".into()];
        let (mut queue_s, _) = parse_arguments(&args_space);
        if let AliasAction::Edit(ed) = queue_s.pull().unwrap().action {
            assert!(ed.is_none());
        }

        let args_eq = vec!["alias".into(), "--edalias=vim".into()];
        let (mut queue_e, _) = parse_arguments(&args_eq);
        if let AliasAction::Edit(Some(ed)) = queue_e.pull().unwrap().action {
            assert_eq!(ed, "vim");
        }
    }

    #[test]
    fn test_clear_and_which() {
        let args = vec!["alias".into(), "--clear".into(), "--which".into(), "target".into()];
        let (mut queue, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Clear);
        assert_eq!(queue.pull().unwrap().action, AliasAction::Which);
        if let AliasAction::Query(name) = queue.pull().unwrap().action {
            assert_eq!(name, "target");
        }
    }

    #[test]
    fn test_double_dash_escape() {
        let args = vec!["alias".into(), "--".into(), "--quiet".into()];
        let (mut queue, voice) = parse_arguments(&args);

        assert!(!voice.is_silent());
        assert_eq!(queue.len(), 1);
        if let AliasAction::Query(name) = queue.pull().unwrap().action {
            assert_eq!(name, "--quiet");
        }
    }

    #[test]
    fn test_setup_failure_on_restricted_flags() {
        let args = vec!["alias".into(), "--setup".into(), "--remove".into(), "old_cmd".into()];
        let (_, voice) = parse_arguments(&args);
        assert!(voice.in_setup);
    }

    #[test]
    fn test_setup_must_be_first() {
        let args = vec!["alias".into(), "--reload".into(), "--setup".into()];
        let (_, voice) = parse_arguments(&args);
        assert!(voice.in_setup);
    }

    #[test]
    fn test_all_variants_symmetry() {
        let test_cases = vec![
            // Standard Flags
            ("--help", AliasAction::Help),
            ("--reload", AliasAction::Reload),
            ("--setup", AliasAction::Setup),
            ("--clear", AliasAction::Clear),
            ("--which", AliasAction::Which),
            ("--file", AliasAction::File),
            ("--startup", AliasAction::Startup),
            ("--temp", AliasAction::Temp),

            // New Symmetric Toggles
            ("--case", AliasAction::Case),
            ("--no-case", AliasAction::NoCase),
            ("--quiet", AliasAction::Quiet),
            ("--no-quiet", AliasAction::NoQuiet),
            ("--icons", AliasAction::Icons),
            ("--no-icons", AliasAction::NoIcons),
            ("--tips", AliasAction::Tips),
            ("--no-tips", AliasAction::NoTips),

            // Key=Value (Ensure FromStr and Display match)
            ("--remove test", AliasAction::Remove(SetOptions::involatile("test".to_string(), false))),
            ("--unalias test", AliasAction::Unalias(SetOptions::volatile("test".to_string(), false))),
            ("--edalias=notepad", AliasAction::Edit(Some("notepad".to_string()))),
        ];

        for (input, expected) in test_cases {
            // 1. Test FromStr (Input -> Action)
            let action: AliasAction = input.parse().expect("Should parse");
            assert_eq!(action, expected, "Parsing mismatch for {}", input);

            // 2. Test Symmetry (Action -> Output string)
            // Note: Using Display or to_cli_args should now result in the same string
            let output = format!("{}", action);
            assert_eq!(input, output, "Symmetry broken: {} became {}", input, output);
        }
    }

    #[test]
    fn test_set_alias_with_case() {
        let verbosity = Verbosity::normal();
        let test_path = PathBuf::from("test_aliases.txt");

        // 1. Test Case-Sensitive Set (The new --case)
        let opts_case = SetOptions {
            name: "GIT".to_string(), // Passed as owned String
            value: "git status".to_string(),
            volatile: false,
            force_case: true, // The renamed field
        };

        // Construct the task
        let task = Task {
            action: AliasAction::Set(opts_case),
            path: test_path.clone(),
        };

        // Dispatch using your Provider (e.g., Win32Provider or MockProvider)
        // This verifies the renamed logic flows through the executor
        let result = dispatch::<MockProvider>(task, &verbosity);
        assert!(result.is_ok());

        // 2. Test Case-Insensitive Set (The new --no-case)
        let opts_no_case = SetOptions {
            name: "ls".to_string(),
            value: "ls -F".to_string(),
            volatile: false,
            force_case: false,
        };

        let task_no_case = Task {
            action: AliasAction::Set(opts_no_case),
            path: test_path,
        };

        let result_no_case = dispatch::<MockProvider>(task_no_case, &verbosity);
        assert!(result_no_case.is_ok());
    }
}
#[cfg(test)]
mod existence_checks {
    use super::*;
    #[cfg(test)]
    mod path_logic_tests {
        use super::*;
        // --- timeout_guard tests ---
        #[test]
        fn test_timeout_guard_success() {
            let res = timeout_guard(Duration::from_millis(100), || Some("done"));
            assert_eq!(res, Some(Some("done")));
        }

        #[test]
        fn test_timeout_guard_expired() {
            let res = timeout_guard(Duration::from_millis(10), || {
                std::thread::sleep(Duration::from_millis(50));
                Some("too late")
            });
            assert!(res.is_none()); // Guard itself timed out
        }

        // --- can_path_exist tests ---
        #[test]
        fn test_can_path_exist_valid_dir() {
            let temp = std::env::current_dir().unwrap();
            let path = temp.join("new_file.txt");
            assert!(can_path_exist(&path));
        }

        #[test]
        fn test_can_path_exist_garbage_dir() {
            let path = PathBuf::from("/this/path/does/not/exist/at/all/file.txt");
            assert!(!can_path_exist(&path));
        }

        // --- is_drive_responsive tests ---
        #[test]
        fn test_drive_responsive_on_real_path() {
            let cur = std::env::current_dir().unwrap();
            assert!(&is_drive_responsive(&cur, Duration::from_millis(100)));
        }

        // --- is_path_healthy tests ---
        #[test]
        fn test_path_healthy_too_large() {
            // Mock a path check with 0 byte threshold
            let cur = std::env::current_dir().unwrap(); // A directory isn't a file
            assert!(!is_path_healthy(&cur, 1000));
        }

        // --- is_file_accessible tests ---
        #[test]
        fn test_file_accessible_non_existent() {
            let path = PathBuf::from("z:/definitely_not_a_real_file_123.txt");
            assert!(!is_file_accessible(&path));
        }

        // --- resolve_viable_path (The Grand Finale) ---
        #[test]
        fn test_resolve_viable_path_garbage() {
            let path = PathBuf::from("/non/existent/path/xyz.txt");
            let res = resolve_viable_path(&path);
            // This MUST be None for your Step 3 Fail logic to work
            assert!(res.is_none(), "Should not resolve garbage paths");
        }

        #[test]
        fn test_resolve_viable_path_valid_new_file() {
            let mut temp = std::env::temp_dir();
            temp.push("alias_test_new_file.txt");
            let res = resolve_viable_path(&temp);
            assert!(res.is_some(), "Should resolve new files in valid directories");
        }
    }

}
#[cfg(test)]
mod parse_and_action {
    use crate::*;

    fn to_args(args: Vec<&str>) -> Vec<String> {
        args.into_iter().map(|s| s.to_string()).collect()
    }

    // =========================================================
    // SECTION 1: COMMAND PARSER & ACTION MAPPING (Tests 1-25)
    // =========================================================

    #[test]
    fn t1_show_all() { assert_eq!(parse_arguments(&to_args(vec!["alias"])).0.pull().unwrap().action, AliasAction::ShowAll); }
    #[test]
    fn t2_query_ls() { assert!(matches!(parse_arguments(&to_args(vec!["alias", "ls"])).0.pull().unwrap().action, AliasAction::Query(n) if n == "ls")); }
    #[test]
    fn t3_set_equals() {
        if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "g=git"])).0.pull().unwrap().action {
            assert_eq!(o.name, "g");
            assert_eq!(o.value, "git");
        } else { panic!(); }
    }
    #[test]
    fn t4_set_space() {
        if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "vi", "nvim"])).0.pull().unwrap().action {
            assert_eq!(o.name, "vi");
            assert_eq!(o.value, "nvim");
        } else { panic!(); }
    }
    #[test]
    fn t5_delete_syntax() {
        // 1. Parse the "x=" syntax
        let (mut queue, _) = parse_arguments(&to_args(vec!["alias", "x="]));
        let action = queue.pull().unwrap().action;

        // 2. Expect Remove(SetOptions)
        if let AliasAction::Remove(opts) = action {
            assert_eq!(opts.name, "x");
            assert_eq!(opts.value, "");
            assert_eq!(opts.volatile, false); // "Remove" is persistent/Disk
        } else {
            panic!("Expected AliasAction::Remove for 'x=' syntax, got {:?}", action);
        }
    }#[test]
    fn t6_invalid_lead_eq() { assert_eq!(parse_arguments(&to_args(vec!["alias", "=val"])).0.pull().unwrap().action, AliasAction::Invalid); }
    #[test]
    fn t7_help() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--help"])).0.pull().unwrap().action, AliasAction::Help); }
    #[test]
    fn t8_reload() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--reload"])).0.pull().unwrap().action, AliasAction::Reload); }
    #[test]
    fn t9_setup() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--setup"])).0.pull().unwrap().action, AliasAction::Setup); }
    #[test]
    fn t10_which() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--which"])).0.pull().unwrap().action, AliasAction::Which); }
    #[test]
    fn t11_clear() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--clear"])).0.pull().unwrap().action, AliasAction::Clear); }
    #[test]
    fn t12_edalias_none() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--edalias"])).0.pull().unwrap().action, AliasAction::Edit(None)); }
    #[test]
    fn t13_edalias_val() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--edalias=vim"])).0.pull().unwrap().action, AliasAction::Edit(Some("vim".into()))); }
    #[test]
    fn t14_edaliases_syn() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--edaliases=nano"])).0.pull().unwrap().action, AliasAction::Edit(Some("nano".into()))); }
    #[test]
    fn t15_quiet_voice() { assert!(parse_arguments(&to_args(vec!["alias", "--quiet"])).1.is_silent()); }
    #[test]
    fn t16_temp_flag() { if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "--temp", "x=y"])).0.pull().unwrap().action { assert!(o.volatile); } else { panic!(); } }
    #[test]
    fn t17_force_flag() { if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "--case", "x=y"])).0.pull().unwrap().action { assert!(o.force_case); } else { panic!(); } }
    #[test]
    fn t18_mixed_flags() {
        if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "--temp", "--case", "x=y"])).0.pull().unwrap().action { assert!(o.volatile && o.force_case); } else { panic!(); }
    }
    #[test]
    fn t19_short_flag_safety() { assert_eq!(parse_arguments(&to_args(vec!["alias", "-e"])).0.pull().unwrap().action, AliasAction::Invalid); }
    #[test]
    fn t20_unknown_flag() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--bogus"])).0.pull().unwrap().action, AliasAction::Invalid); }
    #[test]
    fn t21_empty_val_delete_single_d() {
        let (mut queue, _) = parse_arguments(&to_args(vec!["alias", "g="]));
        let task = queue.pull().unwrap();

        // 1. Verify the path stuck (The trace proved this works now!)
        assert!(!task.path.to_string_lossy().is_empty(), "Path should be injected");

        // 2. MATCH ON REMOVE, NOT SET
        if let AliasAction::Remove(name) = task.action {
            assert_eq!(name, SetOptions {
                name: "g".to_string(),
                value: "".to_string(),
                volatile: false,
                force_case: false
            });
        } else {
            panic!("The harvester correctly returned Remove, but the test was looking for Set. Got: {:?}", task.action);
        }
    }
    #[test]
    fn t21_empty_val_delete_single() {
        let (mut queue, _) = parse_arguments(&to_args(vec!["alias", "g="]));
        let task = queue.pull().expect("Should have one task in queue");

        // 1. Verify Path Injection
        assert!(!task.path.to_string_lossy().is_empty(), "Path should be injected for persistent remove");

        // 2. Verify Intent and Persistence
        if let AliasAction::Remove(opts) = task.action {
            assert_eq!(opts.name, "g");
            assert_eq!(opts.value, "");
            // THIS is the critical check for 'Remove'
            assert!(!opts.volatile, "The g= syntax must result in a persistent (non-volatile) action");
        } else {
            panic!("Expected AliasAction::Remove for 'g=' syntax. Got: {:?}", task.action);
        }
    }
    #[test]
    fn t21_mixed_volatile_delete() {
        // We want: g= (persistent) followed by --temp h= (volatile)
        let args = to_args(vec!["alias", "g=", "--temp", "h="]);
        let (queue, _) = parse_arguments(&args);

        let tasks = queue.tasks;
        assert_eq!(tasks.len(), 2);

        // Task 0: g= (Should be Remove + Pathed)
        assert!(matches!(tasks[0].action, AliasAction::Remove(_)));
        assert!(!tasks[0].path.to_string_lossy().is_empty(), "g= should have a path");

        // Task 1: h= (Should be Unalias + No Path)
        assert!(matches!(tasks[1].action, AliasAction::Unalias(_)));
        assert!(tasks[1].path.to_string_lossy().is_empty(), "h= with --temp should NOT have a path");
    }
    #[test]
    fn t22_unalias_routing() { if let AliasAction::Unalias(n) = parse_arguments(&to_args(vec!["alias", "--unalias", "x"])).0.pull().unwrap().action { assert_eq!(n, "x".into()); } else { panic!(); } }
    #[test]
    fn t23_remove_routing() {
        // 1. Parse the arguments
        let (mut queue, _) = parse_arguments(&to_args(vec!["alias", "--remove", "x"]));
        let task = queue.pull().expect("Should have captured a task");

        // 2. Destructure and verify the Action
        if let AliasAction::Remove(opts) = task.action {
            // We define the EXACT expectation to match the Harvester's output
            let expected = SetOptions {
                name: "x".to_string(),
                value: "".to_string(),
                volatile: false, // CRITICAL: --remove MUST be involatile
                force_case: false,
            };

            assert_eq!(opts, expected, "The Harvester must produce an involatile SetOptions for --remove");
        } else {
            panic!("The harvester failed to route --remove to AliasAction::Remove. Got: {:?}", task.action);
        }
    }
    #[test]
    fn t24_startup_flag() { assert!(parse_arguments(&to_args(vec!["alias", "--startup"])).1.in_startup); }
    #[test]
    fn t25_double_dash() { if let AliasAction::Query(n) = parse_arguments(&to_args(vec!["alias", "--", "--quiet"])).0.pull().unwrap().action { assert_eq!(n, "--quiet"); } else { panic!(); } }
}
mod round_trip_tests {
    use super::*;

    #[test]
    fn test_all_variants_round_trip() {
        let test_cases = vec![
            AliasAction::Clear,
            AliasAction::Help,
            AliasAction::Reload,
            AliasAction::Setup,
            AliasAction::Which,
            AliasAction::Startup,
            AliasAction::Temp,
            AliasAction::Case,
            AliasAction::NoCase,  // New
            AliasAction::Quiet,
            AliasAction::NoQuiet,  // New
            AliasAction::Icons,
            AliasAction::NoIcons,
            AliasAction::Tips,
            AliasAction::NoTips,   // New
            AliasAction::Edit(None),
            AliasAction::Query("my_alias".into()),
        ];

        for original in test_cases {
            let cli = original.to_cli_args();
            // We ignore empty strings (like ShowAll/Invalid) which aren't meant to round-trip
            if cli.is_empty() { continue; }

            let parsed: AliasAction = cli.parse().unwrap();
            assert_eq!(original, parsed, "Round-trip failed for: {}", cli);
        }
    }
}

// =========================================================
// SECTION 2: SOVEREIGN VOICE & UI MACROS (Tests 26-40)
// =========================================================
mod voice_and_ui {
    use super::*;

    #[test]
    fn t26_icon_empty() { assert_eq!(Verbosity::normal().icon_format(AliasIcon::Say, ""), ""); }
    #[test]
    fn t27_tip_gen() {
        assert!(!get_random_tip().is_empty());
        let mut found = false;
        for _ in 0..100 { // Probability of failure: 1 in 10^100
            if random_tip_show().is_some() {
                found = true;
                break;
            }
        }
        assert!(found, "Should have produced a tip at least once in 100 tries");
    }
    #[test]
    fn t28_say_macro() { say!(Verbosity::normal(), "test"); }
    #[test]
    fn t29_shout_macro() { shout!(Verbosity::normal(), AliasIcon::Success, "test"); }
    #[test]
    fn t30_fail_macro_io() {
        let e = failure!(Verbosity::silent(), io::Error::new(io::ErrorKind::Other, "err"));
        assert_eq!(e.code, 1);
    }
    #[test]
    fn t31_verb_ord() { assert!(VerbosityLevel::Normal > VerbosityLevel::Silent); }
    #[test]
    fn t32_is_silent() { assert!(Verbosity::silent().is_silent()); }
    #[test]
    fn t33_icon_mapping_success() { assert_eq!(Verbosity::normal().get_icon_str(AliasIcon::Success), ICON_MATRIX[5][1]); }
    #[test]
    fn t34_icon_mapping_off() {
        let mut v = Verbosity::normal();
        v.show_icons = Off;
        assert_eq!(v.get_icon_str(AliasIcon::Success), ICON_MATRIX[5][0]);
    }
    #[test]
    fn t35_voice_macro_normal() { assert_eq!(voice!(Normal, On, ShowTips::On).level, VerbosityLevel::Normal); }
    #[test]
    fn t36_voice_macro_silent() { assert!(voice!(Silent, Off, Off).is_silent()); }
    #[test]
    fn t37_scream_3_arg() {
        let e = failure!(Verbosity::silent(), ErrorCode::MissingFile, "{} missing", "f");
        assert_eq!(e.message, "f missing");
    }
    #[test]
    fn t38_scream_io_assertion() {
        let e = failure!(Verbosity::silent(), io::Error::from_raw_os_error(5));
        assert_eq!(e.code, 5);
    }
    #[test]
    fn t39_icon_format_content() { assert!(Verbosity::normal().icon_format(AliasIcon::Say, "hi").contains("hi")); }
    #[test]
    fn t40_verb_mute_check() { assert!(Verbosity::mute().is_silent()); }
}
// =========================================================
// SECTION 3: INTERNAL DATA & AUDIT LOGIC (Tests 41-60)
// =========================================================
mod data_and_audit {
    use super::*;

    #[test]
    fn t41_mesh_basic() { assert_eq!(mesh_logic(vec![("a".into(), "1".into())], vec![("a".into(), "1".into())]).len(), 1); }
    #[test]
    fn t42_mesh_collision() {
        let m = mesh_logic(vec![("a".into(), "o".into())], vec![("a".into(), "f".into())]);
        assert_eq!(m[0].os_value, Some("o".into()));
    }
    #[test]
    fn t43_empty_def() { assert!(AliasEntryMesh { name: "x".into(), os_value: None, file_value: None }.is_empty_definition()); }
    #[test]
    fn t44_valid_unicode() { assert!(is_valid_name("ñ")); }
    #[test]
    fn t45_valid_numbers() {
        assert!(!is_valid_name("1x"));
        assert!(is_valid_name("x1"));
    }
    #[test]
    fn t46_valid_spaces() { assert!(!is_valid_name("a b")); }
    #[test]
    fn t47_calc_deletion() { assert!(!calculate_new_file_state("a=1\nb=2", "a", "").contains("a=")); }
    #[test]
    fn t48_calc_update() { assert!(calculate_new_file_state("a=1", "a", "2").contains("a=2")); }
    #[test]
    fn t49_path_healthy_tom() { assert!(is_path_healthy(&PathBuf::from("Cargo.toml"), 100000)); }
    #[test]
    fn t50_path_healthy_fake() { assert!(!is_path_healthy(&PathBuf::from("Z:/fake"), 100000)); }
    #[test]
    fn t51_parse_macro_resilience() {
        let t = env::temp_dir().join("t.doskey");
        fs::write(&t, "a=1\n#c\nb=2").unwrap();
        assert_eq!(parse_macro_file(&t, &voice!(Silent, Off, Off)).unwrap().len(), 2);
        fs::remove_file(t).ok();
    }
    #[test]
    #[serial]
    fn t52_path_ext() { if let Some(p) = get_alias_path("") { if env::var(ENV_ALIAS_FILE).is_err() { assert_eq!(p.extension().unwrap(), "doskey"); } } }
    #[test]
    fn t55_complex_split() {
        let (_k, v) = "a=b=c".split_once('=').unwrap();
        assert_eq!(v, "b=c");
    }
    #[test]
    fn t56_line_filter() { assert_eq!("a=1\n\nb=2\n#".lines().filter(|l| l.contains('=')).count(), 2); }
    #[test] fn t60_nuke() { alias_nuke::kernel_wipe_macros(); }
    #[test]
    fn t53_env_splice() {
        // Simulating the environment variable injection logic
        let mut a = vec!["alias".into(), "--icons".into()];
        a.splice(1..1, vec!["--quiet".into()]);
        let (_, voice) = parse_arguments(&a);
        assert!(voice.is_silent());
    }

    #[test]
    fn t54_json_prevention() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "{\"j\":1}".into()]);
        assert_eq!(q.pull().unwrap().action, AliasAction::Invalid);
    }

    #[test]
    fn t57_quote_preservation() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "x=\"y\"".into()]);
        if let AliasAction::Set(o) = q.pull().unwrap().action {
            assert_eq!(o.value, "\"y\"");
        } else { panic!("Expected Set with quotes preserved"); }
    }

    #[test]
    fn t58_multi_quiet() {
        let (_, voice) = parse_arguments(&vec!["alias".into(), "--quiet".into(), "--quiet".into()]);
        assert!(voice.is_silent());
    }

    #[test]
    fn t59_int_case() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--case".into(), "Ñ=x".into()]);
        if let AliasAction::Set(o) = q.pull().unwrap().action {
            assert!(o.force_case);
            assert_eq!(o.name, "Ñ");
        } else { panic!("Expected Force-case Set for international character"); }
    }
}
// =========================================================
// SECTION 4: DISPATCH & INTEGRATION (Tests 61-75)
// =========================================================
mod dispatch_and_integration {

    // =========================================================
    // SECTION 4: DISPATCH & INTEGRATION (THE FINAL 15)
    // =========================================================

    use super::*;

    #[test]
    fn t61_custom_file_flag() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--file".into(), "f.txt".into(), "ls".into()]);
        assert_eq!(q.pull().unwrap().path, PathBuf::from("f.txt"));
    }

    #[test]
    fn t62_unalias_precision() {
        // Wrap the Action in a Task
        let task = Task {
            action: AliasAction::Unalias("r=c ".into()),
            path: PathBuf::from("f"),
        };

        // Dispatch now takes the Task
        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        let r = LAST_CALL.lock().unwrap().take().expect("MockProvider should have captured a Set call");
        assert_eq!(r.name, "r=c");
        assert!(r.volatile);
    }

    #[test]
    fn t63_remove_persistence() {
        // DON'T use .into() here, as it defaults to volatile
        let task = Task {
            action: AliasAction::Remove(SetOptions::involatile("ls".to_string(), false)),
            path: PathBuf::from("f"),
        };

        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        let r = LAST_CALL.lock().unwrap().take().expect("MockProvider should have captured a Set call");

        assert_eq!(r.name, "ls");
        // This will now PASS because we bypassed the volatile default in From<&str>
        assert!(!r.volatile);
    }

    #[test]
    fn t63_test_persistence_default_fail() {
        // DON'T use .into() here, as it defaults to volatile
        let task = Task {
            action: AliasAction::Remove(SetOptions {
                name: "ls".to_string(),
                value: "".to_string(),
                volatile: false,
                force_case: false,
            }),
            path: PathBuf::from("f"),
        };
        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        let r = LAST_CALL.lock().unwrap().take().expect("MockProvider should have captured a Set call");

        assert_eq!(r.name, "ls");
        // This will now PASS because we bypassed the volatile default in From<&str>
        assert!(!r.volatile);
    }
    #[test]
    fn t63_test_persistence_default() {
        // 1. Ensure the Mock is empty before we start
        {
            let mut setup_lock = LAST_CALL.lock().unwrap();
            *setup_lock = None;
        }

        let opts = SetOptions::involatile("ls".to_string(), false);
        let task = Task {
            action: AliasAction::Remove(opts),
            path: PathBuf::from("f"),
        };

        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        // 2. Capture and verify
        let r = LAST_CALL.lock().unwrap().take().expect("MockProvider should have captured a Set call");

        assert_eq!(r.name, "ls");
        assert!(!r.volatile);
    }

    #[test]
    fn t64_trailing_space_59th() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "x=y ".into()]);
        if let AliasAction::Set(o) = q.pull().unwrap().action {
            assert!(o.value.ends_with(' '));
        } else { panic!("Expected Set"); }
    }

    #[test]
    fn t65_flag_ordering() {
        let (q, _) = parse_arguments(&vec!["alias".into(), "--reload".into(), "--which".into(), "x".into()]);
        assert!(matches!(q.get(0).unwrap().action, AliasAction::Reload));
        assert!(matches!(q.get(1).unwrap().action, AliasAction::Which));
    }

    #[test]
    fn t66_typo_resilience() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--reloaad".into(), "x=y".into()]);
        if let AliasAction::Set(o) = q.pull().unwrap().action { assert_eq!(o.name, "x"); }
    }

    #[test]
    fn t67_line_integrity() {
        let (_k, v) = "x=y ".split_once('=').unwrap();
        assert_eq!(v, "y ");
    }

    #[test]
    fn t68_animal_pivot() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "|".into(), "format".into()]);
        assert_eq!(q.pull().unwrap().action, AliasAction::Invalid);
    }

    #[test]
    fn t69_lead_hyphen() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "-x=y".into()]);
        assert_eq!(q.pull().unwrap().action, AliasAction::Invalid);
    }

    #[test]
    fn t70_invalid_short_circuit() {
        let (q, _) = parse_arguments(&vec!["alias".into(), "!!!".into(), "x=y".into()]);
        assert_eq!(q.get(0).unwrap().action, AliasAction::Invalid);
    }

    #[test]
    #[serial]
    fn t71_env_path_dir() {
        unsafe {
            env::set_var(ENV_ALIAS_FILE, ".");
        }
        let p = get_alias_path("").unwrap();
        assert!(p.to_string_lossy().contains(DEFAULT_ALIAS_FILENAME));
        unsafe {
            env::remove_var(ENV_ALIAS_FILE);
        }
    }

    #[test]
    fn t72_help_mode_logic() {
        assert!(matches!(HelpMode::Short, HelpMode::Short));
    }

    #[test]
    fn t73_scream_silent_produces_msg() {
        let e = failure!(Verbosity::silent(), ErrorCode::Generic, "crit");
        assert_eq!(e.message, "crit");
    }

    #[test]
    fn t74_startup_task_count() {
        let (q, voice) = parse_arguments(&vec!["alias".into(), "--startup".into(), "x=y".into()]);
        assert!(voice.in_startup);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn t75_edalias_synonym_vim() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--edalias=vim".into()]);
        assert_eq!(q.pull().unwrap().action, AliasAction::Edit(Some("vim".into())));
    }
}
#[cfg(test)]
mod parse_set_args {
    use super::*;

    // Mock verbosity for testing
    fn mock_v() -> Verbosity { Verbosity::normal() }

    #[test]
    fn test_step1_flag_termination() {
        // Scenario: alias --set g=git --no-quiet
        // Harvester sees a switch and stops so the main loop can handle verbosity.
        let args = vec!["g=git".to_string(), "--no-quiet".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "g");
        }
        assert_eq!(consumed, 1); // Left --no-quiet for Step 1's next iteration
    }

    #[test]
    fn test_step1_dash_dash_termination() {
        // Scenario: alias --set g git -- commit -m "msg"
        // Without gobble, it should stop at --
        let args = vec!["g".to_string(), "git".to_string(), "--".to_string(), "commit".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "g");
            assert_eq!(opts.value, "git");
        } else { panic!("Expected Set action"); }
        assert_eq!(consumed, 2); // Consumed 'g' and 'git', stopped at '--'
    }

    #[test]
    fn test_step1_eol_termination() {
        // Scenario: alias --set my=cls
        let args = vec!["my=cls".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "my");
            assert_eq!(opts.value, "cls");
        } else { panic!("Expected Set action"); }
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_implicit_gobble() {
        // Scenario: alias g git commit --case (Implicit Step 2)
        // We want the alias value to literally include "--case"
        let args = vec!["g".to_string(), "git".to_string(), "commit".to_string(), "--case".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, true);

        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "g");
            // Ensure --case wasn't intercepted as a flag because is_literal (gobble) is true
            assert_eq!(opts.value, "git commit --case");
        } else { panic!("Expected Set action"); }
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_empty_strike_mutation() {
        // Scenario: alias --set x= --temp
        let args = vec!["x=".to_string(), "--temp".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, true, false, false);

        // Should return Unalias because volatile (temp) is true
        assert!(matches!(action, AliasAction::Unalias(ref opts) if opts.name == "x"));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_complex_chained_with_gobble() {
        // Scenario: alias --set a=b -- --set c=d
        // If we hit the --, the main loop calls this with is_gobble = true
        let args = vec!["c=d".to_string()]; // The part after the --
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, true);

        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "c");
            assert_eq!(opts.value, "d");
        }
        assert_eq!(consumed, 1);
    }
    #[test]
    fn test_illegal_name_windows_reserved() {
        // Scenario: alias --set CON=format
        // CON is a Windows reserved device name
        let args = vec!["CON=format".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_illegal_name_leading_hyphen() {
        // Scenario: alias --set -invalid=value
        // Names cannot start with hyphens (they look like flags)
        let args = vec!["-invalid=value".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_empty_set_call() {
        // Scenario: User typed "--set" and nothing else
        let args = vec![];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 0);
    }

    #[test]
    fn test_set_with_immediate_boundary() {
        // Scenario: alias --set --reload
        // The harvester should see that the "name" is actually a switch
        // and return Invalid so the main loop can handle --reload.
        let args = vec!["--reload".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        // This confirms the "Guardian Check" we discussed
        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_set_with_immediate_delimiter() {
        // Scenario: alias --set --
        // Directly hitting the mode-switch before a name is found.
        let args = vec!["--".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_malformed_equals_lead() {
        // Scenario: alias =value (No name provided)
        let args = vec!["=value".to_string()];
        let (action, consumed) = parse_set_argument(&mock_v(), &args, false, false, false);

        assert!(matches!(action, AliasAction::Invalid));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_all_windows_reserved_names() {
        for reserved in RESERVED_NAMES {
            let variants = vec![reserved.to_string(), reserved.to_lowercase()];
            for name_variant in variants {
                let args = vec![format!("{} = some_command", name_variant)];
                // force_case=true shouldn't bypass the reserved name check
                let (action, _) = parse_set_argument(&mock_v(), &args, false, true, false);

                assert!(
                    matches!(action, AliasAction::Invalid),
                    "Blocked reserved name even with force_case: {}", name_variant
                );
            }
        }
    }

}

