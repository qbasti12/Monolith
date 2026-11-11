//! Monolith terminal client and live telemetry log viewer.

pub mod app;
pub mod ipc;
pub mod ui;

fn main() {
    println!("Monolith TUI v{}", env!("CARGO_PKG_VERSION"));
}
