use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom, Write};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

// -----------------------------------------------------------------------------
const BLOCK_SIZE: u64 = 32768;
const HEADER_SIZE: u64 = 7; // CRC + Data Length + Block Type
// -----------------------------------------------------------------------------
pub struct LogFile {
    pub blocks: Vec<Block>,
    pub batches: Vec<Batch>,
}

pub struct Block {
    pub offset: u64,
    pub crc: u32,
    pub crc_valid: bool,
    pub data_len: u16,
    pub block_type: u8,
    pub data: Vec<u8>,
}

pub struct Batch {
    pub header: BatchHeader,
    pub records: Vec<Record>,
    pub offset: u64,
}

pub struct BatchHeader {
    pub seq_no: u64,
    pub rec_count: u32,
}

pub struct Record {
    pub seq: u64,
    pub state: u8,
    pub key: Vec<u8>,
    pub key_offset: u64,
    pub value: Option<Vec<u8>>,
    pub value_offset: Option<u64>,
}
// -----------------------------------------------------------------------------
pub fn parse_file(file_path: &str) -> io::Result<LogFile> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut blocks = Vec::new();
    let mut batches = Vec::new();
    let mut partial_block_data = Vec::new();
    let mut first_block_offset = 0;

    while reader.stream_position()? < file_size {
        let block = match read_raw_block(&mut reader) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        match block.block_type {
            1 => {
                // Full Block
                if let Ok(batch) = parse_batch(&block.data, block.offset) {
                    batches.push(batch);
                }
            }
            2 => {
                // First Block
                first_block_offset = block.offset;
                partial_block_data.clear();
                partial_block_data.extend_from_slice(&block.data);
            }
            3 => {
                // Middle Block
                if !partial_block_data.is_empty() {
                    partial_block_data.extend_from_slice(&block.data);
                }
            }
            4 => {
                // Last Block
                if !partial_block_data.is_empty() {
                    partial_block_data.extend_from_slice(&block.data);
                    if let Ok(batch) = parse_batch(&partial_block_data, first_block_offset) {
                        batches.push(batch);
                    }
                    partial_block_data.clear();
                }
            }
            _ => {} // Zero Block or Unknown Type => ignore
        }

        blocks.push(block);
    }

    Ok(LogFile { blocks, batches })
}
// -----------------------------------------------------------------------------
pub fn read_raw_block(reader: &mut (impl Read + Seek)) -> io::Result<Block> {
    loop {
        let offset = reader.stream_position()?;
        let pos_in_block = offset % BLOCK_SIZE;
        let bytes_left = BLOCK_SIZE - pos_in_block;

        // not enough space for a header => skip trailer to next 32 KiB boundary.
        if bytes_left < HEADER_SIZE {
            reader.seek(SeekFrom::Current(bytes_left as i64))?;
            continue;
        }

        // read header
        let crc = reader.read_u32::<LittleEndian>()?;
        let data_len = reader.read_u16::<LittleEndian>()? as u64;
        let block_type = reader.read_u8()?;

        // padding / trailer marker
        if data_len == 0 && block_type == 0 {
            reader.seek(SeekFrom::Current((bytes_left - HEADER_SIZE) as i64))?;
            continue;
        }

        // declared payload would cross boundary => skip rest of this 32 KiB chunk
        if HEADER_SIZE + data_len > bytes_left {
            reader.seek(SeekFrom::Current((bytes_left - HEADER_SIZE) as i64))?;
            continue;
        }

        // read payload
        let mut data = vec![0u8; data_len as usize];
        reader.read_exact(&mut data)?;

        let crc_valid = utils::crc_verified(crc, &data, block_type, false);

        return Ok(Block {
            offset,
            crc,
            crc_valid,
            data_len: data_len as u16,
            block_type,
            data,
        });
    }
}

