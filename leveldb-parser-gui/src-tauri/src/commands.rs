use std::fs;
use std::path::Path;
use tauri::Emitter;

use rayon::prelude::*;
use std::sync::Arc;

use leveldb_parser_lib::{ldb_parser, log_parser, log_text_parser, manifest_parser};

#[tauri::command]
pub fn process_dropped_files(window: tauri::Window, paths: Vec<String>) {
    let window = Arc::new(window);

    let mut all_paths = Vec::new();
    for path_str in &paths {
        collect_paths(Path::new(path_str), &mut all_paths);
    }

    all_paths.par_iter().for_each(|path| {
        let window = Arc::clone(&window);
        process_single_file(&window, path);
    });
}

fn collect_paths(path: &Path, result: &mut Vec<std::path::PathBuf>) {
    if path.exists() {
        if path.is_dir() {
            match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        collect_paths(&entry.path(), result);
                    }
                }
                Err(e) => println!("Error reading directory {}: {}", path.display(), e),
            }
        } else {
            result.push(path.to_path_buf());
        }
    }
}

fn process_single_file(window: &tauri::Window, path: &Path) {
    if let Some(file_name) = path.file_name() {
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.ends_with(".ldb") {
            match ldb_parser::parse_file(path.to_str().unwrap()) {
                Ok(ldb_file) => {
                    let csv = ldb_parser::export::csv_string(&ldb_file, &file_name_str);
                    if let Err(e) = window.emit("records_csv", csv) {
                        println!("Error emitting LDB CSV: {}", e);
                    }
                }
                Err(e) => println!("Error parsing LDB file {}: {:?}", path.display(), e),
            }
        } else if file_name_str.ends_with(".log") {
            match log_parser::parse_file(path.to_str().unwrap()) {
                Ok(log_file) => {
                    let csv = log_parser::export::csv_string(&log_file, &file_name_str);
                    if let Err(e) = window.emit("records_csv", csv) {
                        println!("Error emitting Log CSV: {}", e);
                    }
                }
                Err(e) => println!("Error parsing Log file {}: {:?}", path.display(), e),
            }
        } else if file_name_str.starts_with("MANIFEST-") {
            match manifest_parser::parse_file(path.to_str().unwrap()) {
                Ok(manifest_file) => {
                    let csv = manifest_parser::export::csv_string(&manifest_file, &file_name_str);
                    if let Err(e) = window.emit("manifest_csv", csv) {
                        println!("Error emitting Manifest CSV: {}", e);
                    }
                }
                Err(e) => println!("Error parsing Manifest file {}: {:?}", path.display(), e),
            }
        } else if file_name_str.starts_with("LOG") {
            match log_text_parser::parse_file(path.to_str().unwrap()) {
                Ok(log_text_file) => {
                    let csv = log_text_parser::export::csv_string(&log_text_file, &file_name_str);
                    if let Err(e) = window.emit("log_text_csv", csv) {
                        println!("Error emitting LOG text CSV: {}", e);
                    }
                }
                Err(e) => println!("Error parsing LOG text file {}: {:?}", path.display(), e),
            }
        }
    }
}
