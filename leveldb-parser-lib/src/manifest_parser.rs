use std::fs::File;
use std::io::{self, BufReader, Cursor, Seek};

use byteorder::ReadBytesExt;

use crate::log_parser;
use crate::utils;

pub struct ManifestFile {
    pub blocks: Vec<log_parser::Block>, // raw blocks
    pub entries: Vec<ManifestEntrySet>, // logical entry sets
}

pub struct ManifestEntrySet {
    pub entries: Vec<ManifestEntry>,
    pub offset: u64,
}

pub enum ManifestEntry {
    Comparator(Vec<u8>),
    LogNumber(u64),
    NextFileNumber(u64),
    LastSeq(u64),
    CompactPointer {
        level: u64,
        key: Vec<u8>,
        seq: u64,
        state: u8,
    },
    RemoveFile {
        level: u64,
        file_no: u64,
    },
    AddFile {
        level: u64,
        file_no: u64,
        file_size: u64,
        sm_key: Vec<u8>,
        sm_seq: u64,
        sm_state: u8,
        lg_key: Vec<u8>,
        lg_seq: u64,
        lg_state: u8,
    },
    PrevLogNumber(u64),
    Unknown(u8),
}

pub fn parse_file(file_path: &str) -> io::Result<ManifestFile> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut blocks = Vec::new();
    let mut entries = Vec::new();
    let mut partial_block_data = Vec::new();
    let mut first_block_offset = 0;

    while reader.stream_position()? < file_size {
        let block = log_parser::read_block(&mut reader)?;

        match block.block_type {
            1 => {
                // Full Block
                let entry_set = parse_entries(&block.data, block.offset)?;
                entries.push(entry_set);
            }
            2 => {
                // First Block
                first_block_offset = block.offset;
                partial_block_data.clear();
                partial_block_data.extend_from_slice(&block.data);
            }
            3 => {
                // Middle Block
                partial_block_data.extend_from_slice(&block.data);
            }
            4 => {
                // Last Block
                partial_block_data.extend_from_slice(&block.data);
                let entry_set = parse_entries(&partial_block_data, first_block_offset)?;
                entries.push(entry_set);
                partial_block_data.clear();
            }
            _ => {} // Zero Block or Unknown Type - no entries
        }

        blocks.push(block);
    }

    Ok(ManifestFile { blocks, entries })
}

fn parse_entries(data: &[u8], offset: u64) -> io::Result<ManifestEntrySet> {
    let mut result_entries = Vec::new();
    let mut cursor = Cursor::new(data);

    while cursor.position() < data.len() as u64 {
        let tag = cursor.read_u8()?;

        let entry = match tag {
            0x01 => {
                let value = utils::read_varint_slice(&mut cursor)?;
                ManifestEntry::Comparator(value)
            }
            0x02 => {
                let log_no = utils::read_varint(&mut cursor)?;
                ManifestEntry::LogNumber(log_no)
            }
            0x03 => {
                let next_file_no = utils::read_varint(&mut cursor)?;
                ManifestEntry::NextFileNumber(next_file_no)
            }
            0x04 => {
                let last_seq_no = utils::read_varint(&mut cursor)?;
                ManifestEntry::LastSeq(last_seq_no)
            }
            0x05 => {
                let level = utils::read_varint(&mut cursor)?;
                let pointer_key = utils::read_varint_slice(&mut cursor)?;
                let (key, state, seq) = utils::decode_key(&pointer_key)?;

                ManifestEntry::CompactPointer {
                    level,
                    key,
                    seq,
                    state,
                }
            }
            0x06 => {
                let level = utils::read_varint(&mut cursor)?;
                let file_no = utils::read_varint(&mut cursor)?;
                ManifestEntry::RemoveFile { level, file_no }
            }
            0x07 => {
                let level = utils::read_varint(&mut cursor)?;
                let file_no = utils::read_varint(&mut cursor)?;
                let file_size = utils::read_varint(&mut cursor)?;

                let smallest_key = utils::read_varint_slice(&mut cursor)?;
                let (sm_key, sm_state, sm_seq) = utils::decode_key(&smallest_key)?;

                let largest_key = utils::read_varint_slice(&mut cursor)?;
                let (lg_key, lg_state, lg_seq) = utils::decode_key(&largest_key)?;

                ManifestEntry::AddFile {
                    level,
                    file_no,
                    file_size,
                    sm_key,
                    sm_seq,
                    sm_state,
                    lg_key,
                    lg_seq,
                    lg_state,
                }
            }
            0x09 => {
                let prev_log_no = utils::read_varint(&mut cursor)?;
                ManifestEntry::PrevLogNumber(prev_log_no)
            }
            _ => ManifestEntry::Unknown(tag),
        };

        result_entries.push(entry);
    }

    Ok(ManifestEntrySet {
        entries: result_entries,
        offset,
    })
}