fn parse_batch(data: &[u8], offset: u64) -> io::Result<Batch> {
    let mut cursor = Cursor::new(data);
    let header = read_batch_header(&mut cursor)?;

    let mut records = Vec::with_capacity(header.rec_count as usize);
    let mut offset_adjust = 0;

    for i in 0..header.rec_count {
        if cursor.position() >= data.len() as u64 {
            break; // EOF
        }

        let record_seq = header.seq_no + i as u64;
        let (record, bounds_crossed) = parse_record(
            &mut cursor,
            offset + (offset_adjust * HEADER_SIZE),
            record_seq,
        )?;

        records.push(record);

        // if block boundaries crossed, adjust offset for next record
        if bounds_crossed > 0 {
            offset_adjust += bounds_crossed;
        }
    }

    Ok(Batch {
        header,
        records,
        offset,
    })
}
// -----------------------------------------------------------------------------
fn read_batch_header(reader: &mut (impl Read + Seek)) -> io::Result<BatchHeader> {
    let seq_no = reader.read_u64::<LittleEndian>()?;
    let rec_count = reader.read_u32::<LittleEndian>()?;

    Ok(BatchHeader { seq_no, rec_count })
}

fn parse_record(
    cursor: &mut Cursor<&[u8]>,
    block_offset: u64,
    seq: u64,
) -> io::Result<(Record, u64)> {
    let state = cursor.read_u8()?;

    let (key, key_offset, key_bounds_crossed) = read_entry_with_offset(cursor, block_offset)?;

    let mut total_bounds_crossed = key_bounds_crossed;

    // if key crossed block boundaries, adjust offset for value entry
    let adjusted_block_offset = if key_bounds_crossed > 0 {
        block_offset + (key_bounds_crossed * HEADER_SIZE)
    } else {
        block_offset
    };

    let (value, value_offset) = if state != 0 {
        let (value, val_offset, val_bounds_crossed) =
            read_entry_with_offset(cursor, adjusted_block_offset)?;

        total_bounds_crossed += val_bounds_crossed;
        (Some(value), Some(val_offset))
    } else {
        (None, None)
    };

    let record = Record {
        seq,
        state,
        key,
        key_offset,
        value,
        value_offset,
    };

    Ok((record, total_bounds_crossed))
}

// -----------------------------------------------------------------------------

fn read_entry_with_offset(
    cursor: &mut Cursor<&[u8]>,
    block_offset: u64,
) -> io::Result<(Vec<u8>, u64, u64)> {
    // get entry length
    let len = utils::read_varint(cursor)?;

    // calc entry offset
    let current_pos = cursor.position();
    let start_block = current_pos / BLOCK_SIZE;
    let offset = current_pos + block_offset + HEADER_SIZE + (start_block * HEADER_SIZE);

    // get entry data
    let data = utils::read_slice(cursor, len as usize)?;

    // calc crossed bounds count
    let start_pos = offset - HEADER_SIZE;
    let start_block = start_pos / BLOCK_SIZE;

    let end_pos = start_pos + len as u64;
    let end_block = end_pos / BLOCK_SIZE;

    let bounds_crossed = end_block.saturating_sub(start_block);

    Ok((data, offset, bounds_crossed))
}
// -----------------------------------------------------------------------------
pub mod display {
    use super::*;
    // -----------------------------------------------------------------------------
    pub fn print_all(log: &LogFile) -> io::Result<()> {
        let mut current_batch_idx = 0;

        for (i, block) in log.blocks.iter().enumerate() {
            print_block_header(block, i as u64 + 1)?;

            match block.block_type {
                1 => {
                    // Full block
                    if current_batch_idx < log.batches.len() {
                        print_batch(&log.batches[current_batch_idx])?;
                        current_batch_idx += 1;
                    }
                }
                4 => {
                    // Last block
                    if current_batch_idx < log.batches.len() {
                        print_batch(&log.batches[current_batch_idx])?;
                        current_batch_idx += 1;
                    }
                }
                _ => {} // other block types
            }
        }

        Ok(())
    }

