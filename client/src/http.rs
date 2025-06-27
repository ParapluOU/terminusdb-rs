#[cfg(not(target_arch = "wasm32"))]
use {
    futures_util::Stream,
    glob::glob,
    itertools::Itertools,
    reqwest::{Client, Response, StatusCode},
    std::process::Command,
    subprocess::{Exec, Redirection},
    tempfile::tempfile,
};

use {
    crate::{
        document::{DocumentInsertArgs, DocumentType, GetOpts},
        spec::BranchSpec,
        *,
    },
    ::log::{debug, error, trace, warn},
    anyhow::{anyhow, Context},
    enum_variant_macros::*,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    serde_json::{json, Value},
    std::{
        collections::{HashMap, HashSet},
        fmt::Debug,
        fs::File,
        hash::Hash,
        io::Write,
        path::PathBuf,
        time::Instant,
    },
    tap::{tap::Tap, Pipe, TapFallible},
    url::Url,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::log::{CommitLogIterator, CommitState, LogEntry, LogOpts};

use terminusdb_schema::{ToJson, ToTDBInstance};
// Add imports for woql2 and builder
use terminusdb_woql2::prelude::Query as Woql2Query;
use terminusdb_woql_builder::prelude::{node, vars, Var, WoqlBuilder}; // Import WoqlBuilder too

pub type EntityID = String;

#[derive(Clone, Debug)]
pub struct TerminusDBHttpClient {
    pub endpoint: Url,
    // Use conditional compilation for the http client
    #[cfg(not(target_arch = "wasm32"))]
    http: Client,
    /// user auth for this user
    user: String,
    /// this user's password
    pass: String,
    /// organization that we are logging in for
    org: String,
}

// Wrap the entire impl block with a conditional compilation attribute
#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    pub async fn local_node() -> Self {
        Self::new(
            Url::parse("http://localhost:6363").unwrap(),
            "admin",
            "root",
            "admin",
        )
        .await
        .unwrap()
    }

    pub async fn local_node_with_database(db: &str) -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database(db).await
    }

    pub async fn local_node_test() -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database("test").await
    }

    pub async fn new(mut endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        let err = format!("Cannot modify segments for endpoint: {}", &endpoint);

        endpoint.path_segments_mut().expect(&err).push("api");

        Ok(Self {
            user: user.to_string(),
            pass: pass.to_string(),
            endpoint,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            org: org.to_string(),
        })
    }

    pub async fn new_with_database(
        endpoint: Url,
        user: &str,
        pass: &str,
        db: &str,
        org: &str,
    ) -> anyhow::Result<Self> {
        let client = Self::new(endpoint, user, pass, org).await?;
        client.ensure_database(db).await
    }

    // pub fn doc_replace_all_json_files_stream(
    //     &self,
    //     docs: std::path::PathBuf,
    //     graph_type: GraphType,
    //     db: String,
    //     insert_if_not_exists: bool,
    // ) -> impl Stream<Item = TerminusDBResult<std::path::PathBuf>> + '_ {
    //     async_stream::stream! {
    //         for e in glob(format!("{}/*.json", docs.display()).as_str()).expect("Failed to read glob pattern") {
    //             let e2 = e.expect("PathBuf was empty");
    //             yield self.doc_replace_all_json_file(
    //                 e2.clone(),
    //                 graph_type,
    //                 db.clone(),
    //                 insert_if_not_exists).map(|_| e2);
    //         }
    //         for e in glob(format!("{}/*.json.zip", docs.display()).as_str()).expect("Failed to read glob pattern") {
    //             let e2 = e.expect("PathBuf was empty");
    //             yield self.doc_replace_all_json_file(
    //                 e2.clone(),
    //                 graph_type,
    //                 db.clone(),
    //                 insert_if_not_exists).map(|_| e2);
    //         }
    //     }
    // }

    pub async fn ensure_database(&self, db: &str) -> anyhow::Result<Self> {
        let uri = format!("{}/db/{}/{}", &self.endpoint, self.org, db);

        debug!("post uri: {}", &uri);

        // todo: author should probably be node name
        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "comment": "Song database specific for this node",
                    "label": db,
                    "public": true,
                    "schema": true
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to ensure database")?;

        // todo: use parse_response()
        if ![200, 400].contains(&res.status().as_u16()) {
            error!("could not ensure database");

            Err(TerminusDBAdapterError::Other(format!(
                "request failed: {:#?}",
                res.text().await?
            )))?;
        }

        // todo: dont print if it already existed
        debug!("ensured database {}", db);

        Ok(self.clone())
    }

    pub async fn delete_database(&self, db: &str) -> anyhow::Result<Self> {
        let uri = format!("{}/db/{}/{}", &self.endpoint, self.org, db);

        debug!("deleting database {}", db);

        self.http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete database")?;

        Ok(self.clone())
    }

    pub async fn insert_entity_schema<S: ToTDBSchema>(
        &self,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<()> {
        self.ensure_database(&args.spec.db)
            .await
            .context("ensuring database")?;

        let root = S::to_schema();

        debug!("inserting entity schema for {}...", root.class_name());

        let subs = S::to_schema_tree();

        // panic!("{:#?}", &subs);

        self.insert_documents(subs.iter().collect(), args.as_schema())
            .await
            .context("insert_documents()")?;

        debug!("inserted schema into TerminusDB");

        Ok(())
    }

    pub async fn insert_schema(
        &self,
        schema: &crate::Schema,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        self.insert_document(
            schema,
            args.tap_mut(|a| {
                a.ty = DocumentType::Schema;
            }),
        )
        .await
    }

    pub async fn has_instance<I: ToTDBInstance + Debug + Serialize>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> bool {
        match model.to_instance(None).gen_id() {
            None => {
                return false;
            }
            Some(id) => self.has_document(&id, args.as_ref()).await,
        }
    }

    pub async fn insert_instance<I: ToTDBInstance + Debug + Serialize>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<HashMap<String, TDBInsertInstanceResult>> {
        let instance = model.to_instance(None);

        let gen_id = instance.gen_id();

        if !args.force && self.has_instance(model, args.clone()).await {
            // todo: make strongly typed ID helper
            let id = gen_id.unwrap().split("/").last().unwrap().to_string();

            warn!("not inserted because it already exists");

            // todo: if the document is configured to not use HashKey, this cannot work
            return Ok(HashMap::from([(
                id.clone(),
                TDBInsertInstanceResult::AlreadyExists(id),
            )]));
        }

        debug!("inserting instance...");

        let res = self
            .insert_instances(instance, args.clone())
            .await
            .tap_err(|_| dump_schema::<I>())?;

        dbg!(&res);

        // Get the first inserted ID from the result HashMap's values
        let inserted_tdb_id = match res.values().next() {
            Some(TDBInsertInstanceResult::Inserted(id)) => id.clone(),
            Some(TDBInsertInstanceResult::AlreadyExists(id)) => {
                // If it already existed, we might still want to verify it exists
                id.clone()
            }
            None => return Err(anyhow!("Insert operation did not return any ID")),
        };

        // assert
        self.get_document(&inserted_tdb_id, args.as_ref(), GetOpts::default())
            .await
            .expect("expected document to exist!");

        Ok(res)
    }

    // pub fn insert_instance_chunked<I: ToTDBInstance>(
    //     &self,
    //     model: &I,
    //     args: DocumentInsertArgs,
    // ) -> anyhow::Result<&Self> {
    //     let mut instances = model.to_instance_tree();
    //
    //     for instance in &mut instances {
    //         instance.set_random_key_prefix();
    //     }
    //
    //     let mut models = instances.iter().collect();
    //
    //     dedup_instances_by_id(&mut models);
    //
    //     for instance in instances.iter().rev() {
    //         self.insert_document(instance, args.clone())?;
    //     }
    //
    //     Ok(self)
    // }

    pub async fn insert_instances(
        &self,
        models: impl IntoBoxedTDBInstances,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<HashMap<String, TDBInsertInstanceResult>> {
        args.ty = DocumentType::Instance;

        // let mut instances = models
        //     .into_iter()
        //     .map(|m| m.to_instance_tree_flatten(true))
        //     .flatten()
        //     .collect::<Vec<_>>();

        let mut instances = models.into_boxed().to_instance_tree_flatten(true);

        for instance in &mut instances {
            instance.set_random_key_prefix();
            // instance.ref_props = false; // todo: make configurable
            instance.capture = true;
        }

        // kick out all references because they will lead to schema failure errors on insertion
        instances.retain(|i| !i.is_reference());

        let mut models = instances.iter().collect();

        // dedup_instances_by_id(&mut models);

        self.insert_documents(models, args).await
    }

    pub async fn insert_documents_by_schema_type<T: ToTDBSchema>(
        &self,
        mut model: Vec<&Instance>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<HashMap<String, TDBInsertInstanceResult>> {
        // dedup_instances_by_id(&mut model);

        let selection = model
            .into_iter()
            .filter(|instance| instance.schema.class_name() == &T::schema_name())
            .collect::<Vec<_>>();

        self.insert_documents(selection, args).await
    }

    pub async fn insert_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<HashMap<String, TDBInsertInstanceResult>> {
        let ty = args.ty.to_string().to_lowercase();

        let uri = format!(
            "{}/document/{}/{}?author={}&message={}&graph_type={}&create=true",
            &self.endpoint,
            &self.org,
            &args.spec.db,
            &args.author,
            (*urlencoding::encode(&args.message)).to_string(),
            &ty
        );

        let mut to_jsoned = model
            .into_iter()
            .map(|t| t.to_json())
            .rev()
            .collect::<Vec<_>>();

        // for (i, j) in &mut to_jsoned.iter_mut().enumerate() {
        //     j.as_object_mut()
        //         .unwrap()
        //         .insert("@capture".to_string(), format!("v{}", i).into());
        // }

        dedup_documents_by_id(&mut to_jsoned);

        eprintln!("inserting document(s): {:#?}", &to_jsoned);

        let json = serde_json::to_string(&to_jsoned).unwrap();

        trace!(
            "about to insert {} at URI {}: {:#?}",
            &ty,
            &uri,
            // todo: add trailing "..." or "(truncated)" if message was longer
            &json[0..(10000.min(json.len()))]
        );

        if args.ty.is_instance() {
            // panic!("{:#?}", &to_jsoned);
        }

        // todo: author should probably be node name
        // todo: dont clone just for the failed paload dumping
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.clone())
            .send()
            .await?;

        let parsed = self.parse_response::<Vec<String>>(res).await;

        trace!("parsed response from insert_documents(): {:#?}", &parsed);

        // panic!("{:#?}", &parsed);

        // todo: create proper response format for this
        if let Err(e) = parsed {
            error!("request error: {:#?}", &e);

            // dump request payload to file for debugging
            dump_failed_payload(&json);

            return Err(e);
        }

        debug!("inserted {} into TerminusDB", ty);

        Ok(parsed?
            .into_iter()
            .map(|id| (id.clone(), TDBInsertInstanceResult::Inserted(id)))
            .collect())
    }

    pub async fn insert_document(
        &self,
        model: &impl ToJson,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        let json = model.to_json();

        let ty = args.ty.to_string().to_lowercase();

        let uri = format!(
            "{}/document/{}/{}?author={}&message={}&graph_type={}&create=true",
            &self.endpoint,
            &self.org,
            &args.spec.db,
            &args.author,
            (*urlencoding::encode(&args.message)).to_string(),
            &ty
        );

        debug!("about to insert {} at URI {}: {:#?}", &ty, &uri, &json);

        // todo: author should probably be node name
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.to_string())
            .send()
            .await?;

        self.parse_response::<Value>(res).await?;

        debug!("inserted {} into TerminusDB", ty);

        Ok(self.clone())
    }

    #[pseudonym::alias(verify_connection)]
    pub async fn info(&self) -> anyhow::Result<Info> {
        let uri = format!("{}/info", &self.endpoint,);
        debug!(
            "📡 Making HTTP request to TerminusDB info endpoint: {}",
            &uri
        );

        let res = self
            .http
            .get(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context(format!("failed to parse response for {}", &uri))?;

        debug!("📨 Received response from TerminusDB, parsing...");
        self.parse_response(res).await
    }

    pub async fn is_running(&self) -> bool {
        self.info().await.is_ok()
    }

    // returns commit log entries from new to old
    // todo: accept parameter to define ordering
    pub async fn log(&self, spec: &BranchSpec, opts: LogOpts) -> anyhow::Result<Vec<LogEntry>> {
        let LogOpts {
            offset,
            verbose,
            count,
        } = opts;

        let uri = format!(
            "{}/log/{}/{}?start={}&count={}&verbose={}",
            // todo: expose default to Config
            &self.endpoint,
            self.org,
            &spec.db,
            offset.unwrap_or_default(),
            count.unwrap_or(10),
            verbose
        );

        debug!("retrieving log at {}...", &uri);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        self.parse_response(res).await
    }

    async fn parse_response<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<T> {
        let json = res.json::<serde_json::Value>().await?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(r)
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    pub async fn log_iter(&self, db: BranchSpec, opts: LogOpts) -> CommitLogIterator {
        CommitLogIterator::new(self.clone(), db, opts)
    }

    pub async fn entity_iter<
        T: ToTDBInstance + 'static,
        Deser: TDBInstanceDeserializer<T> + 'static,
    >(
        &self,
        spec: BranchSpec,
        deserializer: Deser,
        opts: LogOpts,
    ) -> EntityIterator<T, Deser>
    where
        T: Send,
        Deser: Send,
    {
        EntityIterator::new(
            CommitLogIterator::new(self.clone(), spec, opts),
            deserializer,
        )
    }

    pub async fn commit_added_entities_query<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        limit: Option<usize>,
    ) -> Woql2Query {
        // Build the query using WoqlBuilder
        let db_collection = format!("{}/{}", &self.org, &spec.db);
        let commit_collection = format!("commit/{}", &commit.identifier);
        let type_node_str = format!("@schema:{}", T::schema_name());

        let id_var = vars!("id"); // Define the variable 'id'

        // Start from the innermost query and wrap outwards
        let query_builder = WoqlBuilder::new()
            .added_triple(
                id_var.clone(),             // subject: variable "id"
                "rdf:type",                 // predicate: node "rdf:type"
                node(&type_node_str),       // object: node "@schema:MyType" - Use T::schema_name()
                GraphType::Instance.into(), // graph: "instance" - This makes it AddedQuad conceptually
            )
            .using(commit_collection) // Wrap in commit collection Using
            .using(db_collection) // Wrap in db collection Using
            .limit(limit.unwrap_or(1000) as u64); // Apply the limit

        // Finalize the query into the woql2 structure
        query_builder.finalize()
    }

    pub async fn commit_added_entities_ids<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<EntityID>> {
        // form the query using the builder
        let woql_query = self
            .commit_added_entities_query::<T>(spec, commit, limit)
            .await;

        // Serialize the woql2::Query to JSON-LD
        let json_query = woql_query.to_instance(None).to_json();

        // perform the query using the serialized JSON
        let res = self.query_raw(Some(spec.clone()), json_query).await?;

        // format the error message in case there is a mismatch in deserialization format
        let err = format!("failed to deserialize from Value: {:#?}", &res);

        #[derive(Deserialize)]
        struct ObjectFormat {
            pub id: String,
        }

        // try deserialization
        Ok(res
            .bindings
            .into_iter()
            .map(|bind| serde_json::from_value::<ObjectFormat>(bind))
            .collect::<Result<Vec<_>, _>>()
            .context(err)?
            .into_iter()
            // this is a bit hacky to shave off the schema name, but we dont use that internally
            // todo: when the WOQL query stuff has matured more this should be a helper on
            // the result wrapper struct
            .map(|obj| obj.id.split("/").last().unwrap().to_string())
            .collect())
    }

    /// return ID for first entity of given type that was created by the given commit
    pub async fn first_commit_created_entity_id<T: ToTDBInstance>(
        &self,
        db: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Option<String>> {
        Ok(self
            .commit_added_entities_ids::<T>(db, commit, Some(1))
            .await?
            .first()
            .cloned())
    }

    pub async fn first_commit_created_entity<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Option<T>> {
        match self
            .first_commit_created_entity_id::<T>(spec, commit)
            .await?
        {
            None => Ok(None),
            Some(id) => self
                .get_instance::<T>(&id, spec, deserializer)
                .await
                .map(Some),
        }
    }

    pub async fn all_commit_created_entity_ids<T: ToTDBInstance>(
        &self,
        db: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Vec<EntityID>> {
        self.commit_added_entities_ids::<T>(db, commit, Some(1000))
            .await
    }

    pub async fn all_commit_created_entities<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<T>> {
        let entity_ids = self
            .all_commit_created_entity_ids::<T>(spec, commit)
            .await?;

        let mut results = Vec::with_capacity(entity_ids.len());
        for id in entity_ids {
            results.push(
                self.get_instance::<T>(&id, spec, deserializer)
                    .await
                    .context(format!(
                        "retrieving entity {} #{} from TDB",
                        std::any::type_name::<T>(),
                        &id
                    ))?,
            );
        }

        Ok(results)
    }

    // pub fn resolve_objects_from_commit<T: ToTDBInstance>(
    //     &self,
    //     spec: &BranchSpec,
    //     deserializer: &mut impl TDBInstanceDeserializer<T>,
    //     commit: &CommitState,
    // ) -> anyhow::Result<Vec<T>> {
    //     self.resolve_objects(spec, deserializer, commit.all_added_entities::<T>())
    // }

    // pub fn resolve_objects<T: ToTDBInstance>(
    //     &self,
    //     spec: &BranchSpec,
    //     deserializer: &mut impl TDBInstanceDeserializer<T>,
    //     objects: Vec<&ObjectState>,
    // ) -> anyhow::Result<Vec<T>> {
    //     objects
    //         .into_iter()
    //         .map(|b| self.get_instance::<T>(&b.id, spec, deserializer))
    //         .collect()
    // }

    pub async fn has_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
    ) -> bool {
        let r: anyhow::Result<_> = async {
            let uri = format!(
                "{}/document/{}/{}?id={}&unfold={}&as_list={}",
                &self.endpoint, &self.org, spec.db, id, false, false
            );

            Ok::<Value, anyhow::Error>(
                self.parse_response::<Value>(
                    self.http
                        .get(uri)
                        .basic_auth(&self.user, Some(&self.pass))
                        .send()
                        .await?,
                )
                .await?,
            )
        }
        .await;

        r.is_ok()
    }

    /// ID here should manually include the type, like
    /// "Song/myid"
    pub async fn get_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<serde_json::Value> {
        if !self.has_document(id, spec).await {
            Err(anyhow!("document #{} does not exist", id))?
        }

        let uri = format!(
            "{}/document/{}/{}?id={}&unfold={}&as_list={}",
            &self.endpoint, &self.org, spec.db, id, opts.unfold, opts.as_list
        );

        debug!("retrieving document at {}...", &uri);

        let start = Instant::now();

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        debug!("retrieved TDB document with status code: {}", res.status());

        let res = self.parse_response::<Value>(res).await?;

        debug!("retrieved TDB document in {:?}", start.elapsed());

        Ok(res)
    }

    pub fn format_id<T: ToTDBInstance>(id: &str) -> String {
        if id.contains("/") {
            id.to_string()
        } else {
            format!("{}/{}", T::schema_name(), id)
        }
    }

    pub async fn get_instance<Target: ToTDBInstance>(
        &self,
        // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target>
    where
        Target:,
    {
        let doc_id = Self::format_id::<Target>(id);

        let json_instance_doc = self.get_document(&doc_id, spec, Default::default()).await?;

        let res = deserializer.from_instance(json_instance_doc.clone());

        match res {
            Ok(t) => Ok(t),
            Err(err) => Err(err).context(format!(
                "TerminusHTTPClient failed to deserialize Instance. See: {}",
                dump_json(&json_instance_doc).display()
            )),
        }
    }

    // Refactor query to accept Woql2Query
    pub async fn query<T: Debug + DeserializeOwned>(
        &self,
        db: Option<BranchSpec>,
        query: Woql2Query, // Changed input type
    ) -> anyhow::Result<WOQLResult<T>> {
        // Serialize the query to JSON-LD here
        let json_query = query.to_instance(None).to_json();
        let woql_context = crate::Context::woql().to_json();
        // self.query_raw(db, Value::Array(vec!(woql_context, json_query))).await // Pass serialized JSON to query_raw
        self.query_raw(db, json_query).await // Pass serialized JSON to query_raw
    }

    // query_raw remains the same, accepting serde_json::Value
    pub async fn query_raw<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<WOQLResult<T>> {
        let uri = match spec {
            None => {
                format!("{}/woql", &self.endpoint)
            }
            Some(spc) => {
                format!("{}/woql/{}/{}", &self.endpoint, &self.org, spc.db)
            }
        };

        eprintln!("querying at {}...: {:#?}", &uri, &query);

        let json = json!({
            "query": query
        });

        let json = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        let json = self.parse_response(res).await?;

        trace!("query result: {:#?}", &json);

        Ok(json)
    }

    // todo: roll into ORM-like model
    pub async fn query_instances<T: TdbModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
        query: impl InstanceQueryable<Model = T>,
    ) -> anyhow::Result<Vec<T>> {
        query.apply(self, spec, limit, offset).await
    }

    // todo: roll into ORM-like model
    pub async fn list_instances<T: TdbModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<T>> {
        self.query_instances(spec, limit, offset, ListModels::<T>::default())
            .await
    }

    /// Count the total number of instances of a specific type in the database
    pub async fn count_instances<T: ToTDBSchema>(
        &self,
        spec: &BranchSpec,
    ) -> anyhow::Result<usize> {
        let count_var = vars!("Count");
        let instance_var = vars!("Instance");
        
        // Build a query to count instances of the specific type using the isa2 shortcut
        let query = WoqlBuilder::new()
            .isa2::<T>(&instance_var)
            .count(count_var.clone())
            .select(vec![count_var.clone()])
            .finalize();

        // Execute the query
        let result = self.query::<std::collections::HashMap<String, serde_json::Value>>(
            Some(spec.clone()), 
            query
        ).await?;

        // Extract count from the result
        if let Some(binding) = result.bindings.first() {
            if let Some(count_value) = binding.get(&*count_var) {
                if let Some(count) = count_value.as_u64() {
                    return Ok(count as usize);
                }
            }
        }
        
        Ok(0)
    }
}

// Add a separate impl block for WASM
#[cfg(target_arch = "wasm32")]
impl TerminusDBHttpClient {
    // Implement a stub or alternative implementation for WASM
    // This is just a basic example, you'll need to adjust based on your needs
    pub async fn new(endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint,
            user: user.to_string(),
            pass: pass.to_string(),
            org: org.to_string(),
        })
    }

    // Implement other methods as needed for WASM
}

