use std::io;
use std::path::Path;
use std::process;

use leveldb_parser_lib::{ldb_parser, log_parser, manifest_parser};

fn main() {
    if let Err(e) = run() {
        if let Some(io_error) = e.downcast_ref::<io::Error>() {
            match io_error.kind() {
                // exit silently for broken pipe errors (common with less/more)
                io::ErrorKind::BrokenPipe => {
                    process::exit(0);
                }
                _ => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        } else {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let use_print_all = args.contains(&"-a".to_string());
    let file_path = args.iter().skip(1).find(|arg| !arg.starts_with('-'));

    let file_path = match file_path {
        Some(path) => path,
        None => {
            println!("Usage: {} [-a] <file>", args[0]);
            println!("  -a     print all details (default is CSV format)");
            println!("  file   .log, .ldb or MANIFEST file to parse");
            return Ok(());
        }
    };

    let path = Path::new(file_path);

    let abs_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            println!("Error: File does not exist: {}", path.display());
            return Ok(());
        }
    };

    let file_name = if !path.exists() {
        println!("Error: File does not exist: {}", path.display());
        return Ok(());
    } else if let Some(name) = path.file_name() {
        name.to_string_lossy()
    } else {
        println!("Error: File name needed.");
        return Ok(());
    };

    if file_name.ends_with(".ldb") {
        let ldb_file = ldb_parser::parse_file(abs_path.to_str().unwrap())?;
        if use_print_all {
            ldb_parser::display::print_all(&ldb_file)?;
        } else {
            ldb_parser::display::print_csv(&ldb_file)?;
        }
    } else if file_name.ends_with(".log") {
        let log_file = log_parser::parse_file(abs_path.to_str().unwrap())?;
        if use_print_all {
            log_parser::display::print_all(&log_file)?;
        } else {
            log_parser::display::print_csv(&log_file)?;
        }
    } else if file_name.starts_with("MANIFEST-") {
        let manifest_file = manifest_parser::parse_file(abs_path.to_str().unwrap())?;
        if use_print_all {
            manifest_parser::display::print_all(&manifest_file)?;
        } else {
            manifest_parser::display::print_csv(&manifest_file)?;
        }
    } else {
        println!("Error: Unsupported file type: {}", file_path);
    }

    Ok(())
}