    pub fn print_block_header(block: &Block, block_counter: u64) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n########## [ Block {} (Offset: {})] ############",
            block_counter,
            block.offset
        )?;

        writeln!(
            io::stdout(),
            "------------------- Header -------------------"
        )?;
        if block.crc_valid {
            writeln!(io::stdout(), "CRC32C: {:02X} (verified)", block.crc)?;
        } else {
            writeln!(
                io::stdout(),
                "CRC32C: {:02X} (verification failed!)",
                block.crc
            )?;
        }

        writeln!(io::stdout(), "Data-Length: {} Bytes", block.data_len)?;

        match block.block_type {
            0 => writeln!(io::stdout(), "Record-Type: 0 (Zero)")?,
            1 => writeln!(io::stdout(), "Record-Type: 1 (Full)")?,
            2 => writeln!(io::stdout(), "Record-Type: 2 (First)")?,
            3 => writeln!(io::stdout(), "Record-Type: 3 (Middle)")?,
            4 => writeln!(io::stdout(), "Record-Type: 4 (Last)")?,
            _ => writeln!(io::stdout(), "Record-Type: {} (Unknown)", block.block_type)?,
        }

        Ok(())
    }

    pub fn print_batch(batch: &Batch) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n//////////////// Batch Header ////////////////"
        )?;
        writeln!(io::stdout(), "Seq: {}", batch.header.seq_no)?;
        writeln!(io::stdout(), "Records: {}", batch.header.rec_count)?;

        for (i, record) in batch.records.iter().enumerate() {
            print_record(record, i as u32)?;
        }

        Ok(())
    }

    pub fn print_record(record: &Record, index: u32) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n****************** Record {} ******************",
            index + 1
        )?;
        writeln!(
            io::stdout(),
            "Seq: {}, State: {}",
            record.seq,
            match record.state {
                0 => "0 (Deleted)",
                1 => "1 (Live)",
                _ => "Unknown",
            },
        )?;

        writeln!(
            io::stdout(),
            "Key (Offset: {}, Size: {}): '{}'",
            record.key_offset,
            record.key.len(),
            utils::bytes_to_ascii_with_hex(&record.key)
        )?;

        if let (Some(value), Some(value_offset)) = (&record.value, record.value_offset) {
            writeln!(
                io::stdout(),
                "Val (Offset: {}, Size: {}): '{}'",
                value_offset,
                value.len(),
                utils::bytes_to_ascii_with_hex(value)
            )?;
        }

        Ok(())
    }
    // -----------------------------------------------------------------------------
    pub fn print_csv(log: &LogFile) -> io::Result<()> {
        // Header
        writeln!(io::stdout(), "\"seq\",\"state\",\"key\",\"value\"")?;

        for batch in &log.batches {
            for record in &batch.records {
                let state_str = match record.state {
                    0 => "Deleted",
                    1 => "Live",
                    _ => "Unknown",
                };

                let key_str = utils::bytes_to_ascii_with_hex(&record.key);
                let key_str = key_str.replace("\"", "\"\"");

                let value_str = if let Some(value) = &record.value {
                    let vs = utils::bytes_to_ascii_with_hex(value);
                    vs.replace("\"", "\"\"")
                } else {
                    "".to_string()
                };

                writeln!(
                    io::stdout(),
                    "\"{}\",\"{}\",\"{}\",\"{}\"",
                    record.seq,
                    state_str,
                    key_str,
                    value_str
                )?;
            }
        }

        Ok(())
    }
}

pub mod export {
    use super::*;

    pub fn csv_string(log: &LogFile, filename: &str, file_path: &str) -> String {
        let mut csv = String::new();
        // Header
        csv.push_str("\"Seq\",\"K\",\"V\",\"Cr\",\"St\",\"BO\",\"C\",\"F\",\"FP\"\n");

        for batch in &log.batches {
            // find the block that contains this batch
            let containing_block = log.blocks.iter().find(|block| block.offset == batch.offset);

            let crc_status = if let Some(block) = containing_block {
                if block.crc_valid { "valid" } else { "failed!" }
            } else {
                "unknown" // shouldn't happen
            };

            for record in &batch.records {
                let state_str = match record.state {
                    0 => "deleted",
                    1 => "live",
                    _ => "unknown",
                };

                let key_str = utils::bytes_to_utf8_lossy(&record.key).replace("\"", "\"\"");

                let value_str = if let Some(value) = &record.value {
                    utils::bytes_to_utf8_lossy(value).replace("\"", "\"\"")
                } else {
                    "".to_string()
                };

                csv.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    record.seq,
                    key_str,
                    value_str,
                    crc_status,
                    state_str,
                    batch.offset,
                    "false", // .log files don't use compression
                    filename,
                    file_path,
                ));
            }
        }

        csv
    }
}
