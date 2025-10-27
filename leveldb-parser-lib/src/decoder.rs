use chrono::{TimeZone, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageKind {
    SessionStorage,
    LocalStorage,
    IndexedDb,
    Generic,
}

pub fn detect_storage_kind(path: &str) -> StorageKind {
    let lower = path.to_ascii_lowercase();
    match () {
        _ if lower.contains("local storage") => StorageKind::LocalStorage,
        _ if lower.contains("session storage") => StorageKind::SessionStorage,
        _ if lower.contains("indexeddb") => StorageKind::IndexedDb,
        _ => StorageKind::Generic,
    }
}

pub fn decode_kv(kind: StorageKind, key: &[u8], value: Option<&[u8]>) -> (String, String) {
    match kind {
        StorageKind::SessionStorage => {
            let k = bytes_to_utf8_lossy(key);
            let v = match value {
                Some(v_bytes) => {
                    if k.starts_with("map-") {
                        if let Some(s) = try_utf16le(v_bytes) {
                            s
                        } else {
                            bytes_to_utf8_lossy(v_bytes)
                        }
                    } else {
                        bytes_to_utf8_lossy(v_bytes)
                    }
                }
                None => String::new(),
            };
            (k, v)
        }
        StorageKind::LocalStorage => {
            let is_entry = key.starts_with(b"_");
            let is_meta = key.starts_with(b"META:") || key.starts_with(b"METAACCESS:");

            let k = if is_entry {
                decode_local_storage_key(key)
            } else {
                bytes_to_latin1_hex_escaped(key)
            };

            let v = match value {
                Some(v_bytes) => {
                    if is_entry {
                        decode_local_storage_value(v_bytes)
                    } else if is_meta {
                        decode_local_storage_meta(v_bytes)
                    } else if key.eq_ignore_ascii_case(b"VERSION") {
                        bytes_to_latin1_hex_escaped(v_bytes)
                    } else {
                        bytes_to_hex(v_bytes)
                    }
                }
                None => String::new(),
            };
            (k, v)
        }
        StorageKind::IndexedDb => {
            match key {
                [_, _, _, 0x01, entry_type @ 0x00..=0x06, key_payload @ ..] => {
                    // record entry
                    let k = decode_indexeddb_key(*entry_type, key_payload);
                    let v = match value {
                        Some(v_bytes) => decode_indexeddb_entry(v_bytes),
                        None => String::new(),
                    };
                    (k, v)
                }
                _ => {
                    let k = bytes_to_hex(key);
                    let v = match value {
                        Some(v_bytes) => bytes_to_hex(v_bytes),
                        None => String::new(),
                    };
                    (k, v)
                }
            }
        }
        StorageKind::Generic => {
            let k = bytes_to_utf8_lossy(key);
            let v = match value {
                Some(v_bytes) => bytes_to_utf8_lossy(v_bytes),
                None => String::new(),
            };
            (k, v)
        }
    }
}

fn decode_local_storage_key(bytes: &[u8]) -> String {
    let delim_pos = match bytes.iter().position(|&b| b == 0x00) {
        Some(p) => p,
        None => return bytes_to_latin1_hex_escaped(bytes), // fallback without delimiter
    };
    if delim_pos + 1 >= bytes.len() {
        // key prefix only
        return bytes_to_latin1_hex_escaped(bytes);
    }

    // decode key
    let key_prefix = &bytes[..delim_pos];
    let encoding_flag = bytes[delim_pos + 1];
    let key_entry = &bytes[delim_pos + 2..];

    match encoding_flag {
        0x00 => {
            // UTF-16LE
            if let Some(entry) = try_utf16le(key_entry) {
                format!("{} {}", bytes_to_latin1_hex_escaped(key_prefix), entry)
            } else {
                bytes_to_latin1_hex_escaped(bytes) // fallback
            }
        }
        0x01 => {
            // Latin-1
            format!(
                "{} {}",
                bytes_to_latin1_hex_escaped(key_prefix),
                bytes_to_latin1_hex_escaped(key_entry)
            )
        }
        _ => bytes_to_latin1_hex_escaped(bytes), // unknown flag => fallback
    }
}

fn decode_local_storage_value(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0x00]) || bytes.starts_with(&[0x01]) {
        let encoding_flag = bytes[0];
        let value_entry = &bytes[1..];

        let value = match encoding_flag {
            0x00 => {
                // UTF-16LE
                if let Some(entry) = try_utf16le(value_entry) {
                    entry
                } else {
                    bytes_to_latin1_hex_escaped(value_entry) // fallback
                }
            }
            0x01 => {
                // Latin-1
                bytes_to_latin1_hex_escaped(value_entry)
            }
            _ => {
                // unknown flag => fallback
                bytes_to_hex(value_entry)
            }
        };

        return value;
    }

    bytes_to_hex(bytes) // fallback
}

