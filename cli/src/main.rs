// use leveldb_parser_lib::log_parser;
// use leveldb_parser_lib::manifest_parser;
use leveldb_parser_lib::ldb_parser;

fn main() {
    ldb_parser::parse_file("./_/000009.ldb").unwrap();
    // manifest_parser::parse_file("./_/MANIFEST-000001").unwrap();
    // log_parser::parse_file("./_/000025.log").unwrap();
}
