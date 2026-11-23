// Auralis binary entry point.
// No console window on release builds (panics are logged to disk instead —
// see lib.rs install_panic_hook).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    auralis_lib::run();
}
