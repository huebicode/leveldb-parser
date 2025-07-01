use std::path::Path;
use tauri::Emitter;

use leveldb_parser_lib::ldb_parser;

#[tauri::command]
pub fn process_dropped_files(window: tauri::Window, paths: Vec<String>) {
    for path in &paths {
        let path = Path::new(path);
        if path.exists() {
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().ends_with(".ldb") {
                    let ldb_file = ldb_parser::parse_file(path.to_str().unwrap()).unwrap();
                    let csv =
                        ldb_parser::export::csv_string(&ldb_file, &file_name.to_string_lossy());
                    window.emit("ldb_csv", csv).unwrap();
                }
            }
        }
    }
}
