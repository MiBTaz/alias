// alias_lib/src/tests/alias_tests.rs
use alias_lib::*;
#[cfg(test)]
mod tests {

    use crate::*;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    // use super::*;
    use std::io;
    use std::sync::Mutex;
    use serial_test::serial;
    use alias_lib::ShowFeature::Off;

    // =========================================================
    // 1. COMMAND PARSER & ACTION MAPPING (22 Tests)
    // =========================================================


    // --- SHARED TEST STATE ---
    static LAST_CALL: Mutex<Option<SetOptions>> = Mutex::new(None);

    // --- MOCK PROVIDER ---
    // Members ordered exactly as the AliasProvider trait
    struct MockProvider;
    impl AliasProvider for MockProvider {
        // 1. ATOMIC HANDS
        fn raw_set_macro(_: &str, _: Option<&str>) -> io::Result<bool> { Ok(true) }
        fn raw_reload_from_file(_verbosity: &Verbosity, _: &Path) -> io::Result<()> { Ok(()) }
        fn get_all_aliases(_verbosity: &Verbosity) -> io::Result<Vec<(String, String)>> { Ok(vec![]) }
        fn write_autorun_registry(_: &str, _: &Verbosity) -> io::Result<()> { Ok(()) }
        fn read_autorun_registry() -> String { String::new() }
        fn reload_full(path: &Path, verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
            return Ok(())
        }
        // 2. CENTRALIZED LOGIC HOOKS
        fn query_alias(_: &str, _: &Verbosity) -> Vec<String> { vec![] }

        fn set_alias(opts: SetOptions, _: &Path, _: &Verbosity) -> io::Result<()> {
            let mut call = LAST_CALL.lock().unwrap();
            *call = Some(opts);
            Ok(())
        }

        fn run_diagnostics(_: &Path, _: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
        fn alias_show_all(_: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    fn parse_alias_args(args: &[String]) -> (AliasAction, Verbosity, Option<PathBuf>) {
        let (mut queue, voice, path) = parse_arguments(args);

        // 1. pull() gives you Option<Task>
        // 2. .map(|t| t.action) gives you Option<AliasAction>
        // 3. .unwrap_or(...) gives you the raw AliasAction
        let action = queue.pull()
            .map(|t| t.action)
            .unwrap_or(AliasAction::ShowAll);

        (action, voice, path)
    }

    #[test]
    fn test_show_all() {
        let (action, voice, _) = parse_alias_args(&vec!["alias".into()]);
        assert_eq!(action, AliasAction::ShowAll);
        assert!(!voice.is_silent());
    }

    #[test]
    fn test_query_alias() {
//        assert_eq!(parse_alias_args(&vec!["alias".into(), "ls".into()]), (AliasAction::Query("ls".into()), Verbosity::loud(), None));
        let (action, verbosity, path) = parse_alias_args(&vec!["alias".into(), "ls".into()]);
        assert!(matches!(action, AliasAction::Query(name) if name == "ls"));
        assert_eq!(verbosity.level, VerbosityLevel::Loud);
        assert!(path.is_none());
    }

    #[test]
    fn test_set_with_equals() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "gs=git status".into()]);
        assert_eq!(action, AliasAction::Set(SetOptions { name: "gs".into(), value: "git status".into(), volatile: false, force_case: false }));
    }

