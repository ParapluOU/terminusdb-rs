use colored::*;
use serde_json::Value;
use std::collections::HashMap;

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Json,
    Compact,
    Pretty,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "compact" => Self::Compact,
            "pretty" => Self::Pretty,
            _ => Self::Pretty, // default
        }
    }
}

/// Color mode options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "always" => Self::Always,
            "never" => Self::Never,
            "auto" => Self::Auto,
            _ => Self::Auto,
        }
    }

    pub fn should_colorize(&self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                // Check if stdout is a TTY and colors are supported
                atty::is(atty::Stream::Stdout)
                    && supports_color::on(supports_color::Stream::Stdout).is_some()
            }
        }
    }
}

/// Extract type name from document ID
/// e.g., "User/123" -> "User", "Person/abc" -> "Person"
fn extract_type_name(id: &str) -> &str {
    id.split('/').next().unwrap_or(id)
}

/// Format a single value for display
fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else if arr.len() <= 3 {
                format!(
                    "[{}]",
                    arr.iter()
                        .map(|v| format_value(v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                format!("[{} items]", arr.len())
            }
        }
        Value::Object(_) => "{...}".to_string(),
    }
}

/// Format changed fields for a document
pub fn format_changed_fields(
    changed_fields: &HashMap<String, Value>,
    colorize: bool,
) -> Vec<String> {
    let mut lines = Vec::new();

    for (field, value) in changed_fields.iter() {
        let value_str = format_value(value);
        let line = format!("  {}: {}", field, value_str);

        if colorize {
            // For now, show all changed fields in yellow/bright
            // In a real implementation, we'd differentiate between added/removed
            lines.push(format!("  {}: {}", field.bright_yellow(), value_str));
        } else {
            lines.push(line);
        }
    }

    lines
}

/// Format a document change event
pub fn format_document_change(
    id: &str,
    action: &str,
    changed_fields: Option<&HashMap<String, Value>>,
    colorize: bool,
) -> String {
    let type_name = extract_type_name(id);
    let id_display = if colorize {
        id.bright_black().to_string()
    } else {
        id.to_string()
    };

    match action {
        "added" => {
            if colorize {
                format!("{} {} {}", "+".green().bold(), type_name.bold(), id_display)
            } else {
                format!("+ {} {}", type_name, id)
            }
        }
        "deleted" => {
            if colorize {
                format!("{} {} {}", "-".red().bold(), type_name.bold(), id_display)
            } else {
                format!("- {} {}", type_name, id)
            }
        }
        "updated" => {
            let prefix = if colorize {
                format!("{} {} {}", "~".yellow().bold(), type_name.bold(), id_display)
            } else {
                format!("~ {} {}", type_name, id)
            };

            if let Some(fields) = changed_fields {
                if fields.is_empty() {
                    prefix
                } else {
                    let mut output = vec![prefix];
                    let type_header = if colorize {
                        format!("  {} {{", type_name.bold())
                    } else {
                        format!("  {} {{", type_name)
                    };
                    output.push(type_header);

                    let field_lines = format_changed_fields(fields, colorize);
                    output.extend(field_lines);

                    output.push("  }".to_string());
                    output.join("\n")
                }
            } else {
                prefix
            }
        }
        _ => {
            if colorize {
                format!("? {} {}", type_name.bold(), id_display)
            } else {
                format!("? {} {}", type_name, id)
            }
        }
    }
}

/// Format a commit header
pub fn format_commit_header(
    commit_id: &str,
    author: &str,
    message: &str,
    timestamp: f64,
    colorize: bool,
) -> String {
    let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string());

    if colorize {
        format!(
            "{} {} {} {}\n{} {}",
            "Commit".bold(),
            commit_id.bright_cyan().bold(),
            "by".dimmed(),
            author.bright_blue(),
            "Message:".bold(),
            message.italic()
        )
    } else {
        format!(
            "Commit {} by {} at {}\nMessage: {}",
            commit_id, author, datetime, message
        )
    }
}

/// Format metadata summary
pub fn format_metadata(
    added: u64,
    updated: u64,
    deleted: u64,
    colorize: bool,
) -> String {
    let parts: Vec<String> = [
        if added > 0 {
            Some(if colorize {
                format!("{} {}", "+".green(), added)
            } else {
                format!("+{}", added)
            })
        } else {
            None
        },
        if updated > 0 {
            Some(if colorize {
                format!("{} {}", "~".yellow(), updated)
            } else {
                format!("~{}", updated)
            })
        } else {
            None
        },
        if deleted > 0 {
            Some(if colorize {
                format!("{} {}", "-".red(), deleted)
            } else {
                format!("-{}", deleted)
            })
        } else {
            None
        },
    ]
    .iter()
    .filter_map(|x| x.clone())
    .collect();

    if parts.is_empty() {
        "No changes".to_string()
    } else {
        parts.join(" ")
    }
}
