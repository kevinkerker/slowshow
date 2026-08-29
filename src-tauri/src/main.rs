// Verhindert ein zusätzliches Konsolenfenster im Windows-Release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    slowshow_lib::run()
}
