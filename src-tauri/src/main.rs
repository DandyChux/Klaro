// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod file_parser;
mod license;
mod ner;
mod pii_detector;
mod scrubber;
mod streaming;

use commands::*;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();

            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                ner::initialize_ner(&handle);
                println!("NER initialized in {:?}", start.elapsed());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            process_file,
            get_supported_formats,
            get_pii_types,
            get_version_info,
            validate_files,
            cancel_processing
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
