//! Command implementations that convert typed options to CLI arguments.

use crate::run_terminusdb;
use std::process::ExitStatus;

/// Convert options to CLI arguments.
pub(crate) trait ToArgs {
    fn to_args(&self) -> Vec<String>;
}

/// Helper to add a flag if condition is true.
pub(crate) fn add_flag(args: &mut Vec<String>, flag: &str, condition: bool) {
    if condition {
        args.push(flag.to_string());
    }
}

/// Helper to add an option with a value.
pub(crate) fn add_option<T: AsRef<str>>(args: &mut Vec<String>, flag: &str, value: &Option<T>) {
    if let Some(v) = value {
        args.push(flag.to_string());
        args.push(v.as_ref().to_string());
    }
}

/// Helper to add a required option.
pub(crate) fn add_required<T: AsRef<str>>(args: &mut Vec<String>, flag: &str, value: T) {
    args.push(flag.to_string());
    args.push(value.as_ref().to_string());
}

/// Execute a command with the given arguments.
pub(crate) fn execute(args: Vec<String>) -> std::io::Result<ExitStatus> {
    let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_terminusdb(&args_str)
}
