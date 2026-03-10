//! ABOUTME: entrypoint delegating to shared tauri library bootstrap.
//! ABOUTME: keeps binary minimal and defer logic to rumble_lib::run.
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    rumble_lib::run()
}
