//! Monolith binary alias entrypoint (`monolith`).

use std::process::ExitCode;

fn main() -> ExitCode {
    monolith_tui::entrypoint()
}
