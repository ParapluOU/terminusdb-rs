use std::env;
use std::process;
use terminusdb_bin::run_terminusdb;

fn main() {
    // Collect all arguments except the program name
    let args: Vec<String> = env::args().skip(1).collect();

    // Convert to &str for the run_terminusdb function
    let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    // Run TerminusDB with the provided arguments
    match run_terminusdb(&args_str) {
        Ok(status) => {
            // Exit with the same code as TerminusDB
            process::exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Error running TerminusDB: {}", e);
            process::exit(1);
        }
    }
}
