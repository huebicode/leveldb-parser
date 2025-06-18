use std::fs::File;
use std::io::{self, BufReader, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let (meta_blk_hndl, idx_blk_hndl) = read_footer(&mut reader)?;

    println!("-------------- Meta Index Block --------------");
    read_raw_block(&mut reader, meta_blk_hndl.offset, meta_blk_hndl.size)?;

    println!("---------------- Index Block -----------------");
    read_raw_block(&mut reader, idx_blk_hndl.offset, idx_blk_hndl.size)?;
    Ok(())
}

struct BlockHandle {
    offset: u64,
    size: u64,
}

fn read_footer(reader: &mut (impl Read + Seek)) -> io::Result<(BlockHandle, BlockHandle)> {
    println!("------------------- Footer -------------------");
    reader.seek(io::SeekFrom::End(-48))?;

    let meta_blk_hndl = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };
    println!(
        "MetaBlock-Handle: offset: {:?}, size: {:?}",
        meta_blk_hndl.offset, meta_blk_hndl.size
    );

    let idx_blk_hndl = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };
    println!(
        "IndexBlock-Handle: offset: {:?}, size: {:?}",
        idx_blk_hndl.offset, idx_blk_hndl.size
    );

    const EXPECTED_MAGIC: [u8; 8] = [0x57, 0xFB, 0x80, 0x8B, 0x24, 0x75, 0x47, 0xDB];
    reader.seek(io::SeekFrom::End(-8))?;
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;

    let is_valid = magic == EXPECTED_MAGIC;
    println!(
        "Magic: {:02X?} {}",
        magic,
        if is_valid { "(valid)" } else { "(invalid!)" }
    );

    Ok((meta_blk_hndl, idx_blk_hndl))
}

struct RawBlock {
    data: Vec<u8>,
    compression_type: u8,
    crc: u32,
}

fn read_raw_block(reader: &mut (impl Read + Seek), offset: u64, size: u64) -> io::Result<RawBlock> {
    reader.seek(io::SeekFrom::Start(offset))?;

    // data
    let mut data = vec![0; size as usize];
    reader.read_exact(&mut data)?;

    // compression
    let compression_type = reader.read_u8()?;
    match compression_type {
        0x0 => println!("Compression-Type: 0 (NoCompression)"),
        0x1 => println!("Compression-Type: 1 (Snappy)"),
        0x2 => println!("Compression-Type: 2 (Zstd)"),
        _ => println!("Compression-Type: {} (Unknown)", compression_type),
    }

    // TODO: decompress data

    // crc
    let crc = reader.read_u32::<LittleEndian>()?;
    if utils::crc_verified(crc, &data, compression_type, true) {
        println!("CRC32C: {:02X} (verified)", crc);
    } else {
        println!("CRC32C: {:02X} (verification failed!)", crc);
    }

    Ok(RawBlock {
        data,
        compression_type,
        crc,
    })
}
