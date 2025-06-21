use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

pub fn parse_file(file_path: &str) -> io::Result<()> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let (meta_idx_blk_hndl, idx_blk_hndl) = read_footer(&mut reader)?;

    println!("\n############## Meta Index Block ##############");
    let meta_idx_blk = read_raw_block(
        &mut reader,
        meta_idx_blk_hndl.offset,
        meta_idx_blk_hndl.size,
    )?;
    let kv = read_block_data_kvs(&meta_idx_blk.data)?;

    for (idx, pair) in kv.iter().enumerate() {
        print_record_kv(pair, idx, BlockType::MetaIndex)?;

        let meta_blk_hndl = parse_block_handle(&pair.value)?;
        println!(
            "\n########## Meta Block {} (Offset: {}) ###########",
            idx + 1,
            meta_blk_hndl.offset,
        );

        let meta_blk = read_raw_block(&mut reader, meta_blk_hndl.offset, meta_blk_hndl.size)?;

        if utils::bytes_to_ascii_with_hex(&pair.key) == "filter.leveldb.BuiltinBloomFilter2" {
            parse_bloom_filter_block(&meta_blk.data)?;
        }
    }

    println!("\n################ Index Block #################");
    let idx_blk = read_raw_block(&mut reader, idx_blk_hndl.offset, idx_blk_hndl.size)?;
    let kv = read_block_data_kvs(&idx_blk.data)?;

    for (idx, pair) in kv.iter().enumerate() {
        print_record_kv(pair, idx, BlockType::Index)?;

        let data_blk_hndl = parse_block_handle(&pair.value)?;
        println!(
            "\n########## Data Block {} (Offset: {}) ##########",
            idx + 1,
            data_blk_hndl.offset
        );

        let data_blk = read_raw_block(&mut reader, data_blk_hndl.offset, data_blk_hndl.size)?;

        let kv = read_block_data_kvs(&data_blk.data)?;
        for (idx, pair) in kv.iter().enumerate() {
            print_record_kv(pair, idx, BlockType::Data)?;
        }
    }

    Ok(())
}

struct BlockHandle {
    offset: u64,
    size: u64,
}

fn read_footer(reader: &mut (impl Read + Seek)) -> io::Result<(BlockHandle, BlockHandle)> {
    println!("################### Footer ###################");
    reader.seek(io::SeekFrom::End(-48))?;

    let meta_blk_hndl = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };
    println!(
        "BlockHandle (Meta Index Block): Offset: {:?}, Size: {:?}",
        meta_blk_hndl.offset, meta_blk_hndl.size
    );

    let idx_blk_hndl = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };
    println!(
        "BlockHandle (Index Block): Offset: {:?}, Size: {:?}",
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
    _compression_type: u8,
    _crc: u32,
}

fn read_raw_block(reader: &mut (impl Read + Seek), offset: u64, size: u64) -> io::Result<RawBlock> {
    reader.seek(io::SeekFrom::Start(offset))?;

    // data
    let mut data = vec![0; size as usize];
    reader.read_exact(&mut data)?;

    // compression
    let compression_type = reader.read_u8()?;
    match compression_type {
        0x0 => println!("CompressionType: 0 (NoCompression)"),
        0x1 => println!("CompressionType: 1 (Snappy)"),
        0x2 => println!("CompressionType: 2 (Zstd)"),
        _ => println!("CompressionType: {} (Unknown)", compression_type),
    }

    // crc
    let crc = reader.read_u32::<LittleEndian>()?;
    if utils::crc_verified(crc, &data, compression_type, true) {
        println!("CRC32C: {:02X} (verified)", crc);
    } else {
        println!("CRC32C: {:02X} (verification failed!)", crc);
    }

    // decompress data after crc-check
    if compression_type == 0x1 {
        let decompressed = snap::raw::Decoder::new().decompress_vec(&data)?;
        data = decompressed;
    } else if compression_type == 0x2 {
        // NOTE: not tested yet
        let decompressed = zstd::decode_all(data.as_slice())?;
        data = decompressed;
    }

    Ok(RawBlock {
        data,
        _compression_type: compression_type,
        _crc: crc,
    })
}

