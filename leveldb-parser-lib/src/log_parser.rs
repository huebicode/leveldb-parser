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

fn print_block_data(block: &Block) -> io::Result<()> {
    let mut cursor = Cursor::new(&block.data);
    if block.block_type == 1 || block.block_type == 2 {
        println!("---------------- Batch Header ----------------");

        let batch_header = read_batch_header(&mut cursor)?;
        println!("Sequence: {}", batch_header.seq_no);
        println!("Records: {}", batch_header.num_records);

        for i in 0..batch_header.num_records {
            let record_state = cursor.read_u8()?;

            match record_state {
                0 => println!("\n******* Record {} (State: 0 [Deleted]) ********", i + 1),
                1 => println!("\n********* Record {} (State: 1 [Live]) *********", i + 1),
                _ => println!("\n*********** Unknown Record State *************"),
            }

            // calc key offset from block start + header + cursor position
            let key_offset = block.offset + 7 + cursor.position();

            println!(
                "-------------- Key (Offset: {}) ---------------",
                key_offset
            );
            let key = utils::read_varint_slice(&mut cursor)?;
            println!("{}", utils::bytes_to_ascii_with_hex(&key));

            if record_state != 0 {
                println!("------------------- Value --------------------");
                let value = utils::read_varint_slice(&mut cursor)?;
                println!("{}", utils::bytes_to_ascii_with_hex(&value));
            }
        }
    } else if block.block_type == 3 || block.block_type == 4 {
        println!("------------------- Value --------------------");
        println!("{}", utils::bytes_to_ascii_with_hex(&block.data));
    }

    Ok(())
}