#[derive(Debug, Clone, PartialEq)]
pub enum TDBInsertInstanceResult {
    /// inserted entity, returning ID
    Inserted(String),
    /// entity already exists, returning ID
    AlreadyExists(String),
}

//
// HELPERS
// todo: move to helpers module
//

pub fn dedup_instances_by_id(instances: &mut Vec<&Instance>) {
    let mut seen_ids = HashSet::new();
    instances.retain(|item| {
        match &item.id {
            Some(id) => seen_ids.insert(id.clone()), // insert returns true if the value was not present in the set
            None => true,                            // keep items with None id
        }
    });
}

fn dedup_documents_by_id(values: &mut Vec<Value>) {
    let mut seen_ids = HashSet::new();
    values.retain(|value| {
        if let Some(id) = value.get("@id").and_then(|id| id.as_str()) {
            seen_ids.insert(id.to_string())
        } else {
            true
        }
    });
}

fn dump_failed_payload(payload: &str) {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-request-{}.log.json", current_datetime);

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

fn dump_schema<S: ToTDBSchema>() {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-schema-{}.log.json", current_datetime);

    let schema_json = serde_json::Value::Array(
        S::to_schema_tree()
            .into_iter()
            .map(|s| s.to_json())
            .collect(),
    );

    let payload = serde_json::to_string_pretty(&schema_json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

fn dump_json(json: &Value) -> PathBuf {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-retrieved-json-{}.log.json", current_datetime);

    let payload = serde_json::to_string_pretty(json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => {
            debug!(
                "Successfully dumped success response to file {}",
                log_filename
            );
            PathBuf::from(log_filename)
        }
        Err(e) => panic!("Could not write to file: {}", e),
    }
}

// #[test]
// fn test_remote_query() {
//     let client =
//         TerminusDBHttpClient::new("http://51.68.146.185:6363", "admin", "root", "admin").unwrap();
//
//     let cnt = var!(Cnt);
//     let n = var!(N);
//
//     let q = { select!([cnt], count(distinct!([n], is_a(n, "ScoreTree")), cnt)) };
//
//     println!("{}", &q.to_rest_query());
//
//     // (count(distinct([N], isa(N, 'ScoreTree')), Cnt))
//     let res = client.query("admin/scores", q);
//
//     dbg!(&res);
//
//     assert!(res.is_ok());
// }
