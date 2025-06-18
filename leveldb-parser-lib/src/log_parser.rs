use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    let mut block_counter = 1;
    while reader.stream_position()? < file_size {
        let block = read_block(&mut reader)?;
        print_block(&block, block_counter)?;

        block_counter += 1;
    }
    Ok(())
}

// -----------------------------------------------------------------------------

pub struct Block {
    pub crc: u32,
    pub data_len: u16,
    pub block_type: u8,
    pub data: Vec<u8>,
}

pub fn read_block(reader: &mut (impl Read + Seek)) -> io::Result<Block> {
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

struct BatchHeader {
    seq_no: u64,
    num_records: u32,
}

fn read_batch_header(reader: &mut (impl Read + Seek)) -> io::Result<BatchHeader> {
    let seq_no = reader.read_u64::<LittleEndian>()?;
    let num_records = reader.read_u32::<LittleEndian>()?;

    Ok(BatchHeader {
        seq_no,
        num_records,
    })
}

fn print_block(block: &Block, block_counter: u64) -> io::Result<()> {
    print_block_header(block, block_counter)?;
    print_block_data(block)?;
    Ok(())
}

pub fn print_block_header(block: &Block, block_counter: u64) -> io::Result<()> {
    println!(
        "\n################ [ Block {} ] #################",
        block_counter
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

fn print_block_data(block: &Block) -> io::Result<()> {
    let mut cursor = Cursor::new(&block.data);
    if block.block_type == 1 || block.block_type == 2 {
        println!("---------------- Batch Header ----------------");

        let batch_header = read_batch_header(&mut cursor)?;
        println!("Sequence-No.: {}", batch_header.seq_no);
        println!("Records-No.: {}", batch_header.num_records);

        println!("---------------- Record State ----------------");
        let record_state = cursor.read_u8()?;
        match record_state {
            0 => println!("0 (Deleted)"),
            1 => println!("1 (Live)"),
            _ => println!("{} (Unknown)", record_state),
        }

        for _ in 0..batch_header.num_records {
            match record_state {
                0 => {
                    println!("-------------------- Key ---------------------");
                    let key = utils::read_varint_slice(&mut cursor)?;
                    println!("{:02X?}", key);
                    println!("ASCII: {}", utils::bytes_to_ascii(&key));
                }
                1 => {
                    println!("-------------------- Key ---------------------");
                    let key = utils::read_varint_slice(&mut cursor)?;
                    println!("{:02X?}", key);
                    println!("ASCII: {}", utils::bytes_to_ascii(&key));

                    println!("------------------- Value --------------------");
                    let value = utils::read_varint_slice(&mut cursor)?;
                    println!("{:02X?}", value);
                    println!("ASCII: {}", utils::bytes_to_ascii(&value));
                }
                _ => {
                    println!("Unknown record state...");
                }
            }
        }
    } else if block.block_type == 3 || block.block_type == 4 {
        println!("------------------- Value --------------------");
        println!("{:02X?}", block.data);
        println!("ASCII: {}", utils::bytes_to_ascii(&block.data));
    }

    Ok(())
}