fn decode_local_storage_meta(v_bytes: &[u8]) -> String {
    // Chrome timestamp epoch: 1601-01-01T00:00:00Z
    const CHROME_EPOCH: i64 = 11644473600000000; // microseconds between 1601-01-01 and 1970-01-01

    // parse protobuf: field 1 = creation_time (varint, microseconds since 1601)
    // optionally, field 2 = size (varint)
    let mut i = 0;
    let mut creation_time: Option<u64> = None;
    let mut size: Option<u64> = None;

    while i < v_bytes.len() {
        let key = v_bytes[i];
        i += 1;
        let field_number = key >> 3;
        let wire_type = key & 0x07;

        match (field_number, wire_type) {
            (1, 0) => {
                // varint: creation_time
                let (val, consumed) = parse_varint(&v_bytes[i..]);
                creation_time = Some(val);
                i += consumed;
            }
            (2, 0) => {
                // varint: size
                let (val, consumed) = parse_varint(&v_bytes[i..]);
                size = Some(val);
                i += consumed;
            }
            _ => {
                // unknown field
                break;
            }
        }
    }

    let mut out = String::new();
    if let Some(ts) = creation_time {
        // convert Chrome timestamp to Unix timestamp
        let unix_us = ts as i64 - CHROME_EPOCH;
        let unix_s = unix_us / 1_000_000;
        let unix_ns = (unix_us % 1_000_000) * 1000;

        let dt = Utc.timestamp_opt(unix_s, unix_ns as u32).single();
        if let Some(dt) = dt {
            out.push_str(&dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        } else {
            out.push_str(&unix_s.to_string()); // fallback to Unix seconds
        }
    }
    if let Some(sz) = size {
        out.push_str(&format!(" ({})", sz));
    }
    if out.is_empty() {
        out.push_str(&bytes_to_hex(v_bytes));
    }
    out
}

fn decode_indexeddb_key(entry_type: u8, payload: &[u8]) -> String {
    match entry_type {
        // String
        0x01 => decode_varint_utf16be(payload),
        // Double-precision float
        0x03 => {
            if payload.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&payload[..8]);
                let num = f64::from_le_bytes(arr);
                num.to_string()
            } else {
                bytes_to_hex(payload)
            }
        }
        _ => bytes_to_hex(payload),
    }
}

