use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, Write};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::decoder;
use crate::utils;

// -----------------------------------------------------------------------------
pub struct LdbFile {
    pub footer: Footer,
    pub meta_index_block: IndexBlock,
    pub index_block: IndexBlock,
    pub meta_blocks: Vec<MetaBlock>,
    pub data_blocks: Vec<DataBlock>,
    pub storage_kind: decoder::StorageKind,
}

pub struct Footer {
    pub offset: u64,
    pub meta_index_handle: BlockHandle,
    pub index_handle: BlockHandle,
    pub magic: [u8; 8],
    pub is_valid: bool,
}

#[derive(Clone, Copy)]
pub struct BlockHandle {
    pub offset: u64,
    pub size: u64,
}

pub struct IndexBlock {
    pub raw_block: RawBlock,
    pub records: Vec<IndexRecord>,
    pub block_handle: BlockHandle,
}

pub struct IndexRecord {
    pub key: Vec<u8>,
    pub block_handle: BlockHandle,
    pub entry: KeyValPair,
}

pub struct MetaBlock {
    pub name: String,
    pub raw_block: RawBlock,
    pub block_handle: BlockHandle,
    pub bloom_filter: Option<BloomFilter>,
}

pub struct DataBlock {
    pub raw_block: RawBlock,
    pub records: Vec<DataRecord>,
    pub block_handle: BlockHandle,
}

pub struct DataRecord {
    pub seq: u64,
    pub state: u8,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub entry: KeyValPair,
}

pub struct RawBlock {
    pub data: Vec<u8>,
    pub compression_type: u8,
    pub crc: u32,
    pub crc_valid: bool,
}

pub struct KeyValPair {
    pub shared_len: usize,
    pub inline_len: usize,
    pub value_len: usize,
    pub key_offset: u64,
    pub key: Vec<u8>,
    pub val_offset: u64,
    pub value: Vec<u8>,
}

pub struct BloomFilter {
    pub filter_data: Vec<u8>,
    pub array_offset: u32,
    pub base_log: u8,
}

// -----------------------------------------------------------------------------
pub fn parse_file(file_path: &str) -> io::Result<LdbFile> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let storage_kind = decoder::detect_storage_kind(file_path);

    // Footer
    let footer = read_footer(&mut reader)?;

    // Meta Index Block
    let meta_index_raw = read_raw_block(
        &mut reader,
        footer.meta_index_handle.offset,
        footer.meta_index_handle.size,
    )?;
    let meta_index_kvs = read_block_data_kvs(&meta_index_raw.data)?;
    let meta_index_records = meta_index_kvs
        .into_iter()
        .map(|entry| {
            let block_handle =
                parse_block_handle(&entry.value).unwrap_or(BlockHandle { offset: 0, size: 0 });
            IndexRecord {
                key: entry.key.clone(),
                block_handle,
                entry,
            }
        })
        .collect();

    let meta_index_block = IndexBlock {
        raw_block: meta_index_raw,
        records: meta_index_records,
        block_handle: footer.meta_index_handle,
    };

    // Meta Blocks
    let mut meta_blocks = Vec::new();
    for record in &meta_index_block.records {
        let meta_raw = read_raw_block(
            &mut reader,
            record.block_handle.offset,
            record.block_handle.size,
        )?;
        let name = decoder::bytes_to_ascii_with_hex(&record.key);
        let bloom_filter = if name == "filter.leveldb.BuiltinBloomFilter2" {
            Some(parse_bloom_filter_block(&meta_raw.data)?)
        } else {
            None
        };

        meta_blocks.push(MetaBlock {
            name,
            raw_block: meta_raw,
            block_handle: record.block_handle,
            bloom_filter,
        });
    }

    // Index Block
    let index_raw = read_raw_block(
        &mut reader,
        footer.index_handle.offset,
        footer.index_handle.size,
    )?;
    let index_kvs = read_block_data_kvs(&index_raw.data)?;
    let index_records = index_kvs
        .into_iter()
        .map(|entry| {
            let block_handle =
                parse_block_handle(&entry.value).unwrap_or(BlockHandle { offset: 0, size: 0 });
            IndexRecord {
                key: entry.key.clone(),
                block_handle,
                entry,
            }
        })
        .collect();

    let index_block = IndexBlock {
        raw_block: index_raw,
        records: index_records,
        block_handle: footer.index_handle,
    };

    // Data Blocks
    let mut data_blocks = Vec::new();
    for record in &index_block.records {
        let data_raw = read_raw_block(
            &mut reader,
            record.block_handle.offset,
            record.block_handle.size,
        )?;
        let data_kvs = read_block_data_kvs(&data_raw.data)?;
        let data_records = data_kvs
            .into_iter()
            .map(|entry| {
                let (key, state, seq) = utils::decode_key(&entry.key).unwrap_or((Vec::new(), 0, 0));
                DataRecord {
                    seq,
                    state,
                    key,
                    value: entry.value.clone(),
                    entry,
                }
            })
            .collect();

        data_blocks.push(DataBlock {
            raw_block: data_raw,
            records: data_records,
            block_handle: record.block_handle,
        });
    }

    Ok(LdbFile {
        footer,
        meta_index_block,
        index_block,
        meta_blocks,
        data_blocks,
        storage_kind,
    })
}

