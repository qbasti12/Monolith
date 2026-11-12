//! Monolith chess client (GUI and TUI) and live telemetry log viewer library.

pub mod analysis;
pub mod app;
pub mod engines;
pub mod entrypoint;
pub mod gui;
pub mod gui_board;
pub mod gui_panels;
pub mod input;
pub mod ipc;
pub mod logs;
pub mod modals;
pub mod theme;
pub mod ui;

pub use entrypoint::entrypoint;
