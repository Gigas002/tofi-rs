//! Shell completion generation (feature **`completions`**).
//!
//! Enabled with `--features completions` at build time.  Not included in
//! default builds to keep the binary free of the generator dependency.
//!
//! # Usage
//!
//! ```sh
//! # bash  (~/.bash_completion.d/tofi or /usr/share/bash-completion/completions/tofi)
//! tofi --completions bash > ~/.bash_completion.d/tofi
//!
//! # zsh  (somewhere in $fpath, e.g. /usr/share/zsh/site-functions/_tofi)
//! tofi --completions zsh > ~/.local/share/zsh/site-functions/_tofi
//!
//! # fish  (~/.config/fish/completions/tofi.fish)
//! tofi --completions fish > ~/.config/fish/completions/tofi.fish
//!
//! # nushell  (add to $nu.completion-path, typically ~/.config/nushell/completions/)
//! tofi --completions nushell > ~/.config/nushell/completions/tofi.nu
//! ```

use clap::CommandFactory as _;
use clap_complete::generate;

use crate::cli::Cli;

/// Supported shells for completion generation.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Nushell,
}

/// Generate a completion script for `shell` and write it to stdout.
pub fn generate_completions(shell: CompletionShell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    let stdout = &mut std::io::stdout();
    match shell {
        CompletionShell::Bash => generate(clap_complete::Shell::Bash, &mut cmd, name, stdout),
        CompletionShell::Zsh => generate(clap_complete::Shell::Zsh, &mut cmd, name, stdout),
        CompletionShell::Fish => generate(clap_complete::Shell::Fish, &mut cmd, name, stdout),
        CompletionShell::Nushell => {
            generate(clap_complete_nushell::Nushell, &mut cmd, name, stdout)
        }
    }
}