// -----------------------------------------------------------------------------
fn read_footer(reader: &mut (impl Read + Seek)) -> io::Result<Footer> {
    let offset = reader.seek(io::SeekFrom::End(-48))?;

    let meta_index_handle = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };

    let index_handle = BlockHandle {
        offset: utils::read_varint(reader)?,
        size: utils::read_varint(reader)?,
    };

    const EXPECTED_MAGIC: [u8; 8] = [0x57, 0xFB, 0x80, 0x8B, 0x24, 0x75, 0x47, 0xDB];
    reader.seek(io::SeekFrom::End(-8))?;
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;

    let is_valid = magic == EXPECTED_MAGIC;

    Ok(Footer {
        offset,
        meta_index_handle,
        index_handle,
        magic,
        is_valid,
    })
}

fn read_raw_block(reader: &mut (impl Read + Seek), offset: u64, size: u64) -> io::Result<RawBlock> {
    reader.seek(io::SeekFrom::Start(offset))?;

    // data
    let mut data = vec![0; size as usize];
    reader.read_exact(&mut data)?;

    // compression
    let compression_type = reader.read_u8()?;

    // crc
    let crc = reader.read_u32::<LittleEndian>()?;

    // verify crc
    let crc_valid = utils::crc_verified(crc, &data, compression_type, true);

    // decompress data if needed
    let data = if compression_type == 0x1 {
        snap::raw::Decoder::new().decompress_vec(&data)?
    } else if compression_type == 0x2 {
        // NOTE: not tested
        zstd::decode_all(data.as_slice())?
    } else {
        data
    };

    Ok(RawBlock {
        data,
        compression_type,
        crc,
        crc_valid,
    })
}

fn read_block_data_kvs(data: &[u8]) -> io::Result<Vec<KeyValPair>> {
    let mut cursor = Cursor::new(data);

    cursor.seek(io::SeekFrom::End(-4))?;
    let restart_arr_len = cursor.read_u32::<LittleEndian>()?;
    let restart_array_offset = cursor.seek(io::SeekFrom::End(-4 - (4 * restart_arr_len as i64)))?;

    let mut entries = Vec::new();
    let mut prev_key = Vec::new();

    cursor.seek(io::SeekFrom::Start(0))?;
    while cursor.position() < restart_array_offset {
        match read_block_entry(&mut cursor, &prev_key) {
            Ok(entry) => {
                prev_key = entry.key.clone();
                entries.push(entry);
            }
            Err(_) => break,
        }
    }

    Ok(entries)
}

fn read_block_entry(cursor: &mut Cursor<&[u8]>, prev_key: &[u8]) -> io::Result<KeyValPair> {
    let shared_len = utils::read_varint(cursor)? as usize;
    let inline_len = utils::read_varint(cursor)? as usize;
    let value_len = utils::read_varint(cursor)? as usize;

    let key_offset = cursor.position();

    let mut inline_key = vec![0; inline_len];
    cursor.read_exact(&mut inline_key)?;

    // construct full key
    let mut key = Vec::with_capacity(shared_len + inline_len);

    if shared_len > 0 && shared_len <= prev_key.len() {
        key.extend_from_slice(&prev_key[0..shared_len]);
    }

    key.extend_from_slice(&inline_key);

    // value
    let val_offset = cursor.position();
    let mut value = vec![0; value_len];
    cursor.read_exact(&mut value)?;

    Ok(KeyValPair {
        shared_len,
        inline_len,
        value_len,
        key_offset,
        key,
        val_offset,
        value,
    })
}

fn parse_bloom_filter_block(data: &[u8]) -> io::Result<BloomFilter> {
    let mut cursor = Cursor::new(data);

    cursor.seek(io::SeekFrom::End(-5))?;
    let array_offset = cursor.read_u32::<LittleEndian>()?;
    let base_log = cursor.read_u8()?;
    let filter_data = data[0..array_offset as usize].to_vec();

    Ok(BloomFilter {
        filter_data,
        array_offset,
        base_log,
    })
}