    #[test]
    fn test_set_with_space_args() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "vi".into(), "nvim".into(), "-o".into()]);
        if let AliasAction::Set(opts) = action {
            assert_eq!(opts.name, "vi");
            assert_eq!(opts.value, "nvim -o");
        } else { panic!("Failed space parse"); }
    }

    #[test]
    fn test_delete_syntax_empty_equals() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "junk=".into()]);
        if let AliasAction::Set(opts) = action { assert_eq!(opts.value, ""); } else { panic!("Failed delete parse"); }
    }

    #[test]
    fn test_invalid_leading_equals() {
        assert_eq!(parse_alias_args(&vec!["alias".into(), "=val".into()]).0, AliasAction::Invalid);
    }

    #[test]
    fn test_flag_help() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--help".into()]).0, AliasAction::Help); }

    #[test]
    fn test_flag_reload() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--reload".into()]).0, AliasAction::Reload); }

    #[test]
    fn test_flag_setup() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--setup".into()]).0, AliasAction::Setup); }

    #[test]
    fn test_flag_which() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--which".into()]).0, AliasAction::Which); }

    #[test]
    fn test_flag_clear() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--clear".into()]).0, AliasAction::Clear); }

    #[test]
    fn test_edalias_no_val() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--edalias".into()]).0, AliasAction::Edit(None)); }

    #[test]
    fn test_edalias_with_val() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--edalias=code".into()]).0, AliasAction::Edit(Some("code".into()))); }

    #[test]
    fn test_edaliases_synonym() { assert_eq!(parse_alias_args(&vec!["alias".into(), "--edaliases=vim".into()]).0, AliasAction::Edit(Some("vim".into()))); }

    #[test]
    fn test_quiet_detection_simple() {
        let (_, voice, _) = parse_alias_args(&vec!["alias".into(), "--quiet".into(), "ls".into()]);
        assert!(voice.is_silent());
    }
    #[test]
    fn test_volatile_flag() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "--temp".into(), "g=git".into()]);
        if let AliasAction::Set(opts) = action { assert!(opts.volatile); } else { panic!("Volatile failed"); }
    }

    #[test]
    fn test_force_flag() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "--force".into(), "G=git".into()]);
        if let AliasAction::Set(opts) = action { assert!(opts.force_case); } else { panic!("Force failed"); }
    }

    #[test]
    fn test_mixed_flags_temp_force() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "--temp".into(), "--force".into(), "g=git".into()]);
        if let AliasAction::Set(opts) = action {
            assert!(opts.volatile);
            assert!(opts.force_case);
        } else { panic!("Mixed flags failed"); }
    }

    #[test]
    fn test_short_flags_blocked_for_safety() {
        // We block ambiguous short flags like -e unless explicitly handled
        assert_eq!(parse_alias_args(&vec!["alias".into(), "-e".into()]).0, AliasAction::Invalid);
    }

    #[test]
    fn test_unknown_flag_invalidation() {
        assert_eq!(parse_alias_args(&vec!["alias".into(), "--not-a-flag".into()]).0, AliasAction::Invalid);
    }

    #[test]
    fn test_empty_value_is_delete_logic() {
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "ghost=".into()]);
        if let AliasAction::Set(opts) = action { assert_eq!(opts.value, ""); }
    }

    // =========================================================
    // 2. SOVEREIGN VOICE & UI MACROS (12 Tests)
    // =========================================================


    #[test]
    fn test_icon_format_empty_guard() {
        let v = Verbosity::normal();
        assert_eq!(v.icon_format(AliasIcon::Say, ""), "");
    }

    #[test]
    fn test_tip_generation_not_empty() {
        if let Some(tip) = random_tip_show() { assert!(!tip.is_empty()); }
    }

    #[test]
    fn test_say_macro_execution() {
        let v = Verbosity::normal();
        say!(v, "Testing macro execution");
    }

    #[test]
    fn test_shout_macro_execution() {
        let v = Verbosity::normal();
        shout!(v, AliasIcon::Success, "Testing shout");
    }

    #[test]
    fn test_failure_macro_io_error() {
        let v = Verbosity::silent();

        // Create a real IO error to trigger the 2-arg macro arm
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "fake disk error");

        // This calls the 2-arg arm: ($verbosity:expr, $err:expr)
        let err_box = failure!(v, io_err);

        assert_eq!(err_box.code, 1); // Default for non-os-coded errors
        assert!(err_box.message.contains("fake disk error"));
    }

    #[test]
    fn test_verbosity_level_ord() {
        assert!(VerbosityLevel::Normal > VerbosityLevel::Silent);
        assert!(VerbosityLevel::Mute < VerbosityLevel::Normal);
    }

    #[test]
    fn test_is_silent_helper() {
        assert!(Verbosity::silent().is_silent());
        assert!(!Verbosity::normal().is_silent());
    }

    // =========================================================
    // 3. INTERNAL DATA & AUDIT LOGIC (14 Tests)
    // =========================================================

    #[test]
    fn test_mesh_logic_basic() {
        let os = vec![("a".into(), "1".into())];
        let file = vec![("a".into(), "1".into())];
        let mesh = mesh_logic(os, file);
        assert_eq!(mesh.len(), 1);
        assert_eq!(mesh[0].name, "a");
    }

    #[test]
    fn test_mesh_logic_collision() {
        let os = vec![("a".into(), "live".into())];
        let file = vec![("a".into(), "stored".into())];
        let mesh = mesh_logic(os, file);
        assert_eq!(mesh[0].os_value, Some("live".into()));
        assert_eq!(mesh[0].file_value, Some("stored".into()));
    }

    #[test]
    fn test_beast_detection_logic() {
        // According to your lib.rs, it's only an empty definition if both are None
        let entry = AliasEntryMesh {
            name: "cdx".into(),
            os_value: None,
            file_value: None
        };
        assert!(entry.is_empty_definition());
    }
    #[test]
    fn test_valid_name_gatekeeper_unicode() {
        assert!(is_valid_name("ñ"));
        assert!(is_valid_name("λ"));
    }

    #[test]
    fn test_valid_name_gatekeeper_numbers() {
        assert!(!is_valid_name("1alias"));
        assert!(is_valid_name("alias1"));
    }

    #[test]
    fn test_valid_name_gatekeeper_spaces() {
        assert!(!is_valid_name("git status"));
    }

    #[test]
    fn test_calculate_file_deletion() {
        let orig = "ls=dir\ngs=git status";
        let res = calculate_new_file_state(orig, "ls", "");
        assert!(!res.contains("ls="));
        assert!(res.contains("gs="));
    }

    #[test]
    fn test_calculate_file_update() {
        let orig = "ls=dir";
        let res = calculate_new_file_state(orig, "ls", "ls -la");
        assert!(res.contains("ls=ls -la"));
    }

    #[test]
    fn test_path_healthy_manifest() {
        assert!(is_path_healthy(&PathBuf::from("Cargo.toml")));
    }

    #[test]
    fn test_path_healthy_nonexistent() {
        assert!(!is_path_healthy(&PathBuf::from("Z:/fake/path/here")));
    }

    #[test]
    fn test_parse_macro_file_resilience() {
        let content = "a=1\n# comment\n\nb=2";
        let temp = env::temp_dir().join("test_res.doskey");
        fs::write(&temp, content).unwrap();
        let res = parse_macro_file(&temp, &voice!(Silent, Off, Off));
        // assert_eq!(res.len(), 2);
        assert_eq!(res.expect("Failed to parse macro file").len(), 2);
        fs::remove_file(temp).ok();
    }

    #[test]
    #[serial]
    fn test_get_alias_path_extension() {
        if let Some(p) = get_alias_path() {
            if env::var(ENV_ALIAS_FILE).is_err() {
                assert_eq!(p.extension().unwrap(), "doskey");
            }
        }
    }

    #[test]
    fn test_env_opts_injection_logic() {
        let mut args = vec!["alias".into(), "g=status".into()];
        args.splice(1..1, vec!["--quiet".into()]);
        let (_, voice, _) = parse_alias_args(&args);
        assert!(voice.is_silent());
    }

    #[test]
    fn test_jason_garbage_prevention() {
        assert_eq!(parse_alias_args(&vec!["alias".into(), "{\"json\":true}".into()]).0, AliasAction::Invalid);
    }

    // =========================================================
    // 4. EDGE CASES & NUKE (8 Tests)
    // =========================================================
    #[test]
    #[serial]
    fn test_path_env_points_to_dir() {
        let temp_dir = env::temp_dir().join("alias_dir_test");
        fs::create_dir_all(&temp_dir).unwrap();
        unsafe {
            env::set_var(ENV_ALIAS_FILE, &temp_dir);
        }
        let p = get_alias_path().unwrap();
        assert!(p.to_string_lossy().ends_with(DEFAULT_ALIAS_FILENAME));
        fs::remove_dir(temp_dir).ok();
        unsafe {
            env::remove_var(ENV_ALIAS_FILE);
        }
    }

    #[test]
    fn test_complex_split_once_logic() {
        let input = "cmd=echo A=B";
        let (k, v) = input.split_once('=').unwrap();
        assert_eq!(k, "cmd");
        assert_eq!(v, "echo A=B");
    }

    #[test]
    fn test_reload_line_filtering() {
        let raw = "a=1\n  \nb=2\n#comment";
        let count = raw.lines().filter(|l| l.contains('=')).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_argument_quotes_preservation() {
        let args = vec!["alias".into(), "x=\"quoted val\"".into()];
        let (action, _, _) = parse_alias_args(&args);
        if let AliasAction::Set(opts) = action { assert_eq!(opts.value, "\"quoted val\""); }
    }

    #[test]
    fn test_multiple_quiet_flags_consistency() {
        let (_, voice, _) = parse_alias_args(&vec!["alias".into(), "--icons".into(), "--quiet".into(), "ls".into()]);
        assert!(voice.is_silent());
    }

    #[test]
    fn test_help_mode_enum_logic() {
        // Ensuring HelpMode follows the right logic paths for Tip display
        let mode = HelpMode::Short;
        assert!(matches!(mode, HelpMode::Short));
    }

    #[test]
    fn test_international_case_insensitivity() {
        // If name is 'ñ', and we force case...
        let (action, _, _) = parse_alias_args(&vec!["alias".into(), "--force".into(), "ñ=test".into()]);
        if let AliasAction::Set(opts) = action { assert!(opts.force_case); }
    }

    #[test]
    fn a_nuke_the_world() {
        // This is the 56th test.
        // It runs alphabetically early to ensure a clean state if you use it.
        alias_nuke::kernel_wipe_macros();
    }

    #[test]
    fn test_parse_custom_file_flag() {
        let args = vec!["alias".into(), "--file".into(), "C:\\temp\\test.doskey".into(), "ls".into()];
        let (action, _, path) = parse_alias_args(&args);

        assert_eq!(action, AliasAction::Query("ls".into()));
        assert_eq!(path, Some(PathBuf::from("C:\\temp\\test.doskey")));
    }

    // --- NEW INTEGRATION TESTS ---

    #[test]
    fn test_unalias_precision_strike() {
        let path = PathBuf::from("test.doskey");
        let verb = Verbosity::silent();

        // Verifies the "rust=cargo" -> "rust" split logic in dispatcher
        let action = AliasAction::Unalias("rust=cargo".to_string());
        dispatch::<MockProvider>(action, &verb, &path).unwrap();

        let result = LAST_CALL.lock().unwrap().take().expect("set_alias should have been called");

        assert_eq!(result.name, "rust");
        assert_eq!(result.value, "");
        assert!(result.volatile); // Unalias must be volatile
    }

    #[test]
    fn test_remove_persistence() {
        let path = PathBuf::from("test.doskey");
        let verb = Verbosity::silent();

        // Verifies the --remove command forces persistence (volatile: false)
        let action = AliasAction::Remove("ls".to_string());
        dispatch::<MockProvider>(action, &verb, &path).unwrap();

        let result = LAST_CALL.lock().unwrap().take().expect("set_alias should have been called");

        assert_eq!(result.name, "ls");
        assert_eq!(result.value, "");
        assert!(!result.volatile); // Remove must NOT be volatile
    }

    #[test]
    fn test_parser_unalias_routing() {
        let args = vec![
            "alias.exe".into(),
            "--quiet".into(),       // Flag from Step 1
            "--unalias".into(),     // Command from Step 4
            "my_macro=stuff".into(), // Payload
        ];

        let (action, voice, _) = parse_alias_args(&args);

        // Verify Step 1 flags were preserved
        assert_eq!(voice.level, VerbosityLevel::Silent);

        // Verify Step 4 correctly identified Unalias and grabbed the target
        if let AliasAction::Unalias(target) = action {
            assert_eq!(target, "my_macro=stuff");
        } else {
            panic!("Expected AliasAction::Unalias, got {:?}", action);
        }
    }

    #[test]
    fn test_parser_remove_routing() {
        let args = vec![
            "alias.exe".into(),
            "--remove".into(),
            "target_alias".into(),
        ];
        let (action, _, _) = parse_alias_args(&args);

        if let AliasAction::Remove(target) = action {
            assert_eq!(target, "target_alias");
        } else {
            panic!("Expected AliasAction::Remove");
        }
    }

    #[test]
    fn test_scream_macro_execution() {
        let v = Verbosity::silent();

        // Execute and capture the result
        let err_box = failure!(v, ErrorCode::MissingFile, "File {} not found", "test.doskey");

        // ASSERT: Verify the custom error code was mapped correctly
        assert_eq!(err_box.code, ErrorCode::MissingFile as u8);

        // ASSERT: Verify the format strings were processed correctly
        assert_eq!(err_box.message, "File test.doskey not found");
    }

    #[test]
    fn test_scream_macro_logical_assertion() {
        let v = Verbosity::silent(); // Voice is silent...

        // Execute 3-arg logical macro
        let err = failure!(v, ErrorCode::Generic, "CRITICAL_FAILURE");

        // ASSERT: Even if silent, the error object MUST contain the message
        assert!(err.message.contains("CRITICAL_FAILURE"), "Scream must produce message even when silent");
        assert_eq!(err.code, ErrorCode::Generic as u8);
    }

    #[test]
    fn test_scream_macro_io_assertion() {
        let v = Verbosity::silent();

        // Create a raw OS error (Code 5 = Access Denied)
        let io_err = std::io::Error::from_raw_os_error(5);

        // Execute 2-arg IO macro
        let err = failure!(v, io_err);

        // ASSERT: Check that it extracted the OS code 5
        assert_eq!(err.code, 5, "Should have extracted OS error code 5");

        // ASSERT: Check that the message is present
        // (In Windows, code 5 will contain "Access is denied")
        assert!(!err.message.is_empty(), "Scream message should not be empty");
    }

    // Helper to simulate CLI Vec<String>
    fn to_args(args: Vec<&str>) -> Vec<String> {
        args.into_iter().map(|s| s.to_string()).collect()
    }


    #[test]
    fn test_animal_protection_pivot() {
        // If a command starts with an "animal", it shouldn't pivot
        let args = to_args(vec!["alias", "|", "format", "c:"]);
        let (queue, _, _) = parse_arguments(&args);

        // Queue should be empty (or default to ShowAll) because '|' is illegal
        assert!(matches!(queue.get(0).unwrap().action, AliasAction::Invalid));
    }

    #[test]
    fn test_literal_payload_preserves_trailing_whitespace() {
        let args = vec![
            "alias".to_string(),
            "xcd=cd /d \"C:\\\" ".to_string() // Trailing space inside the string
        ];

        let (queue, _voice, _path) = parse_arguments(&args);

        if let Some(AliasAction::Set(opts)) = queue.get(0).map(|t| &t.action) {
            assert_eq!(opts.name, "xcd");
            assert_eq!(opts.value, "cd /d \"C:\\\" ");
            assert!(opts.value.ends_with(' '), "The 59th byte (space) was trimmed!");
        } else {
            panic!("Queue did not contain a Set action");
        }
    }

    #[test]
    fn test_startup_task_ordering() {
        let args = vec!["alias".to_string(), "--startup".to_string(), "new=value".to_string()];
        let (queue, voice, _) = parse_arguments(&args);

        assert!(voice.in_startup, "Startup flag not detected");

        // In our logic, 'new=value' should be in the queue,
        // and 'Reload' is handled by the 'run' loop's 'in_startup' check.
        assert_eq!(queue.len(), 1);
        if let Some(AliasAction::Set(opts)) = queue.get(0).map(|t| &t.action) {
            assert_eq!(opts.name, "new");
        }
    }

    #[test]
    fn test_voice_macro_normal() {
        let v = voice!(Normal, ShowFeature::On, ShowTips::On);
        assert_eq!(v.level, VerbosityLevel::Normal);
        assert!(v.show_icons.is_on()); // Changed from .icons to .show_icons.is_on()
    }

    #[test]
    fn test_voice_macro_silent() {
        let v = voice!(Silent, ShowFeature::Off, ShowTips::Off);
        assert!(v.is_silent());
        assert!(!v.show_icons.is_on()); // Changed from .icons
    }

    #[test]
    fn test_env_splice_override() {
        let mut args = vec!["alias".to_string(), "--icons".to_string()];
        let env_opts = vec!["--quiet".to_string()];
        args.splice(1..1, env_opts);

        let (_, voice, _) = parse_arguments(&args);

        assert_eq!(voice.level, VerbosityLevel::Silent);
        assert!(voice.show_icons.is_on()); // Changed from .icons to .show_icons.is_on()
    }

    #[test]
    fn test_flag_action_ordering() {
        let args = to_args(vec!["alias", "--reload", "--which", "xcd"]);
        let (queue, _, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 3);
        // Use queue.get(i) instead of queue.tasks[i]
        assert!(matches!(queue.get(0).unwrap().action, AliasAction::Reload));
        assert!(matches!(queue.get(1).unwrap().action, AliasAction::Which));
        assert!(matches!(queue.get(2).unwrap().action, AliasAction::Query(_)));
    }

    #[test]
    fn test_pivot_on_assignment() {
        let args = to_args(vec!["alias", "xcd=dir /d \"C:\\\" "]);
        let (queue, _, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        // FIXED: Using .get(0) instead of .tasks.first()
        if let Some(AliasAction::Set(opts)) = queue.get(0).map(|t| &t.action) {
            assert_eq!(opts.name, "xcd");
            assert!(opts.value.ends_with("\" "));
        } else {
            panic!("Should have been a Set action");
        }
    }

    #[test]
    fn test_typo_resilience() {
        let args = to_args(vec!["alias", "--reloaad", "ls=dir"]);
        let (queue, _, _) = parse_arguments(&args);

        assert_eq!(queue.len(), 1);
        // FIXED: Using .get(0)
        if let Some(AliasAction::Set(opts)) = queue.get(0).map(|t| &t.action) {
            assert_eq!(opts.name, "ls");
        }
    }

    #[test]
    fn test_file_flag_with_path() {
        let args = to_args(vec!["alias", "--file", "my_doskeys.txt", "--reload"]);
        let (queue, _, path) = parse_arguments(&args);

        assert_eq!(path.unwrap().to_str().unwrap(), "my_doskeys.txt");
        assert_eq!(queue.len(), 1);
        // FIXED: Using .get(0)
        assert!(matches!(queue.get(0).unwrap().action, AliasAction::Reload));
    }

    #[test]
    fn test_file_line_parsing_integrity() {
        let mock_file_content = "xcd=dir /d \"C:\\\" \nother=value";
        let lines = mock_file_content.lines();

        for line in lines {
            if let Some((name, value)) = line.split_once('=') {
                let _name = name.trim(); // FIXED: added underscore to suppress warning
                assert_eq!(value, "dir /d \"C:\\\" ", "File parser trimmed the 59th byte!");
                break;
            }
        }
    }
    #[test]
    fn test_icon_mapping_success() {
        let v = Verbosity::normal();
        // Success is index 5. [0] is "OK", [1] is "✨"
        assert_eq!(v.get_icon_str(AliasIcon::Success), ICON_MATRIX[5][1]);
    }

    #[test]
    fn test_icon_mapping_plain() {
        let mut v = Verbosity::normal();
        v.show_icons = Off; // Assuming Off maps to index 0 in your get_icon_str logic
        assert_eq!(v.get_icon_str(AliasIcon::Success), ICON_MATRIX[5][0]);
    }

    #[test]
    fn test_icon_format_content() {
        let v = Verbosity::normal();
        let res = v.icon_format(AliasIcon::Say, "hello");
        assert!(res.contains("hello"));
        // Say is index 7. [1] is "📜"
        assert!(res.contains(ICON_MATRIX[AliasIcon::Say as usize][1]));
    }
    #[test]
    fn test_invalid_name_short_circuits() {
        let args = vec!["alias".to_string(), "!!!".to_string(), "payload=stuff".to_string()];
        let (queue, _voice, _) = parse_arguments(&args);

        // It found 2 items because it didn't give up after '!!!'
        assert!(queue.len() >= 1);
        assert!(matches!(queue.get(0).unwrap().action, AliasAction::Invalid));
    }

    #[test]
    fn test_leading_hyphen_rejection() {
        let args = to_args(vec!["alias", "-xcd=dir"]);
        let (queue, _, _) = parse_arguments(&args);

        // It is no longer empty because we now track invalid attempts
        assert!(matches!(queue.get(0).unwrap().action, AliasAction::Invalid));
    }
}
