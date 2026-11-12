//! Monolith TUI / GUI client executable entrypoint.

use std::process::ExitCode;

fn main() -> ExitCode {
    monolith_tui::entrypoint()
}