fn decode_indexeddb_entry(bytes: &[u8]) -> String {
    // find two 0xFF sentinels in header
    let mut i = 0;
    let mut ff = 0;
    while i < bytes.len() {
        if bytes[i] == 0xFF {
            ff += 1;
            if ff == 2 {
                i += 1; // move past second 0xFF
                break;
            }
        }
        i += 1;
    }

    // fallback
    if ff < 2 || i >= bytes.len() {
        return bytes_to_utf8_lossy(bytes);
    }

    // skip v8 version tag
    if read_varint_len(bytes, &mut i).is_none() {
        return bytes_to_utf8_lossy(bytes);
    }

    // skip padding zeros
    while i < bytes.len() && bytes[i] == 0x00 {
        i += 1;
    }

    // fallback
    if i >= bytes.len() {
        return bytes_to_utf8_lossy(bytes);
    }

    // context-aware formatter
    #[derive(Debug)]
    enum Ctx {
        Object { expect_key: bool, first: bool },
        Array { first: bool },
    }

    let mut out = String::new();
    let mut stack: Vec<Ctx> = Vec::new();

    // emit a scalar or delimiter in the correct spot, adding separators
    fn emit(out: &mut String, stack: &mut [Ctx], s: &str) {
        match stack.last_mut() {
            Some(Ctx::Array { first }) => {
                if !*first {
                    out.push(',');
                } else {
                    *first = false;
                }
                out.push_str(s);
            }
            Some(Ctx::Object { expect_key, first }) => {
                if *expect_key {
                    if !*first {
                        out.push(',');
                    } else {
                        *first = false;
                    }
                    out.push_str(s);
                    out.push(':'); // "key:val"
                    *expect_key = false; // next token is the value
                } else {
                    out.push_str(s); // value
                    *expect_key = true; // next token is the next key
                }
            }
            None => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(s);
            }
        }
    }

    while i < bytes.len() {
        let tag = bytes[i];
        i += 1; // consume tag

        match tag {
            0x00 | 0x2D => continue, // Padding | Array-Gap

            // Primitive Values
            0x5F => emit(&mut out, &mut stack, "\"undefined\""),
            0x30 => emit(&mut out, &mut stack, "null"),
            0x54 => emit(&mut out, &mut stack, "true"),
            0x46 => emit(&mut out, &mut stack, "false"),

            0x49 => {
                // ZigZag Int32
                let s = decode_tag_or_hex(bytes, &mut i, tag, handle_tag_0x49);
                emit(&mut out, &mut stack, &s);
            }
            0x4E => {
                // 8-byte Double (LE)
                let s = decode_tag_or_hex(bytes, &mut i, tag, handle_tag_0x4e);
                emit(&mut out, &mut stack, &s);
            }
            0x22 => {
                // 1-byte String (Latin-1)
                let s = decode_tag_or_hex(bytes, &mut i, tag, handle_tag_0x22);
                emit(&mut out, &mut stack, &s);
            }
            0x63 => {
                // 2-byte String (UTF-16LE)
                let s = decode_tag_or_hex(bytes, &mut i, tag, handle_tag_0x63);
                emit(&mut out, &mut stack, &s);
            }

            // Property Objects
            0x6F => {
                // Object start tag
                emit(&mut out, &mut stack, "{");
                stack.push(Ctx::Object {
                    expect_key: true,
                    first: true,
                });
            }
            0x7B => {
                // Object end tag
                let _ = read_varint_len(bytes, &mut i); // property count
                out.push('}');
                let _ = stack.pop();
            }
            0x61 | 0x41 => {
                // SparseArray | DenseArray start tag
                let _ = read_varint_len(bytes, &mut i); // len
                emit(&mut out, &mut stack, "[");
                stack.push(Ctx::Array { first: true });
            }
            0x40 | 0x24 => {
                // SparseArray | DenseArray end tag
                let _ = read_varint_len(bytes, &mut i); // property count
                let _ = read_varint_len(bytes, &mut i); // len
                out.push(']');
                let _ = stack.pop();
            }

            _ => emit(&mut out, &mut stack, &format!("\\x{:02X}", tag)),
        };
    }

    out
}

// indexeddb decode helpers ----------------------------------------------------

// read Varint starting at *i, advance cursor, return length
fn read_varint_len(bytes: &[u8], i: &mut usize) -> Option<usize> {
    let (val, consumed) = parse_varint(&bytes[*i..]);
    if consumed == 0 {
        return None;
    }
    *i += consumed;
    Some(val as usize)
}

