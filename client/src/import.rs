use std::fs;
use std::fs::File;
use std::io::{Read, Write};

use crate::*;

// #[test]
// fn test_import_denormalised() {
//     // read from zip
//     let fname = "../../packages/parser/output/tdb_ir/f8b0f10fde0d460bd23959a388b754a5.json.zip";
//     let file = fs::File::open(fname).unwrap();
//     let mut archive = zip::ZipArchive::new(file).unwrap();
//     let mut zippedfile = archive.by_index(0).unwrap();
//
//     // TDB json
//     let json : serde_json::Value = serde_json::from_reader(zippedfile).unwrap();
//     // let normalized = crate::normalization::normalize(json);
//
//     // import
//     let client = TerminusDBCli::connect_catalog(None);
//     client.doc_replace_all_json(
//         &json,
//         GraphType::Instance,
//         "admin/scores".to_string(),
//         true).unwrap();
// }
//
// #[test]
// fn test_import_normalised() {
//     // read from zip
//     let fname = "../../packages/parser/output/tdb_ir/f8b0f10fde0d460bd23959a388b754a5.json.zip";
//     let file = fs::File::open(fname).unwrap();
//     let mut archive = zip::ZipArchive::new(file).unwrap();
//     let mut zippedfile = archive.by_index(0).unwrap();
//
//     // TDB json
//     let json : serde_json::Value = serde_json::from_reader(zippedfile).unwrap();
//     let normalized = crate::normalize(json);
//
//     // dump
//     let mut output = File::create("normalized.test.json").unwrap();
//     write!(output, "{}", serde_json::to_string(&normalized).unwrap());
//
//     // import
//     let client = TerminusDBCli::connect_catalog(None);
//     client.doc_replace_all_json(
//         &normalized,
//         GraphType::Instance,
//         "admin/scores".to_string(),
//         true).unwrap();
// }
