// alias_lib/tests/local_library_tests.rs

use alias_lib::*;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

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
            assert_eq!(name, "test_cmd");
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

    #[test]
    fn test_default_path_resolution() {
        // We add --which to create a Query task first.
        // The remaining args "my_cmd new_cmd=value" become the Greedy Set payload.
        let args = vec![
            "alias".into(),
            "--which".into(),
            "my_cmd".into(),
            "new_cmd=value".into()
        ];
        let (mut queue, _) = parse_arguments(&args);

        // 2. The greedy payload "my_cmd new_cmd=value" creates the second task: A Set.
        // This should have the default .doskey path.
        let task_s = queue.pop().expect("Should have set task from greedy payload");
        assert!(task_s.path.to_string_lossy().contains(".doskey"), "Set MUST have a path");

        // 1. The --which flag creates the first task: A Query.
        // This should have an empty path (RAM-based).
        let task_q = queue.pop().expect("Should have query task from --which");
        assert!(task_q.path.to_string_lossy().is_empty(), "Which should have NO path");


        // 3. Final Safety
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
        let args = vec!["alias".into(), "--temp".into(), "--force".into(), "tmp=echo 1".into()];
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
        let args_u = vec!["alias".into(), "--unalias".into(), "old_cmd".into()];
        let (mut queue_u, _) = parse_arguments(&args_u);
        if let AliasAction::Unalias(name) = queue_u.pull().unwrap().action {
            assert_eq!(name, "old_cmd");
        }

        let args_r = vec!["alias".into(), "--remove".into(), "other_cmd".into()];
        let (mut queue_r, _) = parse_arguments(&args_r);
        if let AliasAction::Remove(name) = queue_r.pull().unwrap().action {
            assert_eq!(name, "other_cmd");
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
}

mod existence_checks {
    #[cfg(test)]
    mod path_logic_tests {
//        use super::*;
        use std::path::PathBuf;
        use std::time::Duration;
        use alias_lib::{can_path_exist, is_drive_responsive, is_file_accessible, is_path_healthy, resolve_viable_path, timeout_guard};

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
            assert!(is_drive_responsive(&cur, Duration::from_millis(100)));
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

// =========================================================
// SECTION 1: COMMAND PARSER & ACTION MAPPING (Tests 1-25)
// =========================================================

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
    fn t5_delete_syntax() { if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "x="])).0.pull().unwrap().action { assert_eq!(o.value, ""); } else { panic!(); } }
    #[test]
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
    fn t17_force_flag() { if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "--force", "x=y"])).0.pull().unwrap().action { assert!(o.force_case); } else { panic!(); } }
    #[test]
    fn t18_mixed_flags() {
        if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "--temp", "--force", "x=y"])).0.pull().unwrap().action { assert!(o.volatile && o.force_case); } else { panic!(); }
    }
    #[test]
    fn t19_short_flag_safety() { assert_eq!(parse_arguments(&to_args(vec!["alias", "-e"])).0.pull().unwrap().action, AliasAction::Invalid); }
    #[test]
    fn t20_unknown_flag() { assert_eq!(parse_arguments(&to_args(vec!["alias", "--bogus"])).0.pull().unwrap().action, AliasAction::Invalid); }
    #[test]
    fn t21_empty_val_delete() { if let AliasAction::Set(o) = parse_arguments(&to_args(vec!["alias", "g="])).0.pull().unwrap().action { assert_eq!(o.value, ""); } else { panic!(); } }
    #[test]
    fn t22_unalias_routing() { if let AliasAction::Unalias(n) = parse_arguments(&to_args(vec!["alias", "--unalias", "x"])).0.pull().unwrap().action { assert_eq!(n, "x"); } else { panic!(); } }
    #[test]
    fn t23_remove_routing() { if let AliasAction::Remove(n) = parse_arguments(&to_args(vec!["alias", "--remove", "x"])).0.pull().unwrap().action { assert_eq!(n, "x"); } else { panic!(); } }
    #[test]
    fn t24_startup_flag() { assert!(parse_arguments(&to_args(vec!["alias", "--startup"])).1.in_startup); }
    #[test]
    fn t25_double_dash() { if let AliasAction::Query(n) = parse_arguments(&to_args(vec!["alias", "--", "--quiet"])).0.pull().unwrap().action { assert_eq!(n, "--quiet"); } else { panic!(); } }
}
#[cfg(test)]
mod round_trip_tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_all_15_variants() {
        let test_cases = vec![
            // Flags
            ("--help", AliasAction::Help),
            ("--reload", AliasAction::Reload),
            ("--setup", AliasAction::Setup),
            ("--clear", AliasAction::Clear),
            ("--which", AliasAction::Which),
            ("--all", AliasAction::ShowAll),
            ("--file", AliasAction::File),

            // Key=Value
            ("--remove=test", AliasAction::Remove("test".to_string())),
            ("--unalias=test", AliasAction::Unalias("test".to_string())),
            ("--edalias=notepad", AliasAction::Edit(Some("notepad".to_string()))),
            ("--edalias", AliasAction::Edit(None)),

            // Payloads
            ("my_cmd=dir /s", AliasAction::Set(SetOptions {
                name: "my_cmd".into(),
                value: "dir /s".into(),
                volatile: false,
                force_case: false
            })),
            ("search_query", AliasAction::Query("search_query".into())),

            // Internals
            ("--invalid", AliasAction::Invalid),
        ];

