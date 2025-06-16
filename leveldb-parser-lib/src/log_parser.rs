use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};

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

        print_block(&block, block_counter)?;

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
    println!(
        "\n################ [ Block {} ] #################",
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

    let mut cursor = Cursor::new(&block.data);
    if block.block_type == 1 || block.block_type == 2 {
        println!("---------------- Batch Header ----------------");

        let batch_header = read_batch_header(&mut cursor)?;
        println!("Sequence Number: {}", batch_header.seq_no);
        println!("Number of Records: {}", batch_header.num_records);

        println!("---------------- Record State ----------------");
        let record_state = cursor.read_u8()?;
        match record_state {
            0 => println!("Deleted (0)"),
            1 => println!("Live (1)"),
            _ => println!("Unknown ({})", record_state),
        }

        for _ in 0..batch_header.num_records {
            match record_state {
                0 => {
                    println!("-------------------- Key ---------------------");
                    let key = read_varint32_record(&mut cursor)?;
                    println!("{:02X?}", key);
                    println!("ASCII: {}", bytes_to_ascii(&key));
                }
                1 => {
                    println!("-------------------- Key ---------------------");
                    let key = read_varint32_record(&mut cursor)?;
                    println!("{:02X?}", key);
                    println!("ASCII: {}", bytes_to_ascii(&key));

                    println!("------------------- Value --------------------");
                    let value = read_varint32_record(&mut cursor)?;
                    println!("{:02X?}", value);
                    println!("ASCII: {}", bytes_to_ascii(&value));
                }
                _ => {
                    println!("Unknown record state...");
                }
            }
        }
    } // TODO: implement other block types (3 and 4)

    // println!("-------------------- Data --------------------");
    // println!("Block Data: {:02X?}", block.data);

    Ok(())
}

// helper ----------------------------------------------------------------------

fn crc_verified(block: &Block) -> bool {
    let mut buf = Vec::with_capacity(1 + block.data.len());
    buf.push(block.block_type);
    buf.extend_from_slice(&block.data);

    let calculated_crc = crc32c(&buf);
    let unmasked_crc = utils::unmask_crc32c(block.crc);

    calculated_crc == unmasked_crc
}

fn read_varint32_record(reader: &mut (impl Read + Seek)) -> io::Result<Vec<u8>> {
    let mut varint_bytes = Vec::new();
    let mut buf = [0; 1];

    loop {
        reader.read_exact(&mut buf)?;
        varint_bytes.push(buf[0]);

        if buf[0] & 0x80 == 0 {
            break; // break if the continuation bit is not set
        }

        if varint_bytes.len() >= 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Varint32 overflow",
            ));
        }
    }

    let record_len = utils::decode_varint32(&varint_bytes) as usize;

    let mut record_data = vec![0; record_len];
    reader.read_exact(&mut record_data)?;

    Ok(record_data)
}

fn bytes_to_ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| if b.is_ascii() { b as char } else { '.' })
        .collect()
}
