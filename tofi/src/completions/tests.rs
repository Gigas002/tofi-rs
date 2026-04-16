//! Tests for shell completion generation.

use super::{CompletionShell, generate_completions};

#[test]
fn generate_bash_completions_runs() {
    generate_completions(CompletionShell::Bash);
}

#[test]
fn generate_zsh_completions_runs() {
    generate_completions(CompletionShell::Zsh);
}

#[test]
fn generate_fish_completions_runs() {
    generate_completions(CompletionShell::Fish);
}

#[test]
fn generate_nushell_completions_runs() {
    generate_completions(CompletionShell::Nushell);
}
