use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

// -----------------------------------------------------------------------------
const BLOCK_SIZE: u64 = 32768;
const HEADER_SIZE: u64 = 7; // CRC + Data Length + Block Type
// -----------------------------------------------------------------------------

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut block_counter = 1;
    let mut partial_block_data = Vec::new();

    let mut first_block_offset = 0;

    while reader.stream_position()? < file_size {
        let block = read_block(&mut reader)?;

        print_block_header(&block, block_counter)?;

        match block.block_type {
            1 => {
                // Full Block
                process_batch(&block.data, block.offset)?;
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
                process_batch(&partial_block_data, first_block_offset)?;
                partial_block_data.clear();
            }
            _ => {
                // Zero Block or Unknown Type
                println!("Block Type {} is not processed.", block.block_type);
            }
        }

        block_counter += 1;
    }
    Ok(())
}

// -----------------------------------------------------------------------------

pub struct Block {
    pub offset: u64,
    pub crc: u32,
    pub data_len: u16,
    pub block_type: u8,
    pub data: Vec<u8>,
}

pub fn read_block(reader: &mut (impl Read + Seek)) -> io::Result<Block> {
    let offset = reader.stream_position()?;

    let crc = reader.read_u32::<LittleEndian>()?;
    let data_len = reader.read_u16::<LittleEndian>()?;
    let block_type = reader.read_u8()?;

    let mut data = vec![0; data_len as usize];
    reader.read_exact(&mut data)?;

    Ok(Block {
        offset,
        crc,
        data_len,
        block_type,
        data,
    })
}

pub fn print_block_header(block: &Block, block_counter: u64) -> io::Result<()> {
    println!(
        "\n########## [ Block {} (Offset: {})] ############",
        block_counter, block.offset
    );

    println!("------------------- Header -------------------");
    if utils::crc_verified(block.crc, &block.data, block.block_type, false) {
        println!("CRC32C: {:02X} (verified)", block.crc);
    } else {
        println!("CRC32C: {:02X} (verification failed!)", block.crc);
    }

    println!("Data-Length: {} Bytes", block.data_len);

    match block.block_type {
        0 => println!("Record-Type: 0 (Zero)"),
        1 => println!("Record-Type: 1 (Full)"),
        2 => println!("Record-Type: 2 (First)"),
        3 => println!("Record-Type: 3 (Middle)"),
        4 => println!("Record-Type: 4 (Last)"),
        _ => println!("Record-Type: {} (Unknown)", block.block_type),
    }

    Ok(())
}

fn process_batch(data: &[u8], offset: u64) -> io::Result<()> {
    println!("\n//////////////// Batch Header ////////////////");

    let mut cursor = Cursor::new(data);
    let batch_header = read_batch_header(&mut cursor)?;
    println!("Seq: {}", batch_header.seq_no);
    println!("Records: {}", batch_header.rec_count);

    let mut offset_adjust = 0;
    for i in 0..batch_header.rec_count {
        if cursor.position() >= data.len() as u64 {
            println!("Unexpected end of data at record: {}", i + 1);
            break;
        }

        let bounds_crossed =
            process_record(&mut cursor, offset + (offset_adjust * HEADER_SIZE), i)?;

        // if block boundaries crossed, adjust offset for next record
        if bounds_crossed > 0 {
            offset_adjust += bounds_crossed;
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------

struct BatchHeader {
    seq_no: u64,
    rec_count: u32,
}

fn read_batch_header(reader: &mut (impl Read + Seek)) -> io::Result<BatchHeader> {
    let seq_no = reader.read_u64::<LittleEndian>()?;
    let rec_count = reader.read_u32::<LittleEndian>()?;

    Ok(BatchHeader { seq_no, rec_count })
}

fn process_record(cursor: &mut Cursor<&[u8]>, block_offset: u64, i: u32) -> io::Result<u64> {
    println!("\n****************** Record {} ******************", i + 1);
    let record_state = cursor.read_u8()?;
    println!(
        "State: {}",
        match record_state {
            0 => "0 (Deleted)",
            1 => "1 (Live)",
            _ => "Unknown",
        },
    );

    let (key, key_offset, key_bounds_crossed) = read_entry_with_offset(cursor, block_offset)?;

    let mut total_bounds_crossed = key_bounds_crossed;

    // if key crossed block boundaries, adjust offset for value entry
    let adjusted_block_offset = if key_bounds_crossed > 0 {
        block_offset + (key_bounds_crossed * HEADER_SIZE)
    } else {
        block_offset
    };

    println!(
        "Key (Offset: {}, Size: {}): '{}'",
        key_offset,
        key.len(),
        utils::bytes_to_ascii_with_hex(&key)
    );

    if record_state != 0 {
        let (value, value_offset, val_bounds_crossed) =
            read_entry_with_offset(cursor, adjusted_block_offset)?;

        total_bounds_crossed += val_bounds_crossed;

        println!(
            "Val (Offset: {}, Size: {}): '{}'",
            value_offset,
            value.len(),
            utils::bytes_to_ascii_with_hex(&value)
        );
    }

    Ok(total_bounds_crossed)
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