pub mod display {
    use super::*;
    use std::io::Write;

    pub fn print_all(manifest: &ManifestFile) -> io::Result<()> {
        let mut current_entry_idx = 0;

        for (i, block) in manifest.blocks.iter().enumerate() {
            log_parser::display::print_block_header(block, i as u64 + 1)?;

            match block.block_type {
                1 => {
                    // Full block
                    if current_entry_idx < manifest.entries.len() {
                        print_entries(&manifest.entries[current_entry_idx])?;
                        current_entry_idx += 1;
                    }
                }
                4 => {
                    // Last block
                    if current_entry_idx < manifest.entries.len() {
                        print_entries(&manifest.entries[current_entry_idx])?;
                        current_entry_idx += 1;
                    }
                }
                _ => {} // other block types
            }
        }

        Ok(())
    }

    fn print_entries(entry_set: &ManifestEntrySet) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n-------------------- Tags --------------------"
        )?;

        for entry in &entry_set.entries {
            print_entry(entry)?;
        }

        Ok(())
    }

    fn print_entry(entry: &ManifestEntry) -> io::Result<()> {
        match entry {
            ManifestEntry::Comparator(value) => {
                writeln!(
                    io::stdout(),
                    "[1] Comparator: {}",
                    utils::bytes_to_latin1_with_hex(value)
                )
            }
            ManifestEntry::LogNumber(log_no) => {
                writeln!(io::stdout(), "[2] LogNumber: {}", log_no)
            }
            ManifestEntry::NextFileNumber(next_file_no) => {
                writeln!(io::stdout(), "[3] NextFileNumber: {}", next_file_no)
            }
            ManifestEntry::LastSeq(last_seq_no) => {
                writeln!(io::stdout(), "[4] LastSeq: {}", last_seq_no)
            }
            ManifestEntry::CompactPointer {
                level,
                key,
                seq,
                state,
            } => {
                writeln!(
                    io::stdout(),
                    "[5] CompactPointer: Level: {}, Key: {} @ {} : {}",
                    level,
                    utils::bytes_to_latin1_with_hex(key),
                    seq,
                    state
                )
            }
            ManifestEntry::RemoveFile { level, file_no } => {
                writeln!(
                    io::stdout(),
                    "[6] RemoveFile: Level: {}, No.: {}",
                    level,
                    file_no
                )
            }
            ManifestEntry::AddFile {
                level,
                file_no,
                file_size,
                sm_key,
                sm_seq,
                sm_state,
                lg_key,
                lg_seq,
                lg_state,
            } => {
                writeln!(
                    io::stdout(),
                    "[7] AddFile: Level: {}, No.: {}, Size: {} Bytes, Key-Range: '{}' @ {} : {} .. '{}' @ {} : {}",
                    level,
                    file_no,
                    file_size,
                    utils::bytes_to_latin1_with_hex(sm_key),
                    sm_seq,
                    sm_state,
                    utils::bytes_to_latin1_with_hex(lg_key),
                    lg_seq,
                    lg_state
                )
            }
            ManifestEntry::PrevLogNumber(prev_log_no) => {
                writeln!(io::stdout(), "[9] PrevLogNumber: {}", prev_log_no)
            }
            ManifestEntry::Unknown(tag) => {
                writeln!(io::stdout(), "Unknown tag: {:02X}", tag)
            }
        }
    }

    pub fn print_csv(manifest: &ManifestFile) -> io::Result<()> {
        // Header
        writeln!(io::stdout(), "\"tag\",\"value\"")?;

        for entry_set in &manifest.entries {
            for entry in &entry_set.entries {
                let (tag, value) = match entry {
                    ManifestEntry::Comparator(value) => {
                        ("Comparator", utils::bytes_to_latin1_with_hex(value))
                    }
                    ManifestEntry::LogNumber(log_no) => ("LogNumber", format!("{}", log_no)),
                    ManifestEntry::NextFileNumber(next_file_no) => {
                        ("NextFileNumber", format!("{}", next_file_no))
                    }
                    ManifestEntry::LastSeq(last_seq_no) => ("LastSeq", format!("{}", last_seq_no)),
                    ManifestEntry::CompactPointer {
                        level,
                        key,
                        seq,
                        state,
                    } => (
                        "CompactPointer",
                        format!(
                            "Level: {}, Key: {} @ {} : {}",
                            level,
                            utils::bytes_to_latin1_with_hex(key),
                            seq,
                            state
                        ),
                    ),
                    ManifestEntry::RemoveFile { level, file_no } => {
                        ("RemoveFile", format!("Level: {}, No.: {}", level, file_no))
                    }
                    ManifestEntry::AddFile {
                        level,
                        file_no,
                        file_size,
                        sm_key,
                        sm_seq,
                        sm_state,
                        lg_key,
                        lg_seq,
                        lg_state,
                    } => (
                        "AddFile",
                        format!(
                            "Level: {}, No.: {}, Size: {} Bytes, Key-Range: '{}' @ {} : {} .. '{}' @ {} : {}",
                            level,
                            file_no,
                            file_size,
                            utils::bytes_to_latin1_with_hex(sm_key),
                            sm_seq,
                            sm_state,
                            utils::bytes_to_latin1_with_hex(lg_key),
                            lg_seq,
                            lg_state
                        ),
                    ),
                    ManifestEntry::PrevLogNumber(prev_log_no) => {
                        ("PrevLogNumber", format!("{}", prev_log_no))
                    }
                    ManifestEntry::Unknown(tag) => ("Unknown", format!("{:02X}", tag)),
                };

                let escaped_value = value.replace("\"", "\"\"");

                writeln!(io::stdout(), "\"{}\",\"{}\"", tag, escaped_value)?;
            }
        }

        Ok(())
    }
}

