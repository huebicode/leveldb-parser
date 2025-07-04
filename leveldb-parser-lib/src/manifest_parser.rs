use std::fs::File;
use std::io::{self, BufReader, Cursor, Seek};

use byteorder::ReadBytesExt;

use crate::log_parser;
use crate::utils;

pub struct ManifestFile {
    pub blocks: Vec<ManifestBlock>,
}

pub struct ManifestBlock {
    pub block: log_parser::Block,
    pub entries: Vec<ManifestEntry>,
    pub block_no: u64,
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
    let mut block_no = 1;

    while reader.stream_position()? < file_size {
        let block = log_parser::read_block(&mut reader)?;
        let entries = parse_block(&block)?;

        blocks.push(ManifestBlock {
            block,
            entries,
            block_no,
        });

        block_no += 1;
    }

    Ok(ManifestFile { blocks })
}

fn parse_block(block: &log_parser::Block) -> io::Result<Vec<ManifestEntry>> {
    let mut entries = Vec::new();
    let mut cursor = Cursor::new(&block.data);

    while cursor.position() < block.data.len() as u64 {
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

        entries.push(entry);
    }

    Ok(entries)
}

pub mod display {
    use super::*;
    use std::io::Write;

    pub fn print_all(manifest: &ManifestFile) -> io::Result<()> {
        for block in &manifest.blocks {
            print_manifest_block(block)?;
        }
        Ok(())
    }

    pub fn print_manifest_block(block: &ManifestBlock) -> io::Result<()> {
        log_parser::display::print_block_header(&block.block, block.block_no)?;

        writeln!(
            io::stdout(),
            "-------------------- Tags --------------------"
        )?;
        for entry in &block.entries {
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

        for block in &manifest.blocks {
            for entry in &block.entries {
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
