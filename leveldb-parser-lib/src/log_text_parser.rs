use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub struct LogEntry {
    pub timestamp: String,
    pub thread_id: String,
    pub message: String,
}

pub struct LogTextFile {
    pub entries: Vec<LogEntry>,
}

pub fn parse_file(path: &str) -> Result<LogTextFile, io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.splitn(3, ' ').collect();

        if parts.len() >= 3 {
            entries.push(LogEntry {
                timestamp: parts[0].to_string(),
                thread_id: parts[1].to_string(),
                message: parts[2].to_string(),
            });
        }
    }

    Ok(LogTextFile { entries })
}

pub mod export {
    use super::*;

    pub fn csv_string(log_file: &LogTextFile, filename: &str) -> String {
        let mut csv = String::from("\"Date\",\"ThreadId\",\"Msg\",\"File\"\n");

        for entry in &log_file.entries {
            let message = entry.message.replace("\"", "\"\""); // escape quotes for CSV
            csv.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\"\n",
                entry.timestamp, entry.thread_id, message, filename
            ));
        }

        csv
    }
}
