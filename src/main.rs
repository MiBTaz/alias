use std::env;
use std::fs::{OpenOptions, read_to_string};
use std::io::Write;
use std::process::Command;

const ALIAS_FILE: &str = r"V:\lbin\aliases";

fn main() {
    // Collect CLI args (index 0 is the program name)
    let args: Vec<String> = env::args().collect();

    match args.len() {
        // CONDITION 1: No arguments -> alias.exe
        1 => {
            let output = Command::new("doskey")
                .arg("/macros:all")
                .output()
                .expect("Failed to execute doskey");
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }

        // CONDITION 2: One argument -> alias.exe name
        2 => {
            let search_term = &args[1];
            if let Ok(content) = read_to_string(ALIAS_FILE) {
                let found = content.lines()
                    .find(|line| line.to_lowercase().starts_with(&format!("{}=", search_term.to_lowercase())));

                match found {
                    Some(line) => println!("{}", line),
                    None => println!("Alias \"{}\" not found.", search_term),
                }
            }
        }

        // CONDITION 3: Multiple arguments -> alias.exe x=y or alias.exe x y
        _ => {
            // Join all args starting from index 1 into a single string
            let input = args[1..].join(" ");

            // Split by either '=' or ' ' to get key and value
            let parts: Vec<&str> = input.splitn(2, |c| c == '=' || c == ' ').collect();

            if parts.len() == 2 {
                let name = parts[0].trim();
                let value = parts[1].trim();

                // Append to file
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(ALIAS_FILE)
                    .expect("Cannot open alias file");

                writeln!(file, "{}={}", name, value).expect("Failed to write to file");

                // Reload doskey
                Command::new("doskey")
                    .arg(format!("/macrofile={}", ALIAS_FILE))
                    .status()
                    .expect("Failed to reload doskey");

                println!("{} set to: {}", name, value);
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum AliasAction {
    ShowAll,
    Query(String),
    Set { name: String, value: String },
    Invalid,
}

fn parse_alias_args(args: &[String]) -> AliasAction {
    match args.len() {
        1 => AliasAction::ShowAll,
        2 => AliasAction::Query(args[1].clone()),
        n if n >= 3 => {
            // Join args into one string to handle both 'x y' and 'x=y'
            let input = args[1..].join(" ");
            let parts: Vec<&str> = input.splitn(2, |c| c == '=' || c == ' ').collect();

            if parts.len() == 2 {
                let name = parts[0].trim().to_string();
                let value = parts[1].trim().to_string();
                if name.is_empty() {
                    AliasAction::Invalid
                } else {
                    AliasAction::Set { name, value }
                }
            } else {
                AliasAction::Invalid
            }
        }
        _ => AliasAction::Invalid,
    }
}
