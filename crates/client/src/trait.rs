use serde::de::DeserializeOwned;

use crate::*;

/// common methods for different targets
pub trait TerminusDBClient {
    type DocumentResult;
    // type QueryResult = QueryResult;

    fn db_create(&self, name: &str) -> TerminusDBResult<&Self>;

    fn doc_insert(&self, doc: Document, db: String) -> TerminusDBResult<Self::DocumentResult>;

    fn doc_insert_all(&self, docs: Documents, db: String)
        -> TerminusDBResult<Self::DocumentResult>;

    fn doc_delete(&self) -> TerminusDBResult<()>;

    fn doc_replace(
        &self,
        doc: Document,
        db: String,
        insert_if_not_exists: bool,
    ) -> TerminusDBResult<Self::DocumentResult>;

    fn doc_replace_all(
        &self,
        docs: Documents,
        db: String,
        insert_if_not_exists: bool,
    ) -> TerminusDBResult<Self::DocumentResult>;

    // impl specific
    fn doc_replace_all_json(
        &self,
        docs: &serde_json::Value,
        graph_type: GraphType,
        db: String,
        insert_if_not_exists: bool,
    ) -> TerminusDBResult<Self::DocumentResult>;

    // todo: make specific input type that encapsulates Document and Path and Value
    fn doc_replace_all_json_file(
        &self,
        docs: std::path::PathBuf,
        graph_type: GraphType,
        db: String,
        insert_if_not_exists: bool,
    ) -> TerminusDBResult<Self::DocumentResult> {
        // println!("TDB client doc replace file: {}", docs.display());
        //
        // // todo: replace with parture_common::json_or_zip_to_value
        // let json : serde_json::Value = {
        //     // zipped instance file
        //     if docs.to_str().unwrap().ends_with(".json.zip") {
        //         let file = File::open(docs)
        //             .expect("file not found");
        //         let mut archive = zip::ZipArchive::new(file)
        //             .expect("could not open zip archive");
        //         let mut zippedfile = archive.by_index(0)
        //             .expect("could not find first zip file entry");
        //
        //         serde_json::from_reader(zippedfile)
        //             .expect("unable to create reader from zip file")
        //     }
        //
        //     // unzipped instance file
        //     else {
        //         serde_json::from_reader(File::open(&docs)?)
        //             .expect(&*format!("unable to create reader from regular file: {}", docs.display()))
        //     }
        // };
        //
        // let normalized
        //     = terminusdb_schema::normalize(json);
        //
        // // dump
        // // let mut output = File::create("normalized.test.json").unwrap();
        // // write!(output, "{}", serde_json::to_string(&normalized).unwrap());
        //
        // println!("calling doc_replace_all_json()");
        //
        // // import
        // self.doc_replace_all_json(
        //     &normalized,
        //     graph_type,
        //     db,
        //     insert_if_not_exists)

        todo!()
    }

    // todo: make stream, but the impl Stream is not supported in trait,
    // so we have to use BoxedStream
    fn doc_replace_all_json_files(
        &self,
        docs: &std::path::Path,
        graph_type: GraphType,
        db: String,
        insert_if_not_exists: bool,
    ) -> TerminusDBResult<()> {
        // for e in glob(format!("{}/*.json", docs.display()).as_str())
        //     .expect("Failed to read glob pattern")
        // {
        //     self.doc_replace_all_json_file(e?, graph_type, db.clone(), insert_if_not_exists)?;
        // }
        // for e in glob(format!("{}/*.json.zip", docs.display()).as_str())
        //     .expect("Failed to read glob pattern")
        // {
        //     self.doc_replace_all_json_file(e?, graph_type, db.clone(), insert_if_not_exists)?;
        // }
        // Ok(())
        todo!()
    }

    fn doc_get(&self) -> TerminusDBResult<()>;

    // Remove method using legacy trait ToSerializedQuery
    // fn query(
    //     &self,
    //     db: impl AsRef<str>,
    //     query: impl ToSerializedQuery,
    // ) -> TerminusDBResult<QueryResult>;

    // Remove method using legacy type TypedQuery1
    // fn query_all_var1<T: DeserializeOwned>(
    //     &self,
    //     db: impl AsRef<str>,
    //     query: &TypedQuery1<T>,
    // ) -> TerminusDBResult<Vec<T>> {
    //     let res = self
    //         .query(db, query)
    //         .expect("error in WOQL query")
    //         .woql_result();
    //
    //     res.get_variable_values_typed(query.var1())
    // }
}
