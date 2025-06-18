// use leveldb_parser_lib::log_parser;
use leveldb_parser_lib::ldb_parser;

fn main() {
    ldb_parser::parse_file("./_/one_entry_with_bloom.ldb").unwrap();
    // manifest_parser::parse_file("./_/MANIFEST-000017").unwrap();
    // log_parser::parse_file("./_/_000003.log").unwrap();
}
