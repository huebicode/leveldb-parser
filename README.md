# LevelDB Parser

This project is part of my master’s thesis in digital forensics at *Hochschule Albstadt-Sigmaringen*, Germany, which involves developing a parser for LevelDB. The final thesis will be uploaded here after completion.

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
- [ ] GUI

## Build
- install Rust for your system: https://www.rust-lang.org/
- download this project
- in the project dir execute `cargo build --release`
- the application will be compiled in `target/release`

## CLI
`leveldb-parser-cli` can parse `.log`, `.ldb` or `MANIFEST` files. 

### Usage
`leveldb-parser-cli [-a] <file>`

Default output is CSV with key/value information. Option `-a` will output all available details.

