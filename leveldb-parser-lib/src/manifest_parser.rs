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
    log_parser::print_block_header(block, block_counter)?;

    println!("-------------------- Tags --------------------");
    let mut cursor = Cursor::new(&block.data);
    while cursor.position() < block.data.len() as u64 {
        let tag = cursor.read_u8()?;

        match tag {
            0x01 => {
                let value = utils::read_varint_slice(&mut cursor)?;
                println!("Comparator (1): {}", utils::bytes_to_ascii(&value));
            }
            // 0x02 => "LogNumber",
            // 0x03 => "NextFileNumber",
            // 0x04 => "LastSequenceNumber",
            // 0x05 => "CompactPointer",
            // 0x06 => "DeletedFile",
            0x07 => {
                let level = utils::read_varint(&mut cursor)?;
                let file_no = utils::read_varint(&mut cursor)?;
                let file_size = utils::read_varint(&mut cursor)?;

                let smallest_key = utils::read_varint_slice(&mut cursor)?;
                let (sm_key, sm_stat, sm_seq) = utils::decode_key(&smallest_key)?;

                let largest_key = utils::read_varint_slice(&mut cursor)?;
                let (lg_key, lg_stat, lg_seq) = utils::decode_key(&largest_key)?;

                println!(
                    "AddFile (7): Level: {}, File-No.: {}, File-Size: {}, Key-Range: {}@{}:{} .. {}@{}:{}",
                    level,
                    file_no,
                    file_size,
                    utils::bytes_to_ascii(&sm_key),
                    sm_seq,
                    sm_stat,
                    utils::bytes_to_ascii(&lg_key),
                    lg_seq,
                    lg_stat
                );
            }
            // 0x09 => "PrevLogNumber",
            _ => println!("Unknown tag: {:02X}", tag),
        };
    }

    // println!("------------------- Value --------------------");
    // println!("{:02X?}", block.data);
    // println!("ASCII: {}", utils::bytes_to_ascii(&block.data));

    Ok(())
}
