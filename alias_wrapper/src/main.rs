// alias_wrapper/src/main.rs

use std::env;
use std::process::Command;
use alias_lib::*;
use alias_wrapper::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();

    #[cfg(debug_assertions)]
    {
        eprintln!("--------------------------------------------------");
        eprintln!("DEBUG [Raw OS Input]: {:?}", args);
        eprintln!("--------------------------------------------------");
    }

    if let Ok(env_opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = env_opts.split_whitespace().map(String::from).collect();
        if !extra.is_empty() {
            args.splice(1..1, extra);
        }
    }
    let (action, quiet) = parse_alias_args(&args);

    let alias_path = match get_alias_path() {
        Some(path) => path,
        None => {
            return Err(format!(
                "❌ Error: No usable alias file found. Set %{}% or create '{}' in APPDATA.",
                ENV_ALIAS_FILE, DEFAULT_ALIAS_FILENAME
            ).into());
        }
    };
    let mode = OutputMode::set_quiet(quiet);

    match action {
        AliasAction::Clear => {
            qprintln!(quiet, "🧹 Clearing RAM macros...");
            clear_ram_macros()?;
            qprintln!(quiet, "✨ RAM is now empty.");
        },
        AliasAction::Reload => {
            qprintln!(quiet, "🔄 Syncing RAM with {}...", alias_path.display());
            reload_full(&alias_path)?;
            qprintln!(quiet, "✨ Reload complete.");
        },
        AliasAction::ShowAll => {
            Command::new("doskey").arg("/macros:all").status()?;
        }
        AliasAction::Query(term) => {
            // query_alias(&term, &alias_path)?;
            let _ = print_results(query_alias(&term, mode));
        }
        AliasAction::Set { name, value } => {
            if value.is_empty() {
                // Remove from RAM immediately
                Command::new("doskey").arg(format!("{}=", name)).status()?;
            }
            set_alias(&name, &value, &alias_path, quiet)?;
        }
        AliasAction::Edit(custom_editor) => {
            open_editor(&alias_path, custom_editor, quiet)?;
            reload_full(&alias_path)?;
            qprintln!(quiet, "✨ Aliases reloaded after edit.");
        }
        AliasAction::Which => run_diagnostics(&alias_path),
        AliasAction::Help => print_help(HelpMode::Full, Some(&alias_path)),
        AliasAction::Invalid => {
            eprintln!("❌ Invalid command.");
            print_help(HelpMode::Short, Some(&alias_path));
        }
        AliasAction::Setup => {
            qprintln!(quiet, "🛠️  Setting up Windows AutoRun hook...");
            if let Err(e) = install_autorun(quiet) {
                eprintln!("❌ Setup failed: {}", e);
            } else {
                qprintln!(quiet, "✅ Success! Your aliases are now global.");
            }
        }
    }

    Ok(())
}

