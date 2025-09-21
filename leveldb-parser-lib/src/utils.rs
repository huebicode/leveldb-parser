use crc32c::crc32c;
use std::io::{self, Read, Seek};

const MASK_DELTA: u32 = 0xa282ead8;
pub fn unmask_crc32c(masked_crc: u32) -> u32 {
    let rot = masked_crc.wrapping_sub(MASK_DELTA);
    rot.rotate_left(15)
}

pub fn crc_verified(crc: u32, data_slice: &[u8], type_byte: u8, ldb_file_flag: bool) -> bool {
    let mut buf = Vec::with_capacity(data_slice.len() + 1);

    if ldb_file_flag {
        buf.extend_from_slice(data_slice);
        buf.push(type_byte);
    } else {
        // log file
        buf.push(type_byte);
        buf.extend_from_slice(data_slice);
    }

    let calculated_crc = crc32c(&buf);
    let unmasked_crc = unmask_crc32c(crc);

    calculated_crc == unmasked_crc
}

pub fn decode_key(key: &[u8]) -> io::Result<(Vec<u8>, u8, u64)> {
    if key.len() < 8 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Key too short"));
    }

    let user_key = &key[..key.len() - 8];
    let suffix = &key[key.len() - 8..];

    let status = suffix[0];

    let mut seq_bytes = [0; 8];
    seq_bytes[0..7].copy_from_slice(&suffix[1..8]);
    let sequence = u64::from_le_bytes(seq_bytes);

    Ok((user_key.to_vec(), status, sequence))
}

pub fn decode_varint(bytes: &[u8]) -> io::Result<u64> {
    let mut result: u64 = 0;
    let mut shift: u64 = 0;

    for &byte in bytes {
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break; // break if continuation bit is not set
        }
        shift += 7;
        if shift >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Varint too long",
            ));
        }
    }
    Ok(result)
}

pub fn read_varint(reader: &mut (impl Read + Seek)) -> io::Result<u64> {
    let mut varint_bytes = Vec::new();
    let mut buf = [0; 1];

    loop {
        reader.read_exact(&mut buf)?;
        varint_bytes.push(buf[0]);

        if buf[0] & 0x80 == 0 {
            break; // break if the continuation bit is not set
        }

        if varint_bytes.len() >= 10 {
            // 64-bit varint can take up to 10 bytes
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Varint too long",
            ));
        }
    }

    decode_varint(&varint_bytes)
}

pub fn read_slice(reader: &mut (impl Read + Seek), length: usize) -> io::Result<Vec<u8>> {
    let mut data = vec![0; length];
    let bytes_read = reader.read(&mut data)?;

    // if partial data block (record type 2)
    if bytes_read < length {
        data.truncate(bytes_read);
    }

    Ok(data)
}

pub fn read_varint_slice(reader: &mut (impl Read + Seek)) -> io::Result<Vec<u8>> {
    let record_len = read_varint(reader)? as usize;
    read_slice(reader, record_len)
}
