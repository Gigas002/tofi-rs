//! CLI **parameters and parsing** (`clap`) only — no app logic here. Options are **argv-only**
//! (no `#[arg(env = …)]`; migration plan §1.4).

use clap::Parser;

/// Wayland application launcher (Rust port; work in progress).
#[derive(Parser)]
#[command(name = "tofi", version, about)]
pub struct Cli {}