fn parse_block_handle(data: &[u8]) -> io::Result<BlockHandle> {
    let mut cursor = Cursor::new(data);
    let offset = utils::read_varint(&mut cursor)?;
    let size = utils::read_varint(&mut cursor)?;
    Ok(BlockHandle { offset, size })
}
// -----------------------------------------------------------------------------
pub mod display {
    use super::*;

    pub fn print_all(ldb: &LdbFile) -> io::Result<()> {
        print_footer(&ldb.footer)?;
        print_meta_index_block(&ldb.meta_index_block)?;

        for (idx, meta_block) in ldb.meta_blocks.iter().enumerate() {
            print_meta_block(meta_block, idx)?;
        }

        print_index_block(&ldb.index_block)?;

        for (idx, data_block) in ldb.data_blocks.iter().enumerate() {
            print_data_block(data_block, idx)?;
        }

        Ok(())
    }

    pub fn print_footer(footer: &Footer) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "############# Footer (Offset: {}) #############",
            footer.offset
        )?;
        writeln!(
            io::stdout(),
            "BlockHandle (Meta Index Block): Offset: {}, Size: {}",
            footer.meta_index_handle.offset,
            footer.meta_index_handle.size
        )?;
        writeln!(
            io::stdout(),
            "BlockHandle (Index Block): Offset: {}, Size: {}",
            footer.index_handle.offset,
            footer.index_handle.size
        )?;
        writeln!(
            io::stdout(),
            "Magic: {:02X?} {}",
            footer.magic,
            if footer.is_valid {
                "(valid)"
            } else {
                "(invalid!)"
            }
        )?;
        Ok(())
    }

    pub fn print_meta_index_block(block: &IndexBlock) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n######## Meta Index Block (Offset: {}) ########",
            block.block_handle.offset
        )?;
        print_raw_block_info(&block.raw_block)?;
        print_block_data_info(&block.raw_block.data)?;

        for (idx, record) in block.records.iter().enumerate() {
            writeln!(
                io::stdout(),
                "\n//////////// Meta Index Record {} /////////////",
                idx + 1
            )?;
            writeln!(
                io::stdout(),
                "FilterName: {}\nBlockHandle: Offset: {}, Size: {}",
                decoder::bytes_to_ascii_with_hex(&record.key),
                record.block_handle.offset,
                record.block_handle.size
            )?;
        }

        Ok(())
    }

    pub fn print_meta_block(meta_block: &MetaBlock, idx: usize) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n########## Meta Block {} (Offset: {}) ###########",
            idx + 1,
            meta_block.block_handle.offset,
        )?;
        print_raw_block_info(&meta_block.raw_block)?;

        if let Some(bloom_filter) = &meta_block.bloom_filter {
            print_bloom_filter(bloom_filter)?;
        }

        Ok(())
    }

    pub fn print_index_block(block: &IndexBlock) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n########## Index Block (Offset: {}) ###########",
            block.block_handle.offset
        )?;
        print_raw_block_info(&block.raw_block)?;
        print_block_data_info(&block.raw_block.data)?;

        for (idx, record) in block.records.iter().enumerate() {
            writeln!(
                io::stdout(),
                "\n/////////////// Index Record {} ///////////////",
                idx + 1
            )?;
            writeln!(
                io::stdout(),
                "SeparatorKey: {}\nBlockHandle: Offset: {}, Size: {}",
                decoder::bytes_to_ascii_with_hex(&record.key),
                record.block_handle.offset,
                record.block_handle.size
            )?;
        }

        Ok(())
    }

    pub fn print_data_block(data_block: &DataBlock, idx: usize) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n########## Data Block {} (Offset: {}) ##########",
            idx + 1,
            data_block.block_handle.offset
        )?;
        print_raw_block_info(&data_block.raw_block)?;
        print_block_data_info(&data_block.raw_block.data)?;

        for (record_idx, record) in data_block.records.iter().enumerate() {
            print_data_record(record, record_idx, data_block.block_handle.offset)?;
        }

        Ok(())
    }

    pub fn print_data_record(record: &DataRecord, idx: usize, block_offset: u64) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n*************** Data Record {} ****************",
            idx + 1
        )?;
        writeln!(
            io::stdout(),
            "Seq: {}, State: {}",
            record.seq,
            match record.state {
                0 => "0 (Deleted)",
                1 => "1 (Live)",
                _ => "Unknown",
            },
        )?;
        writeln!(
            io::stdout(),
            "Key (Offset: {}, Size: {} [shared], {} [inline]): '{}'",
            block_offset + record.entry.key_offset,
            record.entry.shared_len,
            record.entry.inline_len,
            decoder::bytes_to_hex(&record.key),
        )?;
        writeln!(
            io::stdout(),
            "Val (Offset: {}, Size: {}): '{}'",
            block_offset + record.entry.val_offset,
            record.entry.value_len,
            decoder::bytes_to_hex(&record.value)
        )?;

        Ok(())
    }

    pub fn print_raw_block_info(raw_block: &RawBlock) -> io::Result<()> {
        match raw_block.compression_type {
            0x0 => writeln!(io::stdout(), "CompressionType: 0 (NoCompression)")?,
            0x1 => writeln!(io::stdout(), "CompressionType: 1 (Snappy)")?,
            0x2 => writeln!(io::stdout(), "CompressionType: 2 (Zstd)")?,
            _ => writeln!(
                io::stdout(),
                "CompressionType: {} (Unknown)",
                raw_block.compression_type
            )?,
        }

        if raw_block.crc_valid {
            writeln!(io::stdout(), "CRC32C: {:02X} (verified)", raw_block.crc)?;
        } else {
            writeln!(
                io::stdout(),
                "CRC32C: {:02X} (verification failed!)",
                raw_block.crc
            )?;
        }

        Ok(())
    }

    pub fn print_block_data_info(data: &[u8]) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "----------------- Block Data -----------------"
        )?;

        let mut cursor = Cursor::new(data);

        // restart array
        cursor.seek(io::SeekFrom::End(-4))?;
        let restart_count = cursor.read_u32::<LittleEndian>()?;

        writeln!(io::stdout(), "RestartArray (Count: {})", restart_count)?;

        Ok(())
    }

    pub fn print_bloom_filter(bloom_filter: &BloomFilter) -> io::Result<()> {
        writeln!(
            io::stdout(),
            "\n**************** Bloom Filter ****************"
        )?;
        writeln!(
            io::stdout(),
            "FilterData: {:02X?}",
            bloom_filter.filter_data
        )?;
        writeln!(io::stdout(), "ArrayOffset: {}", bloom_filter.array_offset)?;
        writeln!(io::stdout(), "BaseLog: {}", bloom_filter.base_log)?;
        Ok(())
    }
    // -----------------------------------------------------------------------------
    pub fn print_csv(ldb: &LdbFile) -> io::Result<()> {
        // Header
        writeln!(io::stdout(), "\"seq\",\"state\",\"key\",\"value\"")?;

        for data_block in &ldb.data_blocks {
            for record in &data_block.records {
                let state_str = match record.state {
                    0 => "Deleted",
                    1 => "Live",
                    _ => "Unknown",
                };

                let (key_str, value_str, _) = decoder::decode_kv(
                    ldb.storage_kind,
                    &record.key,
                    (record.state != 0).then_some(record.value.as_slice()),
                );
                let key_str = key_str.replace("\"", "\"\"");
                let value_str = value_str.replace("\"", "\"\"");

                writeln!(
                    io::stdout(),
                    "\"{}\",\"{}\",\"{}\",\"{}\"",
                    record.seq,
                    state_str,
                    key_str,
                    value_str
                )?;
            }
        }

        Ok(())
    }
}

