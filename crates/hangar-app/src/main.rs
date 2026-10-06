#![cfg_attr(windows, no_std)]
#![cfg_attr(windows, no_main)]
#![cfg_attr(windows, windows_subsystem = "windows")]
extern crate alloc;
#[cfg(not(windows))]
mod cli;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "linux.rs"]
mod platform;
mod saving;
mod ui;
#[cfg(not(windows))]
fn main() {
    if let Err(e) = cli::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
