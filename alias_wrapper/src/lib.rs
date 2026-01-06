// alias_wrapper/src/lib.rs

use std::{env, io};
use std::error::Error;
use std::path::Path;
use std::process::Command;
use alias_lib::*;

pub struct WrapperLibraryInterface;

impl alias_lib::AliasProvider for WrapperLibraryInterface {
    /// UPDATED: Now returns io::Result to match the AliasProvider trait.
    fn get_all_aliases(verbosity: Verbosity) -> io::Result<Vec<(String, String)>> {
        let output = Command::new("doskey")
            .arg("/macros:cmd.exe")
            .output()
            .map_err(|e| {
                // If we can't even spawn doskey, that's a system error
                let err_box = scream!(Verbosity::loud(), e);
                io::Error::new(io::ErrorKind::Other, err_box.message)
            })?;

        if !output.status.success() {
            return Err(io::Error::new(io::ErrorKind::Other, "Doskey process returned an error."));
        }

        // Doskey output is usually UTF-8 in modern Windows CMD
        let stdout = String::from_utf8_lossy(&output.stdout);

        let list = stdout.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() { return None; }

                line.split_once('=').map(|(n, v)| {
                    // Sanitization: Remove quotes so RAM matches Disk during Audit
                    (n.trim_matches('"').to_string(), v.trim_matches('"').to_string())
                })
            })
            .collect();

        Ok(list)
    }

    fn query_alias(name: &str, verbosity: Verbosity) -> Vec<String> {
        trace!("Querying for: {:?} (len: {})", name, name.len());
        let search_target = name.to_lowercase();

        // FIX: Handle the Result from get_all_aliases()
        let os_list = match Self::get_all_aliases(verbosity) {
            Ok(list) => list,
            Err(e) => {
                if verbosity.level == VerbosityLevel::Normal {
                    return vec![text!(verbosity, AliasIcon::Alert, "Doskey Query Failed: {}", e)];
                }
                return vec![];
            }
        };

        for (n, v) in os_list {
            let clean_n = n.trim_matches('"').to_lowercase();
            if clean_n == search_target {
                trace!("  MATCH FOUND!");
                return vec![format!("{}={}", clean_n, v.trim_matches('"'))];
            }
        }

        if verbosity.level == VerbosityLevel::Normal {
            return vec![text!(verbosity, AliasIcon::Alert, "'{}' not found via doskey query.", name)];
        }
        vec![]
    }

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        let val = value.unwrap_or("");
        let clean_name = name.trim_matches('"');
        let clean_val = val.trim_matches('"');

        let status = Command::new("doskey")
            .args(["/exename=cmd.exe", &format!("{}={}", clean_name, clean_val)])
            .status()
            .map_err(|e| {
                let err_box = scream!(Verbosity::loud(), e);
                io::Error::new(io::ErrorKind::Other, err_box.message)
            })?;

        if !status.success() {
            let err_box = scream!(Verbosity::loud(), ErrorCode::Generic, "Doskey rejected alias: {}", clean_name);
            return Err(io::Error::new(io::ErrorKind::InvalidInput, err_box.message));
        }
        Ok(true)
    }

    fn set_alias(opts: SetOptions, path: &Path, verbosity: Verbosity) -> io::Result<()> {
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };

        if name.is_empty() {
            let err = scream!(verbosity, ErrorCode::MissingName, "Alias name cannot be empty.");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, err.message));
        }

        if !opts.volatile {
            alias_lib::update_disk_file(verbosity, &name, &opts.value, path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        // Percolate RAM/Doskey errors
        Self::raw_set_macro(&name, Some(&opts.value))?;

        let tag = if opts.volatile { "(volatile)" } else { "(saved)" };
        whisper!(verbosity, AliasIcon::Success, "Wrapper set {}: {}={}", tag, name, opts.value);
        Ok(())
    }

    fn raw_reload_from_file(path: &Path) -> io::Result<()> {
        let status = Command::new("doskey")
            .arg(format!("/macrofile={}", path.display()))
            .status()
            .map_err(|e| {
                let err_box = scream!(Verbosity::loud(), e);
                io::Error::new(io::ErrorKind::Other, err_box.message)
            })?;

        if !status.success() {
            let err_box = scream!(Verbosity::loud(), ErrorCode::Generic, "Doskey failed to load file: {}", path.display());
            return Err(io::Error::new(io::ErrorKind::Other, err_box.message));
        }
        Ok(())
    }

    fn write_autorun_registry(cmd: &str, verbosity: Verbosity) -> io::Result<()> {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(REG_SUBKEY)
            .map_err(|e| io::Error::new(e.kind(), scream!(verbosity, e).message))?;

        let existing: String = key.get_value(REG_AUTORUN_KEY).unwrap_or_default();
        if existing.contains(cmd) {
            say!(verbosity, AliasIcon::Info, "AutoRun hook is already up to date.");
            return Ok(());
        }

        let new_val = if existing.is_empty() {
            cmd.to_string()
        } else {
            format!("{} & {}", existing, cmd)
        };

        key.set_value(REG_AUTORUN_KEY, &new_val)
            .map_err(|e| io::Error::new(e.kind(), scream!(verbosity, e).message))
    }

    fn read_autorun_registry() -> String {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        hkcu.open_subkey(REG_SUBKEY)
            .and_then(|key| key.get_value(REG_AUTORUN_KEY))
            .unwrap_or_default()
    }

    fn run_diagnostics(path: &Path, verbosity: Verbosity) -> Result<(), Box<dyn Error>> {
        let report = DiagnosticReport {
            binary_path: env::current_exe().ok(),
            resolved_path: path.to_path_buf(),
            env_file: env::var(ENV_ALIAS_FILE).unwrap_or_else(|_| "NOT SET".into()),
            env_opts: env::var(ENV_ALIAS_OPTS).unwrap_or_else(|_| "NOT SET".into()),
            file_exists: path.exists(),
            is_readonly: path.metadata().map(|m| m.permissions().readonly()).unwrap_or(false),
            drive_responsive: is_drive_responsive(path),
            registry_status: check_registry_wrapper(),
            api_status: Some("SPAWNER (doskey.exe)".into()),
        };
        alias_lib::render_diagnostics(report, verbosity);
        Ok(())
    }

    fn alias_show_all(verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        // FIX: Extract the Vec from the Result using '?'
        let os_aliases = Self::get_all_aliases(verbosity)?;

        // Perform the audit and percolate any error immediately
        alias_lib::perform_audit(os_aliases, verbosity)
    }
}

fn check_registry_wrapper() -> RegistryStatus {
    let raw = WrapperLibraryInterface::read_autorun_registry();
    if raw.is_empty() {
        RegistryStatus::NotFound
    } else if raw.contains("--reload") || raw.contains("alias") {
        RegistryStatus::Synced
    } else {
        RegistryStatus::Mismatch("Found other AutoRun commands".into())
    }
}
