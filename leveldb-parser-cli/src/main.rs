use std::path::Path;

use leveldb_parser_lib::ldb_parser;
use leveldb_parser_lib::log_parser;
use leveldb_parser_lib::manifest_parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <[.log, .ldb, MANIFEST-] file>", args[0]);
        return;
    }

    let file_path = &args[1];
    let path = Path::new(file_path);

    let file_name = match path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => {
            println!("Invalid file path");
            return;
        }
    };

    if file_name.ends_with(".ldb") {
        ldb_parser::parse_file(file_path).unwrap();
    } else if file_name.ends_with(".log") {
        log_parser::parse_file(file_path).unwrap();
    } else if file_name.starts_with("MANIFEST-") {
        manifest_parser::parse_file(file_path).unwrap();
    } else {
        println!("Unsupported file type: {}", file_path);
    }
}
