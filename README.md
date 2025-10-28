# LevelDB Parser
[LevelDB](https://github.com/google/leveldb) is a key/value database from Google, used among other things in Chromium-based browsers and applications, such as Electron. 

This project is part of my master’s thesis in Digital Forensics at the University of Applied Sciences Albstadt-Sigmaringen, Germany, which involves developing a parser for LevelDB. The final thesis will be uploaded here after completion.

## Project Goal

The goal is to explore and understand the internal structure of LevelDB and implement a parser that can read and interpret LevelDB file formats. The tool may be useful in fields like:

- Digital forensics
- Database debugging
- Educational insight into LevelDB internals


## Status

**Work in progress!** – This repository will be continuously updated and changed as the thesis progresses.


Implemented features:

- [x] parse `.log` files
- [x] parse `MANIFEST` files
- [x] parse `.ldb` files
- [x] decode `Web Storage` entries (applied if path contains `Session Storage` or `Local Storage`)
- [x] decode `IndexedDB` entries (implemented for common types, applied if path contains `IndexedDB`)

## GUI
The GUI parser can accept both individual files and folders, which are processed recursively.

<img alt="leveldb-gui-interface" src="https://github.com/user-attachments/assets/a3df841b-6623-4e5d-838f-0b83faed630e" />


## CLI
The CLI parser can parse single `.log`, `.ldb` or `MANIFEST` files. 

### Usage
`leveldb-parser-cli [-a] <file>`

Default output is CSV with key/value information:
```
"seq","state","key","value"
"1","Live","Mozart","Eine kleine Nachtmusik"
"2","Live","Vivaldi","Le quattro stagioni"
"3","Live","Bach","Air"
```

Option `-a` will output all available details including meta data:
```
########## [ Block 3 (Offset: 98)] ############
------------------- Header -------------------
CRC32C: DC3ADC4D (verified)
Data-Length: 22 Bytes
Record-Type: 1 (Full)

//////////////// Batch Header ////////////////
Seq: 3
Records: 1

****************** Record 1 ******************
Seq: 3, State: 1 (Live)
Key (Offset: 119, Size: 4): '\x42\x61\x63\x68'
Val (Offset: 124, Size: 3): '\x41\x69\x72'
```

## Build
Pre-built binaries are available under [Releases](https://github.com/huebicode/leveldb-parser/releases).

Alternatively, to build from source:
- install Rust: https://www.rust-lang.org
- install Tauri and its prerequisites: https://tauri.app
- download this project
- execute `cargo build --release` in the project dir
- the applications will be compiled in `target/release`