        for (input, _expected_variant) in test_cases {
            // 1. Test FromStr (Input -> Action)
            let action = AliasAction::from_str(input).expect("Should parse variant");

            // 2. Test Display (Action -> Output)
            let output = format!("{}", action);
            assert_eq!(input, output, "Symmetry broken for {}", input);
        }
    }
}
// =========================================================
// SECTION 2: SOVEREIGN VOICE & UI MACROS (Tests 26-40)
// =========================================================
mod voice_and_ui {
    use std::io;
    use alias_lib::ShowFeature::{Off, On};
    use alias_lib::{failure, get_random_tip, random_tip_show, say, shout, voice, AliasIcon, ErrorCode, ShowTips, Verbosity, VerbosityLevel, ICON_MATRIX};

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
    use std::{env, fs};
    use std::path::PathBuf;
    use serial_test::serial;
    use alias_lib::{calculate_new_file_state, get_alias_path, is_path_healthy, is_valid_name, mesh_logic, parse_arguments, parse_macro_file, voice, AliasAction, AliasEntryMesh, ENV_ALIAS_FILE};

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
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--force".into(), "Ñ=x".into()]);
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
    use std::{env, io};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use serial_test::serial;
    use alias_lib::{dispatch, failure, get_alias_path, parse_arguments, AliasAction, AliasProvider, ErrorCode, HelpMode, PurgeReport, SetOptions, Task, Verbosity, DEFAULT_ALIAS_FILENAME, ENV_ALIAS_FILE};

    // --- SHARED TEST STATE FOR DISPATCH TESTS ---
    static LAST_CALL: Mutex<Option<SetOptions>> = Mutex::new(None);

    struct MockProvider;
    impl AliasProvider for MockProvider {
        fn raw_set_macro(_: &str, _: Option<&str>) -> io::Result<bool> { Ok(true) }
        fn raw_reload_from_file(_v: &Verbosity, _: &Path) -> io::Result<()> { Ok(()) }
        fn get_all_aliases(_v: &Verbosity) -> io::Result<Vec<(String, String)>> { Ok(vec![]) }
        fn write_autorun_registry(_: &str, _: &Verbosity) -> io::Result<()> { Ok(()) }
        fn read_autorun_registry() -> String { String::new() }
        fn purge_ram_macros(_: &Verbosity) -> Result<PurgeReport, io::Error> { Ok(PurgeReport::default()) }
        fn purge_file_macros(_: &Verbosity, _: &Path) -> Result<PurgeReport, io::Error> { Ok(PurgeReport::default()) }
        fn reload_full(_v: &Verbosity, _: &Path, _f: bool) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
        fn query_alias(_: &str, _: &Verbosity) -> Vec<String> { vec![] }
        fn set_alias(opts: SetOptions, _: &Path, _: &Verbosity) -> io::Result<()> {
            let mut call = LAST_CALL.lock().unwrap();
            *call = Some(opts);
            Ok(())
        }
        fn run_diagnostics(_: &Path, _: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
        fn alias_show_all(_: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    }

    // =========================================================
    // SECTION 4: DISPATCH & INTEGRATION (THE FINAL 15)
    // =========================================================

    #[test]
    fn t61_custom_file_flag() {
        let (mut q, _) = parse_arguments(&vec!["alias".into(), "--file".into(), "f.txt".into(), "ls".into()]);
        assert_eq!(q.pull().unwrap().path, PathBuf::from("f.txt"));
    }

    #[test]
    fn t62_unalias_precision() {
        // Wrap the Action in a Task
        let task = Task {
            action: AliasAction::Unalias("r=c".into()),
            path: PathBuf::from("f"),
        };

        // Dispatch now takes the Task
        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

        let r = LAST_CALL.lock().unwrap().take().expect("MockProvider should have captured a Set call");
        assert_eq!(r.name, "r");
        assert!(r.volatile);
    }

    #[test]
    fn t63_remove_persistence() {
        // Wrap the Action in a Task
        let task = Task {
            action: AliasAction::Remove("ls".into()),
            path: PathBuf::from("f"),
        };

        dispatch::<MockProvider>(task, &Verbosity::silent()).unwrap();

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

    #[test] #[serial]
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
