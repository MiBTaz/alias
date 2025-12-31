// alias_win32/src/main.rs

use std::env;

use alias_lib::*;

use alias_win32::{api_purge_all_macros, api_show_all, install_autorun, query_alias, reload_full, set_alias};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();

    // 1. Inject Environment Options (Consolidated)
    if let Ok(opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = opts.split_whitespace().map(String::from).collect();
        if !extra.is_empty() { args.splice(1..1, extra); }
    }

    let (action, quiet) = parse_alias_args(&args);
    let alias_path = get_alias_path().ok_or_else(|| {
        format!("❌ Error: No usable alias file found. Set %{}%.", ENV_ALIAS_FILE)
    })?;
    let mode = OutputMode::set_quiet(quiet);

    match action {
        AliasAction::Clear => {
            qprintln!(quiet, "🧹 Clearing RAM...");
            api_purge_all_macros(quiet);
        },
        AliasAction::Reload => reload_full(&alias_path, quiet)?,
        AliasAction::ShowAll => api_show_all(),
        AliasAction::Query(term) => {  let _ = print_results(query_alias(&term, mode)); },
        AliasAction::Set { name, value } => set_alias(&name, &value, &alias_path, quiet)?,
        AliasAction::Edit(ed) => {
            open_editor(&alias_path, ed, quiet)?;
            reload_full(&alias_path, quiet)?;
        }
        AliasAction::Which => println!("--- 🛠️ Diagnostics ---\nFile: {}", alias_path.display()),
        AliasAction::Help => print_help(HelpMode::Full, Some(&alias_path)),
        AliasAction::Setup => install_autorun(quiet)?,
        AliasAction::Invalid => { eprintln!("❌ Invalid command."); print_help(HelpMode::Short, Some(&alias_path)); }
    }
    Ok(())
}

