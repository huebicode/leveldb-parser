use std::fs::File;
use std::io::{self, BufReader, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};
use crc32c::crc32c;

use crate::utils;

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut block_counter = 1;
    while reader.stream_position()? < file_size {
        let block = read_block(&mut reader)?;

        print_block(&block, block_counter);

        block_counter += 1;
    }
    Ok(())
}

// -----------------------------------------------------------------------------

struct Block {
    crc: u32,
    data_len: u16,
    block_type: u8,
    data: Vec<u8>,
}

fn read_block(reader: &mut (impl Read + Seek)) -> io::Result<Block> {
    let crc = reader.read_u32::<LittleEndian>()?;
    let data_len = reader.read_u16::<LittleEndian>()?;
    let block_type = reader.read_u8()?;

    let mut data = vec![0; data_len as usize];
    reader.read_exact(&mut data)?;

    Ok(Block {
        crc,
        data_len,
        block_type,
        data,
    })
}

// -----------------------------------------------------------------------------

fn print_block(block: &Block, block_counter: u64) {
    println!(
        "################ [ Block {} ] #################",
        block_counter
    );

    println!("------------------- Header -------------------");

    if crc_verified(block) {
        println!("CRC32C: {:02X} (verified)", block.crc);
    } else {
        println!("CRC32C: {:02X} (verification failed!)", block.crc);
    }

    println!("Data Length: {} Bytes", block.data_len);

    match block.block_type {
        0 => println!("Record Type: Zero (0)"),
        1 => println!("Record Type: Full (1)"),
        2 => println!("Record Type: First (2)"),
        3 => println!("Record Type: Middle (3)"),
        4 => println!("Record Type: Last (4)"),
        _ => println!("Record Type: Unknown ({})", block.block_type),
    }

    println!("-------------------- Data --------------------");

    println!("Block Data: {:02X?}", block.data);
}

// helper ----------------------------------------------------------------------

fn crc_verified(block: &Block) -> bool {
    let mut buffer = Vec::with_capacity(1 + block.data.len());
    buffer.push(block.block_type);
    buffer.extend_from_slice(&block.data);

    let calculated_crc = crc32c(&buffer);
    let unmasked_crc = utils::unmask_crc32c(block.crc);

    calculated_crc == unmasked_crc
}
