//! Dmenu-style item source: newline-separated lines read from stdin.

use std::io::{self, BufRead};

use crate::unicode::utf8_normalize;

/// Read newline-separated items from an arbitrary reader.
///
/// Empty lines are skipped. When `normalize` is true each line is NFC-normalised.
pub fn read_lines_from<R: BufRead>(mut reader: R, normalize: bool) -> Vec<String> {
    let mut line = String::new();
    let mut items = Vec::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    continue;
                }
                items.push(if normalize {
                    utf8_normalize(trimmed)
                } else {
                    trimmed.to_owned()
                });
            }
            Err(_) => break,
        }
    }
    items
}

/// Read newline-separated items from stdin.
pub fn read_lines(normalize: bool) -> Vec<String> {
    read_lines_from(io::stdin().lock(), normalize)
}

#[cfg(test)]
mod tests;
