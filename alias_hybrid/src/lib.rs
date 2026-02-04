// alias_hybrid/src/lib.rs
// Licensed under PolyForm Noncommercial 1.0.0 via alias_lib.

use std::io;
use std::path::Path;
use alias_lib::*;
use alias_win32::Win32LibraryInterface;
use alias_wrapper::WrapperLibraryInterface;
#[allow(unused_imports)]
#[cfg(debug_assertions)]
use function_name::named;
include!(concat!(env!("OUT_DIR"), "/version_data.rs"));

extern crate alias_lib;

pub struct HybridLibraryInterface;

impl AliasProvider for HybridLibraryInterface {
    // --- 1. THE ATOMIC "HANDS" ---

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        match Win32LibraryInterface::raw_set_macro(name, value) {
            // 1. Success! Return immediately.
            Ok(true) => Ok(true),

            // 2. Win32 says "False" (it didn't work, but no crash). Try Wrapper.
            Ok(false) => {
                WrapperLibraryInterface::raw_set_macro(name, value)
            },

            // 3. HARD ERROR: The Win32 API actually failed/errored out.
            Err(e) => {
                // Check if it's a "safe" error to ignore (like the API not being available)
                // Otherwise, PERCOLATE the error up.
                if e.kind() == io::ErrorKind::Unsupported {
                    WrapperLibraryInterface::raw_set_macro(name, value)
                } else {
                    // Return the actual error so the user knows WHY it failed.
                    Err(e)
                }
            }
        }
    }

    fn raw_reload_from_file(verbosity: &Verbosity, path: &Path) -> io::Result<()> {
        // Try native reload, fallback to process spawn if it fails
        if Win32LibraryInterface::raw_reload_from_file(verbosity, path).is_err() {
            return WrapperLibraryInterface::raw_reload_from_file(verbosity, path);
        }
        Ok(())
    }

    fn get_all_aliases(verbosity: &Verbosity) -> io::Result<Vec<(String, String)>> {
        // If Win32 returns our "Alert String" (Error 203), try the wrapper.
        let w32_res = Win32LibraryInterface::get_all_aliases(verbosity)?;

        // If the Win32 list only contains our Alert Message, it's "effectively" empty.
        if w32_res.is_empty() || (w32_res.len() == 1 && w32_res[0].0.contains("found in Win32 RAM")) {
            return WrapperLibraryInterface::get_all_aliases(verbosity);
        }
        Ok(w32_res)
    }

    fn write_autorun_registry(cmd: &str, v: &Verbosity) -> io::Result<()> {
        Win32LibraryInterface::write_autorun_registry(cmd, v)
    }

    fn read_autorun_registry() -> String {
        Win32LibraryInterface::read_autorun_registry()
    }

    // --- 2. THE CENTRALIZED LOGIC ---

    fn purge_ram_macros(verbosity: &Verbosity) -> io::Result<PurgeReport> {
        let report = Win32LibraryInterface::purge_ram_macros(verbosity)?;
        // If report has failures, let the Wrapper try to finish the job
        if !report.failed.is_empty() {
            return WrapperLibraryInterface::purge_ram_macros(verbosity);
        }
        Ok(report)
    }

    fn query_alias(name: &str, verbosity: &Verbosity) -> Vec<String> {
        let output = Win32LibraryInterface::query_alias(name, verbosity);

        // If Win32 returns the "not found" alert, ask the wrapper
        if output.get(0).map_or(false, |s| s.contains("not found") || s.contains("not a known alias")) {
            let wrap_output = WrapperLibraryInterface::query_alias(name, verbosity);
            if !wrap_output.is_empty() {
                return wrap_output;
            }
        }
        output
    }

    fn set_alias(opts: SetOptions, path: &Path, verbosity: &Verbosity) -> io::Result<()> {
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };
        let val_opt = if opts.value.is_empty() { None } else { Some(opts.value.as_str()) };

        // Attempt API strike first
        if Win32LibraryInterface::raw_set_macro(&name, val_opt).is_err() {
            WrapperLibraryInterface::set_alias(opts.clone(), path, verbosity)?;
        } else if !opts.volatile {
            // Update disk only if RAM strike was accepted and not volatile
            update_disk_file(verbosity, &name, &opts.value, path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        if opts.volatile {
            say!(verbosity, AliasIcon::Win32, "Volatile strike (Hybrid): {}", name);
        }
        Ok(())
    }

    fn run_diagnostics(path: &Path, verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        Win32LibraryInterface::run_diagnostics(path, verbosity)
    }

    fn alias_show_all(verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        if verbosity.level == VerbosityLevel::Mute { return Ok(()); }

        // 1. Try Win32
        let w32 = match Win32LibraryInterface::get_all_aliases(verbosity) {
            Ok(list) => list,
            Err(e) => {
                // MAP: Convert Box<dyn Error> into a concrete io::Error
                let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                let err_struct = failure!(verbosity, io_err);
                shout!(verbosity, AliasIcon::Fail, "{}", err_struct.message);
                Vec::new()
            }
        };

        // 2. Try Wrapper
        let wrap = match WrapperLibraryInterface::get_all_aliases(verbosity) {
            Ok(list) => list,
            Err(e) => {
                let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                let err_struct = failure!(verbosity, io_err);
                shout!(verbosity, AliasIcon::Fail, "{}", err_struct.message);
                Vec::new()
            }
        };

        // 3. Try File
        let file = match get_alias_path("") {
            Some(p) => match parse_macro_file(&p, verbosity) {
                Ok(list) => list,
                Err(e) => {
                    let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                    let err_struct = failure!(verbosity, io_err);
                    shout!(verbosity, AliasIcon::Fail, "File Error: {}", err_struct.message);
                    Vec::new()
                }
            },
            None => Vec::new(),
        };

        // 4. Final Audit
        perform_triple_audit(verbosity, w32, wrap, file, &Self::provider_type());

        Ok(())
    }
    fn provider_type() -> ProviderType {
        ProviderType::Hybrid
    }
    fn get_version() -> &'static Versioning {
        &VERSION
    }
    fn get_versions() -> Vec<&'static Versioning> {
        vec![
            alias_lib::Versioning::current(),
            WrapperLibraryInterface::get_version(),
            Win32LibraryInterface::get_version(),
            Self::get_version(),
        ]
    }
}
