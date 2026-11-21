//! higiri-cli (v0.5.0): Higiri's TODOs from the command line, for Claude Code
//! and the like. Everything is in `higiri_lib::cli`.
//!
//! A console program of its own: the app (`Higiri.exe`) has no console on
//! Windows (`windows_subsystem = "windows"`).

fn main() -> std::process::ExitCode {
    higiri_lib::cli::main()
}
