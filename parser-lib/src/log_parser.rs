use crate::utils;

pub fn hello_from_log() {
    println!("Hello from log parser!");

    let bytes = [0x80, 0xdc, 0xff, 0x97, 0xa0, 0x01];
    let decoded_value = utils::decode_varint64(&bytes);
    println!("Decoded value: {}", decoded_value);
}