// try handler; on failure, revert i and return hex val
fn decode_tag_or_hex(
    bytes: &[u8],
    i: &mut usize,
    tag: u8,
    handler: fn(&[u8], &mut usize) -> Option<String>,
) -> String {
    let before = *i;
    if let Some(s) = handler(bytes, i) {
        s
    } else {
        *i = before;
        format!("\\x{:02X}", tag)
    }
}

// 1-byte String (Latin-1)
fn handle_tag_0x22(bytes: &[u8], i: &mut usize) -> Option<String> {
    let len = read_varint_len(bytes, i)?;
    if *i + len > bytes.len() {
        return None;
    }
    let payload = &bytes[*i..*i + len];
    *i += len;
    Some(format!("\"{}\"", bytes_to_latin1_hex_escaped(payload)))
}

// 2-byte String (UTF-16LE)
fn handle_tag_0x63(bytes: &[u8], i: &mut usize) -> Option<String> {
    let len = read_varint_len(bytes, i)?;
    if *i + len > bytes.len() {
        return None;
    }
    let payload = &bytes[*i..*i + len];
    *i += len;
    if len % 2 == 0 {
        if let Some(s) = try_utf16le(payload) {
            return Some(format!("\"{}\"", s));
        }
    }
    None
}

// ZigZag Int32 (like protobuf sint32)
fn handle_tag_0x49(bytes: &[u8], i: &mut usize) -> Option<String> {
    let (val, consumed) = parse_varint(&bytes[*i..]);
    if consumed == 0 {
        return None;
    }
    *i += consumed;
    let raw = val as u32;
    // ZigZag decode
    let signed = ((raw >> 1) as i32) ^ (-((raw & 1) as i32));
    Some(format!("{}", signed))
}

// 8-byte Double (LE)
fn handle_tag_0x4e(bytes: &[u8], i: &mut usize) -> Option<String> {
    if *i + 8 > bytes.len() {
        return None;
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[*i..*i + 8]);
    *i += 8;
    let v = f64::from_le_bytes(arr);
    Some(format!("{}", v))
}

// -----------------------------------------------------------------------------

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\x{:02X}", b)).collect()
}

pub fn bytes_to_ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

pub fn bytes_to_ascii_with_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 {
                (b as char).to_string()
            } else {
                format!("\\x{:02X}", b)
            }
        })
        .collect()
}

fn bytes_to_latin1_hex_escaped(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 || b >= 0xA0 {
                (b as char).to_string()
            } else {
                format!("\\x{:02X}", b)
            }
        })
        .collect()
}

pub fn bytes_to_utf8_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

fn try_utf16(bytes: &[u8], little_endian: bool) -> Option<String> {
    if bytes.len() >= 2 && bytes.len() % 2 == 0 {
        let mut buf = Vec::with_capacity(bytes.len() / 2);
        for c in bytes.chunks(2) {
            let unit = if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            };
            buf.push(unit);
        }
        if let Ok(s) = String::from_utf16(&buf) {
            if s.chars().any(|ch| !ch.is_control()) {
                return Some(s.chars().filter(|ch| !ch.is_control()).collect());
            }
        }
    }
    None
}

fn try_utf16le(bytes: &[u8]) -> Option<String> {
    try_utf16(bytes, true)
}

fn try_utf16be(bytes: &[u8]) -> Option<String> {
    try_utf16(bytes, false)
}

fn parse_varint(bytes: &[u8]) -> (u64, usize) {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    let mut consumed = 0;
    for &b in bytes {
        value |= ((b & 0x7F) as u64) << shift;
        consumed += 1;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (value, consumed)
}

fn decode_varint_utf16be(bytes: &[u8]) -> String {
    let (val, consumed) = parse_varint(bytes);
    let slice = &bytes[consumed..consumed + val as usize * 2]; // varint gives number of UTF-16 code units
    try_utf16be(slice).unwrap_or_else(|| bytes_to_utf8_lossy(slice))
}
