use std::fs::File;
use std::io::{self, BufReader, Cursor, Seek};

use byteorder::ReadBytesExt;

use crate::log_parser;
use crate::utils;

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut block_counter = 1;
    while reader.stream_position()? < file_size {
        let block = log_parser::read_block(&mut reader)?;
        print_block(&block, block_counter)?;

        block_counter += 1;
    }
    Ok(())
}

fn print_block(block: &log_parser::Block, block_counter: u64) -> io::Result<()> {
    log_parser::display::print_block_header(block, block_counter)?;

    println!("-------------------- Tags --------------------");
    let mut cursor = Cursor::new(&block.data);
    while cursor.position() < block.data.len() as u64 {
        let tag = cursor.read_u8()?;

        match tag {
            0x01 => {
                let value = utils::read_varint_slice(&mut cursor)?;
                println!("[1] Comparator: {}", utils::bytes_to_ascii_with_hex(&value));
            }
            0x02 => {
                let log_no = utils::read_varint(&mut cursor)?;
                println!("[2] LogNumber: {}", log_no);
            }
            0x03 => {
                let next_file_no = utils::read_varint(&mut cursor)?;
                println!("[3] NextFileNumber: {}", next_file_no);
            }
            0x04 => {
                let last_seq_no = utils::read_varint(&mut cursor)?;
                println!("[4] LastSeq: {}", last_seq_no);
            }
            0x05 => {
                let level = utils::read_varint(&mut cursor)?;
                let pointer_key = utils::read_varint_slice(&mut cursor)?;
                let (key, stat, seq) = utils::decode_key(&pointer_key)?;

                println!(
                    "[5] CompactPointer: Level: {}, Key: {} @ {} : {}",
                    level,
                    utils::bytes_to_ascii_with_hex(&key),
                    seq,
                    stat
                );
            }
            0x06 => {
                let level = utils::read_varint(&mut cursor)?;
                let file_no = utils::read_varint(&mut cursor)?;
                println!("[6] RemoveFile: Level: {}, No.: {}", level, file_no);
            }
            0x07 => {
                let level = utils::read_varint(&mut cursor)?;
                let file_no = utils::read_varint(&mut cursor)?;
                let file_size = utils::read_varint(&mut cursor)?;

                let smallest_key = utils::read_varint_slice(&mut cursor)?;
                let (sm_key, sm_stat, sm_seq) = utils::decode_key(&smallest_key)?;

                let largest_key = utils::read_varint_slice(&mut cursor)?;
                let (lg_key, lg_stat, lg_seq) = utils::decode_key(&largest_key)?;

                println!(
                    "[7] AddFile: Level: {}, No.: {}, Size: {} Bytes, Key-Range: '{}' @ {} : {} .. '{}' @ {} : {}",
                    level,
                    file_no,
                    file_size,
                    utils::bytes_to_ascii_with_hex(&sm_key),
                    sm_seq,
                    sm_stat,
                    utils::bytes_to_ascii_with_hex(&lg_key),
                    lg_seq,
                    lg_stat
                );
            }
            0x09 => {
                let prev_log_no = utils::read_varint(&mut cursor)?;
                println!("[9] PrevLogNumber: {}", prev_log_no);
            }
            _ => println!("Unknown tag: {:02X}", tag),
        };
    }

    Ok(())
}
