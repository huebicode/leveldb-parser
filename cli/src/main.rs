use leveldb_parser_lib::log_parser;

fn main() {
    log_parser::parse_file("./_/000003.log").unwrap();
}
