use leveldb_parser_lib::manifest_parser;
// use leveldb_parser_lib::log_parser;

fn main() {
    manifest_parser::parse_file("./_/MANIFEST-000017").unwrap();
    // log_parser::parse_file("./_/000003.log").unwrap();
}