fn read_block_data_kvs(data: &[u8]) -> io::Result<Vec<KeyValPair>> {
    println!("----------------- Block Data -----------------");
    let mut cursor = Cursor::new(data);

    // restart array
    cursor.seek(io::SeekFrom::End(-4))?;
    let restart_arr_len = cursor.read_u32::<LittleEndian>()?;
    let restart_array_offset = cursor.seek(io::SeekFrom::End(-4 - (4 * restart_arr_len as i64)))?;
    println!(
        "RestartArray (Count: {}, Offset: {})",
        restart_arr_len, restart_array_offset
    );

    // read block entries
    let mut entries = Vec::new();
    let mut prev_key = Vec::new();

    cursor.seek(io::SeekFrom::Start(0))?;
    while cursor.position() < restart_array_offset {
        match read_block_entry(&mut cursor, &prev_key) {
            Ok(entry) => {
                prev_key = entry.key.clone();
                entries.push(entry);
            }
            Err(e) => {
                eprintln!("Error reading block entry: {}", e);
                break;
            }
        }
    }

    Ok(entries)
}

struct KeyValPair {
    key: Vec<u8>,
    value: Vec<u8>,
}

fn read_block_entry(cursor: &mut Cursor<&[u8]>, prev_key: &[u8]) -> io::Result<KeyValPair> {
    let shared_len = utils::read_varint(cursor)? as usize;
    let non_shared_len = utils::read_varint(cursor)? as usize;
    let value_len = utils::read_varint(cursor)? as usize;

    // read inline key
    let mut inline_key = vec![0; non_shared_len];
    cursor.read_exact(&mut inline_key)?;

    // construct full key
    let mut key = Vec::with_capacity(shared_len + non_shared_len);

    if shared_len > 0 && shared_len <= prev_key.len() {
        key.extend_from_slice(&prev_key[0..shared_len]);
    }

    key.extend_from_slice(&inline_key);

    // value
    let mut value = vec![0; value_len];
    cursor.read_exact(&mut value)?;

    Ok(KeyValPair { key, value })
}

fn parse_bloom_filter_block(data: &[u8]) -> io::Result<()> {
    println!("\n**************** Bloom Filter ****************");
    let mut cursor = Cursor::new(data);

    cursor.seek(io::SeekFrom::End(-5))?;
    let array_offset = cursor.read_u32::<LittleEndian>()?;
    let base_log = cursor.read_u8()?;
    let filter_data = &data[0..array_offset as usize];

    println!("FilterData: {:02X?}", filter_data);
    println!("ArrayOffset: {}", array_offset);
    println!("BaseLog: {}", base_log);

    Ok(())
}

// helper ----------------------------------------------------------------------

fn parse_block_handle(data: &[u8]) -> io::Result<BlockHandle> {
    let mut cursor = Cursor::new(data);
    let offset = utils::read_varint(&mut cursor)?;
    let size = utils::read_varint(&mut cursor)?;
    Ok(BlockHandle { offset, size })
}

enum BlockType {
    MetaIndex,
    Index,
    Data,
}

fn print_record_kv(pair: &KeyValPair, idx: usize, block_type: BlockType) -> io::Result<()> {
    match block_type {
        BlockType::MetaIndex => {
            println!("\n************ Meta Index Record {} *************", idx + 1);
            let handle = parse_block_handle(&pair.value)?;
            println!(
                "FilterName: {}\nBlockHandle: Offset: {}, Size: {}",
                utils::bytes_to_ascii_with_hex(&pair.key),
                handle.offset,
                handle.size
            );
        }
        BlockType::Index => {
            println!("\n*************** Index Record {} ***************", idx + 1);
            let handle = parse_block_handle(&pair.value)?;
            println!(
                "SeparatorKey: {}\nBlockHandle: Offset: {}, Size: {}",
                utils::bytes_to_ascii_with_hex(&pair.key),
                handle.offset,
                handle.size
            );
        }
        BlockType::Data => {
            println!("\n*************** Data Record {} ****************", idx + 1);
            let (key, stat, seq) = utils::decode_key(&pair.key)?;
            println!(
                "Seq: {}, Stat: {}\nKey: '{}'\nVal: '{}'",
                seq,
                match stat {
                    1 => "1 (Live)",
                    2 => "2 (Deleted)",
                    _ => "Unknown",
                },
                utils::bytes_to_ascii_with_hex(&key),
                utils::bytes_to_ascii_with_hex(&pair.value)
            );
        }
    }

    Ok(())
}
