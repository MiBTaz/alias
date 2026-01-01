// alias_wrapper/src/main.rs

use std::env;
use alias_lib::*;
// Swap this based on the crate:
use alias_wrapper::WrapperLibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Collect args and merge %ALIAS_OPTS%
    let mut args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        // Call your run function directly with ShowAll
        // This bypasses injection and parsing entirely.
        let path = alias_lib::get_alias_path().unwrap_or_default();
        run::<Interface>(AliasAction::ShowAll, false, &path)?;
        return Ok::<(), Box<dyn std::error::Error>>(());
    }
    if let Ok(opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = opts.split_whitespace().map(String::from).collect();
        args.splice(1..1, extra);
    }

    // 2. Parse intent and find file
    let (action, quiet) = parse_alias_args(&args);
    let path = get_alias_path().ok_or("❌ Error: No alias file found.")?;

    // 3. Static Handoff
    run::<Interface>(action, quiet, &path)
}