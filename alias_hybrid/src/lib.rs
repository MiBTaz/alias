use std::io;
use std::path::Path;
use alias_lib::*;
use alias_win32::Win32LibraryInterface;
use alias_wrapper::WrapperLibraryInterface;

pub struct HybridLibraryInterface;

impl AliasProvider for HybridLibraryInterface {
    // --- 1. THE ATOMIC "HANDS" ---

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        // Try Win32 API first, fallback to Wrapper if API returns false
        let success = Win32LibraryInterface::raw_set_macro(name, value)?;
        if !success {
            return WrapperLibraryInterface::raw_set_macro(name, value);
        }
        Ok(true)
    }

    fn raw_reload_from_file(path: &Path) -> io::Result<()> {
        if Win32LibraryInterface::raw_reload_from_file(path).is_err() {
            return WrapperLibraryInterface::raw_reload_from_file(path);
        }
        Ok(())
    }

    fn get_all_aliases() -> Vec<(String, String)> {
        let mut list = Win32LibraryInterface::get_all_aliases();
        if list.is_empty() {
            list = WrapperLibraryInterface::get_all_aliases();
        }
        list
    }

    fn write_autorun_registry(cmd: &str, v: Verbosity) -> io::Result<()> {
        Win32LibraryInterface::write_autorun_registry(cmd, v)
    }

    fn read_autorun_registry() -> String {
        Win32LibraryInterface::read_autorun_registry()
    }

    // --- 2. THE CENTRALIZED LOGIC (Overriding Defaults for Hybrid Efficiency) ---

    fn purge_ram_macros() -> io::Result<PurgeReport> {
        let report = Win32LibraryInterface::purge_ram_macros()?;
        if report.is_fully_clean() {
            Ok(report)
        } else {
            // API missed something or failed; use the wrapper's aggressive nuke
            WrapperLibraryInterface::purge_ram_macros()
        }
    }

    fn reload_full(path: &Path, verbosity: Verbosity) -> io::Result<()> {
        let _ = Self::purge_ram_macros();
        let definitions = parse_macro_file(path);
        let mut api_success_count = 0;

        for (name, value) in &definitions {
            if Win32LibraryInterface::raw_set_macro(name, Some(value)).unwrap_or(false) {
                api_success_count += 1;
            }
        }

        if api_success_count < definitions.len() {
            shout!(verbosity, AliasIcon::Alert, "API missed {} macros. Running Fallback...", definitions.len() - api_success_count);
            WrapperLibraryInterface::reload_full(path, verbosity)?;
        } else {
            whisper!(verbosity, AliasIcon::Success, "Hybrid Reload: {} macros via API.", api_success_count);
        }
        Ok(())
    }

    // Note: install_autorun is handled by the trait's default implementation
    // using our atomic hands above, but you can override if needed.

    fn query_alias(name: &str, verbosity: Verbosity) -> Vec<String> {
        let mut output = Win32LibraryInterface::query_alias(name, verbosity.clone());
        if output.is_empty() {
            output = WrapperLibraryInterface::query_alias(name, verbosity);
        }
        output
    }

    fn set_alias(opts: SetOptions, path: &Path, verbosity: Verbosity) -> io::Result<()> {
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };
        let val_opt = if opts.value.is_empty() { None } else { Some(opts.value.as_str()) };

        if !Win32LibraryInterface::raw_set_macro(&name, val_opt).unwrap_or(false) {
            WrapperLibraryInterface::set_alias(opts.clone(), path, verbosity.clone())?;
        } else if !opts.volatile {
            update_disk_file(&name, &opts.value, path)?;
        }

        if opts.volatile {
            say!(verbosity, AliasIcon::Win32, "Volatile strike (Hybrid): {}", name);
        }
        Ok(())
    }

    fn run_diagnostics(path: &Path, verbosity: Verbosity) {
        // Hybrid diagnostics usually lean on the Win32 implementation for kernel info
        Win32LibraryInterface::run_diagnostics(path, verbosity);
    }

    fn alias_show_all(verbosity: Verbosity) {
        let w32 = Win32LibraryInterface::get_all_aliases();
        let wrap = WrapperLibraryInterface::get_all_aliases();
        let file = get_alias_path().map(|p| parse_macro_file(&p)).unwrap_or_default();

        // MATCHING YOUR ORDER: verbosity, then the three source vectors
        perform_triple_audit(verbosity, w32, wrap, file);
    }
}