pub mod export {
    use super::*;

    pub fn csv_string(ldb: &LdbFile, filename: &str, file_path: &str, hex_view: bool) -> String {
        let mut csv = String::new();
        // Header
        csv.push_str("\"Seq\",\"K\",\"V\",\"Cr\",\"St\",\"BO\",\"C\",\"F\",\"FP\",\"Kind\"\n");

        for data_block in &ldb.data_blocks {
            let compressed = data_block.raw_block.compression_type != 0;
            let crc_valid = if data_block.raw_block.crc_valid {
                "valid"
            } else {
                "failed"
            };

            for record in &data_block.records {
                let state_str = match record.state {
                    0 => "deleted",
                    1 => "live",
                    _ => "unknown",
                };

                let mut key_str;
                let mut value_str;
                let kind_str;

                if hex_view {
                    key_str = decoder::bytes_to_hex_raw(&record.key);
                    value_str = decoder::bytes_to_hex_raw(&record.value);
                    kind_str = "".to_string();
                } else {
                    (key_str, value_str, kind_str) = decoder::decode_kv(
                        ldb.storage_kind,
                        &record.key,
                        (record.state != 0).then_some(record.value.as_slice()),
                    );
                    key_str = key_str.replace("\"", "\"\"");
                    value_str = value_str.replace("\"", "\"\"");
                }

                csv.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    record.seq,
                    key_str,
                    value_str,
                    crc_valid,
                    state_str,
                    data_block.block_handle.offset,
                    compressed,
                    filename,
                    file_path,
                    kind_str,
                ));
            }
        }

        csv
    }
}
