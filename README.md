# LevelDB Parser
[LevelDB](https://github.com/google/leveldb) is a key/value database from Google, used among other things in Chromium-based browsers and applications (such as Electron). This project is part of my master’s thesis in digital forensics at *Hochschule Albstadt-Sigmaringen*, Germany, which involves developing a parser (CLI/GUI) for LevelDB. The final thesis will be uploaded here after completion.

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
- [x] CLI
- [x] GUI

## GUI
`leveldb-parser-gui` can accept both individual files and folders, which are processed recursively.

<img alt="leveldbgui-interface" src="https://github.com/user-attachments/assets/fa7f870a-567a-42f4-b83d-1af49186eacd" />


## CLI
`leveldb-parser-cli` can parse single `.log`, `.ldb` or `MANIFEST` files. 

### Usage
`leveldb-parser-cli [-a] <file>`

Default output is CSV with key/value information:
```
"seq","state","key","value"
"1","Live","Mozart","Eine kleine Nachtmusik"
"2","Live","Bach","Air"
"3","Live","Vivaldi","Le quattro stagioni"
```

Option `-a` will output all available details including meta data:
```
########## [ Block 1 (Offset: 0)] ############
------------------- Header -------------------
CRC32C: 1A2B3C4D (verified)
Data-Length: 46 Bytes
Record-Type: 1 (Full)
//////////////// Batch Header ////////////////
Seq: 1
Records: 1
****************** Record 1 ******************
Seq: 1, State: 1 (Live)
Key (Offset: 21, Size: 9): 'Mozart'
Val (Offset: 31, Size: 22): 'Eine kleine Nachtmusik'
```

## Build
Pre-built binaries are available under [Releases](https://github.com/huebicode/leveldb-parser/releases).

Alternatively, to build from source:
- install Rust: https://www.rust-lang.org
- install Tauri and its prerequisites: https://tauri.app
- download this project
- execute `cargo build --release` in the project dir
- the applications will be compiled in `target/release`
