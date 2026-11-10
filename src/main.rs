use std::process::ExitCode;

use remit::diag::{to_json, to_text};

const HELP: &str = "\
remit - checker for the Remit contract language

Usage:
  remit check [--json] <file>...   Check contracts. Exit code 1 if any rule is broken.
  remit explain <file>...          Print each grant in plain language.
  remit --version

Files named by `use` lines are loaded automatically.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        Some((c, rest)) => (c.as_str(), rest),
        None => {
            print!("{}", HELP);
            return ExitCode::from(2);
        }
    };
    match command {
        "--version" | "-V" => {
            println!("remit {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "--help" | "-h" | "help" => {
            print!("{}", HELP);
            ExitCode::SUCCESS
        }
        "check" | "explain" => {
            let json = rest.iter().any(|a| a == "--json");
            let files: Vec<String> = rest.iter().filter(|a| !a.starts_with("--")).cloned().collect();
            if files.is_empty() {
                eprintln!("remit {}: name at least one file.", command);
                return ExitCode::from(2);
            }
            let mut read = |path: &str| std::fs::read_to_string(path).map_err(|e| e.to_string());
            let world = remit::load(&files, &mut read);
            if command == "check" {
                if json {
                    println!("{}", to_json(&world.diags));
                } else if world.diags.is_empty() {
                    println!("ok: {} file(s) checked, no rules broken.", world.files.len());
                } else {
                    print!("{}", to_text(&world.diags));
                    println!("{} problem(s) found.", world.diags.len());
                }
            } else {
                if !world.diags.is_empty() {
                    // Never describe a grant that did not pass the checker.
                    eprint!("{}", to_text(&world.diags));
                    eprintln!("Not explained: fix the problems above first.");
                    return ExitCode::from(1);
                }
                for &f in &world.entries {
                    print!("{}", remit::explain::explain(&world, f));
                }
            }
            if world.diags.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        other => {
            eprintln!("remit: unknown command `{}`.\n\n{}", other, HELP);
            ExitCode::from(2)
        }
    }
}
