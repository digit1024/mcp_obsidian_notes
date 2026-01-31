use anyhow::{Context, Result};
use std::io::{self, Read};

/// Resolve content from: "@path" (file), "-" (stdin), or literal string.
/// - `@/path/to/file` or `@file.md` - read from file
/// - `-` - read from stdin until EOF
/// - else - treat as literal (replace `\n` with newline)
pub fn resolve_content(arg: &str) -> Result<String> {
    let arg = arg.trim();
    if arg == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read from stdin")?;
        return Ok(buf);
    }
    if arg.starts_with('@') {
        let path = arg[1..].trim_start();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        return Ok(content);
    }
    // Literal: replace \n with actual newline
    Ok(arg.replace("\\n", "\n"))
}
