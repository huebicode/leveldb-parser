use std::fs::File;
use std::io::{self, BufReader, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut block_counter = 0;
    while reader.stream_position()? < file_size {
        println!(
            "----------------- [ Block {} ] -----------------",
            block_counter
        );

        let block = read_block(&mut reader)?;

        println!("CRC (masked): {:02X}", block.crc);
        println!("Data Length: {}", block.data_len);
        println!("Record Type: {}", block.block_type);
        println!("Block Data: {:02X?}", block.data);

        block_counter += 1;
    }

    Ok(())
}

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
