// alias_hybrid/src/lib.rs

use std::io;
use std::path::Path;

use alias_lib::*;
use alias_wrapper::*;

pub struct HybridLibraryInterface;

impl AliasProvider for HybridLibraryInterface {
    // MATCH: purge_ram_macros() -> io::Result<PurgeReport>
    fn purge_ram_macros() -> io::Result<PurgeReport> {
        match alias_win32::purge_ram_macros() {
            Ok(report) if report.is_fully_clean() => Ok(report),
            _ => alias_wrapper::purge_ram_macros(),
        }
    }

    // MATCH: reload_full(path: &Path, quiet: bool)
    fn reload_full(path: &Path, quiet: bool) -> io::Result<()> {
        // Step 1: Clear the RAM using the hybrid logic
        let _ = Self::purge_ram_macros();

        let definitions = parse_macro_file(path);
        let mut api_success_count = 0;

        // Step 2: Try the Win32 API
        for (name, value) in &definitions {
            if alias_win32::api_set_macro(name, Some(value)) {
                api_success_count += 1;
            }
        }

        // Step 3: Fallback if API missed anything
        if api_success_count < definitions.len() {
            qprintln!(quiet, "⚠️ API missed {} macros. Running Doskey fallback...", definitions.len() - api_success_count);
            alias_wrapper::reload_doskey(path)?;
        } else {
            qprintln!(quiet, "✨ API Reload: {} macros synced.", definitions.len());
        }
        Ok(())
    }

    // MATCH: query_alias(name: &str, mode: OutputMode)
    fn query_alias(term: &str, mode: OutputMode) -> Vec<String> {
        // Since the trait doesn't pass the path, we resolve it internally
        let path = get_alias_path().unwrap_or_default();

        let mut output = alias_win32::query_alias(term, OutputMode::DataOnly);
        if output.is_empty() {
            output = alias_wrapper::query_alias(term, OutputMode::DataOnly);
        }
        if output.is_empty() {
            output = query_alias_file(term, &path, mode);
        }
        output
    }

    // MATCH: set_alias(opts: SetOptions, path: &Path, quiet: bool)
    fn set_alias(opts: SetOptions, path: &Path, quiet: bool) -> io::Result<()> {
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };
        let value = opts.value.trim();
        let val_opt = if value.is_empty() { None } else { Some(value) };

        // Attempt API strike
        if !alias_win32::api_set_macro(&name, val_opt) {
            // Fallback to Command line
            let _ = std::process::Command::new("doskey")
                .args(["/exename=cmd.exe", &format!("{}={}", name, value)])
                .status();
        }

        if opts.volatile {
            qprintln!(quiet, "⚡ Volatile alias: {}", name);
            return Ok(());
        }

        alias_lib::update_disk_file(&name, value, path)
    }

    // MATCH: alias_show_all()
    fn alias_show_all() {
        let path = get_alias_path().expect("❌ Missing alias file");
        let w32 = alias_win32::get_all_aliases();
        let wrap = alias_wrapper::get_all_aliases();
        let file = alias_lib::parse_macro_file(&path);

        alias_lib::perform_triple_audit(w32, wrap, file);
    }

    // MATCH: run_diagnostics(path: &Path)
    fn run_diagnostics(path: &Path) {
        alias_win32::run_diagnostics(path);
    }

    // MATCH: install_autorun(quiet: bool)
    fn install_autorun(quiet: bool) -> io::Result<()> {
        let exe_path = std::env::current_exe()?;
        let cmd = get_autorun_command(&exe_path);
        let status = std::process::Command::new("reg")
            .args(["add", REG_SUBKEY, "/v", REG_AUTORUN_KEY, "/t", "REG_EXPAND_SZ", "/d", &cmd, "/f"])
            .status()?;

        if status.success() {
            qprintln!(quiet, "🛠️ AutoRun installed: {}", cmd);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Registry update failed"))
        }
    }
}