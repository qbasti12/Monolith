//! Monolith UCI chess engine binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = monolith_engine::uci::run_uci_stdio() {
        eprintln!("monolith-engine I/O error: {}", err);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