pub mod export {
    use super::*;

    pub fn csv_string(manifest: &ManifestFile, filename: &str) -> String {
        let mut csv = String::new();
        // Header
        csv.push_str("\"Tag\",\"TagValue\",\"CRC\",\"BlockOffset\",\"File\"\n");

        let block_crc_map: std::collections::HashMap<u64, bool> = manifest
            .blocks
            .iter()
            .map(|block| (block.offset, block.crc_valid))
            .collect();

        for entry_set in &manifest.entries {
            let block_offset = entry_set.offset;
            let crc_valid = block_crc_map.get(&block_offset).copied().unwrap_or(false);
            let crc_status = if crc_valid { "valid" } else { "failed!" };

            for entry in &entry_set.entries {
                let (tag, value) = match entry {
                    ManifestEntry::Comparator(value) => {
                        ("Comparator", utils::bytes_to_latin1_with_hex(value))
                    }
                    ManifestEntry::LogNumber(log_no) => ("LogNumber", format!("{}", log_no)),
                    ManifestEntry::NextFileNumber(next_file_no) => {
                        ("NextFileNumber", format!("{}", next_file_no))
                    }
                    ManifestEntry::LastSeq(last_seq_no) => ("LastSeq", format!("{}", last_seq_no)),
                    ManifestEntry::CompactPointer {
                        level,
                        key,
                        seq,
                        state,
                    } => (
                        "CompactPointer",
                        format!(
                            "Level: {}, Key: {} @ {} : {}",
                            level,
                            utils::bytes_to_latin1_with_hex(key),
                            seq,
                            state
                        ),
                    ),
                    ManifestEntry::RemoveFile { level, file_no } => {
                        ("RemoveFile", format!("Level: {}, No.: {}", level, file_no))
                    }
                    ManifestEntry::AddFile {
                        level,
                        file_no,
                        file_size,
                        sm_key,
                        sm_seq,
                        sm_state,
                        lg_key,
                        lg_seq,
                        lg_state,
                    } => (
                        "AddFile",
                        format!(
                            "Level: {}, No.: {}, Size: {} Bytes, Key-Range: '{}' @ {} : {} .. '{}' @ {} : {}",
                            level,
                            file_no,
                            file_size,
                            utils::bytes_to_latin1_with_hex(sm_key),
                            sm_seq,
                            sm_state,
                            utils::bytes_to_latin1_with_hex(lg_key),
                            lg_seq,
                            lg_state
                        ),
                    ),
                    ManifestEntry::PrevLogNumber(prev_log_no) => {
                        ("PrevLogNumber", format!("{}", prev_log_no))
                    }
                    ManifestEntry::Unknown(tag) => ("Unknown", format!("{:02X}", tag)),
                };

                // Escape quotes in the value
                let escaped_value = value.replace("\"", "\"\"");

                csv.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    tag, escaped_value, crc_status, block_offset, filename
                ));
            }
        }

        csv
    }
}
