use std::fs;
use std::path::Path;
use tauri::Emitter;

use leveldb_parser_lib::{ldb_parser, log_parser};

#[tauri::command]
pub fn process_dropped_files(window: tauri::Window, paths: Vec<String>) {
    for path_str in &paths {
        process_path(&window, Path::new(path_str));
    }
}

fn process_path(window: &tauri::Window, path: &Path) {
    if path.exists() {
        if path.is_dir() {
            match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        process_path(window, &entry.path());
                    }
                }
                Err(e) => println!("Error reading directory {}: {}", path.display(), e),
            }
        } else if let Some(file_name) = path.file_name() {
            if file_name.to_string_lossy().ends_with(".ldb") {
                match ldb_parser::parse_file(path.to_str().unwrap()) {
                    Ok(ldb_file) => {
                        let csv =
                            ldb_parser::export::csv_string(&ldb_file, &file_name.to_string_lossy());
                        if let Err(e) = window.emit("records_csv", csv) {
                            println!("Error emitting LDB CSV: {}", e);
                        }
                    }
                    Err(e) => println!("Error parsing LDB file {}: {:?}", path.display(), e),
                }
            } else if file_name.to_string_lossy().ends_with(".log") {
                match log_parser::parse_file(path.to_str().unwrap()) {
                    Ok(log_file) => {
                        let csv =
                            log_parser::export::csv_string(&log_file, &file_name.to_string_lossy());
                        if let Err(e) = window.emit("records_csv", csv) {
                            println!("Error emitting Log CSV: {}", e);
                        }
                    }
                    Err(e) => println!("Error parsing Log file {}: {:?}", path.display(), e),
                }
            }
        }
    }
}
