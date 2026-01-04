// alias_wrapper/src/lib.rs

use std::{env, io};
use std::path::Path;
use std::process::Command;
use alias_lib::*;

pub struct WrapperLibraryInterface;

impl alias_lib::AliasProvider for WrapperLibraryInterface {
    // --- 1. THE REQUIRED "HANDS" (Atomic Operations) ---

    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        let val = value.unwrap_or(""); // doskey name= clears the macro
        let status = Command::new("doskey")
            .args(["/exename=cmd.exe", &format!("{}={}", name, val)])
            .status()?;
        Ok(status.success())
    }

    fn raw_reload_from_file(path: &Path) -> io::Result<()> {
        let status = Command::new("doskey")
            .arg(format!("/macrofile={}", path.display()))
            .status()?;
        if !status.success() {
            return Err(io::Error::new(io::ErrorKind::Other, "Doskey failed to load file"));
        }
        Ok(())
    }

    fn write_autorun_registry(cmd: &str, verbosity: Verbosity) -> io::Result<()> {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(REG_SUBKEY)?;

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

        key.set_value(REG_AUTORUN_KEY, &new_val)?;
        say!(verbosity, AliasIcon::Ok, "AutoRun hook installed successfully.");
        Ok(())
    }

    fn read_autorun_registry() -> String {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        hkcu.open_subkey(REG_SUBKEY)
            .and_then(|key| key.get_value(REG_AUTORUN_KEY))
            .unwrap_or_default()
    }

    // --- 2. HIGH-LEVEL OVERRIDES (Specific to Wrapper) ---
    fn get_all_aliases() -> Vec<(String, String)> {
        let output = Command::new("doskey")
            .arg("/macros:cmd.exe")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        output.lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() { return None; }

                line.split_once('=')
                    .map(|(n, v)| {
                        // FIX: Strip literal quotes from both the name and the value
                        let name = n.trim_matches('"').to_string();
                        let val = v.trim_matches('"').to_string();
                        (name, val)
                    })
            })
            .collect()
    }

    fn query_alias(name: &str, mode: Verbosity) -> Vec<String> {
        trace!("Querying for: {:?} (len: {})", name, name.len());
        let search_target = name.to_lowercase();
        let os_list = Self::get_all_aliases();

        for (n, v) in os_list {
            // We use debug formatting {:?} to see hidden characters in the comparison
            trace!("Comparing: {:?} against {:?}", n.to_lowercase(), search_target);
            if n.to_lowercase() == search_target {
                trace!("  MATCH FOUND!");
                return vec![format!("{}={}", n, v)];
            }
        }

        trace!("  No match found in RAM.");
        if mode.level == VerbosityLevel::Normal {
            return vec![text!(mode, AliasIcon::Alert, "'{}' not found via doskey query.", name)];
        }
        vec![]
    }

    fn set_alias(opts: SetOptions, path: &Path, verbosity: Verbosity) -> io::Result<()> {
        // Ensure name preservation
        let name = if opts.force_case { opts.name.clone() } else { opts.name.to_lowercase() };

        if !opts.volatile {
            alias_lib::update_disk_file(&name, &opts.value, path)?;
        }

        // Pass the name exactly as computed above
        if Self::raw_set_macro(&name, Some(&opts.value))? {
            let tag = if opts.volatile { "(volatile)" } else { "(saved)" };
            say!(verbosity, AliasIcon::Success, "Wrapper set {}: {}={}", tag, name, opts.value);
        }
        Ok(())
    }

    fn alias_show_all(verbosity: Verbosity) {
        alias_lib::perform_audit(Self::get_all_aliases(), verbosity);
    }

    fn run_diagnostics(path: &Path, verbosity: Verbosity) {
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
    }
}

// Keep helper functions that aren't part of the trait contract
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
