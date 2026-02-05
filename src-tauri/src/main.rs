// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod file_parser;
mod license;
mod pii_detector;
mod scrubber;
mod streaming;

use commands::*;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            process_file,
            get_supported_formats,
            get_pii_types
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
