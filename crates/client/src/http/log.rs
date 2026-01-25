//! Log and commit tracking operations

#[cfg(not(target_arch = "wasm32"))]
use crate::log::{CommitLogIterator, EntityIterator, LogEntry, LogOpts};

use {
    crate::{spec::BranchSpec, EntityID, TDBInstanceDeserializer},
    ::tracing::{debug, instrument},
    anyhow::Context,
    serde::Deserialize,
    terminusdb_schema::{GraphType, ToJson, ToTDBInstance},
    terminusdb_woql2::prelude::Query as Woql2Query,
    terminusdb_woql_builder::prelude::{node, vars, Var, WoqlBuilder},
};

use super::TerminusDBModel;

/// Log and commit tracking methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    // returns commit log entries from new to old
    // todo: accept parameter to define ordering
    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.get_entries",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            offset = opts.offset.unwrap_or(0),
            count = opts.count.unwrap_or(10),
            verbose = opts.verbose
        ),
        err
    )]
    pub async fn log(&self, spec: &BranchSpec, opts: LogOpts) -> anyhow::Result<Vec<LogEntry>> {
        let LogOpts {
            offset,
            verbose,
            count,
        } = opts;

        let uri = self
            .build_url()
            .endpoint("log")
            .simple_database(&spec.db)
            .log_params(offset.unwrap_or_default(), count.unwrap_or(10), verbose)
            .build();

        debug!("retrieving log at {}...", &uri);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        self.parse_response(res).await
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn log(&self, _spec: &BranchSpec, _opts: LogOpts) -> anyhow::Result<Vec<LogEntry>> {
        // WASM stub - implement as needed
        Err(anyhow::anyhow!("log not implemented for WASM"))
    }

    /// Get the latest commit ID for a branch
    ///
    /// This is a convenience method that retrieves only the most recent commit
    /// from the branch log. Returns the commit identifier as a `CommitId`.
    ///
    /// # Arguments
    /// * `spec` - Branch specification indicating which branch to query
    ///
    /// # Returns
    /// The commit identifier of the latest commit (HEAD)
    ///
    /// # Example
    /// ```rust
    /// let latest = client.get_latest_commit_id(&branch_spec).await?;
    /// println!("Latest commit: {}", latest);
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.get_latest_commit_id",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch
        ),
        err
    )]
    pub async fn get_latest_commit_id(&self, spec: &BranchSpec) -> anyhow::Result<crate::CommitId> {
        let opts = LogOpts {
            offset: None,
            count: Some(1),
            verbose: false,
        };

        let entries = self.log(spec, opts).await?;

        entries
            .into_iter()
            .next()
            .map(|entry| crate::CommitId::new(entry.identifier))
            .ok_or_else(|| anyhow::anyhow!("No commits found in branch"))
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn get_latest_commit_id(
        &self,
        _spec: &BranchSpec,
    ) -> anyhow::Result<crate::CommitId> {
        Err(anyhow::anyhow!(
            "get_latest_commit_id not implemented for WASM"
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.iterate",
        skip(self),
        fields(
            db = %db.db,
            branch = ?db.branch,
            offset = opts.offset.unwrap_or(0),
            count = opts.count.unwrap_or(10),
            verbose = opts.verbose
        )
    )]
    pub async fn log_iter(&self, db: BranchSpec, opts: LogOpts) -> CommitLogIterator {
        CommitLogIterator::new(self.clone(), db, opts)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn log_iter(&self, _db: BranchSpec, _opts: LogOpts) -> CommitLogIterator {
        // WASM stub - return a dummy iterator
        panic!("log_iter not implemented for WASM")
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.entity_iterate",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            offset = opts.offset.unwrap_or(0),
            count = opts.count.unwrap_or(10),
            verbose = opts.verbose
        )
    )]
    pub async fn entity_iter<
        T: TerminusDBModel + 'static,
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

    #[cfg(target_arch = "wasm32")]
    pub async fn entity_iter<
        T: TerminusDBModel + 'static,
        Deser: TDBInstanceDeserializer<T> + 'static,
    >(
        &self,
        _spec: BranchSpec,
        _deserializer: Deser,
        _opts: LogOpts,
    ) -> EntityIterator<T, Deser>
    where
        T: Send,
        Deser: Send,
    {
        // WASM stub
        panic!("entity_iter not implemented for WASM")
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.build_commit_query",
        skip(self, commit),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier,
            limit = limit.unwrap_or(1000)
        )
    )]
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

    #[cfg(target_arch = "wasm32")]
    pub async fn commit_added_entities_query<T: ToTDBInstance>(
        &self,
        _spec: &BranchSpec,
        _commit: &LogEntry,
        _limit: Option<usize>,
    ) -> Woql2Query {
        // WASM stub
        panic!("commit_added_entities_query not implemented for WASM")
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.commit_added_entities_ids",
        skip(self, commit),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier,
            limit = limit.unwrap_or(1000)
        ),
        err
    )]
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
        let res = self.query_raw(Some(spec.clone()), json_query, None).await?;

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

    #[cfg(target_arch = "wasm32")]
    pub async fn commit_added_entities_ids<T: ToTDBInstance>(
        &self,
        _spec: &BranchSpec,
        _commit: &LogEntry,
        _limit: Option<usize>,
    ) -> anyhow::Result<Vec<EntityID>> {
        // WASM stub
        Err(anyhow::anyhow!(
            "commit_added_entities_ids not implemented for WASM"
        ))
    }

    /// return ID for first entity of given type that was created by the given commit
    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.first_commit_created_entity_id",
        skip(self, commit),
        fields(
            db = %db.db,
            branch = ?db.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier
        ),
        err
    )]
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

    #[cfg(target_arch = "wasm32")]
    pub async fn first_commit_created_entity_id<T: ToTDBInstance>(
        &self,
        _db: &BranchSpec,
        _commit: &LogEntry,
    ) -> anyhow::Result<Option<String>> {
        // WASM stub
        Err(anyhow::anyhow!(
            "first_commit_created_entity_id not implemented for WASM"
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.first_commit_created_entity",
        skip(self, commit, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier
        ),
        err
    )]
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

    #[cfg(target_arch = "wasm32")]
    pub async fn first_commit_created_entity<T: ToTDBInstance>(
        &self,
        _spec: &BranchSpec,
        _commit: &LogEntry,
        _deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Option<T>> {
        // WASM stub
        Err(anyhow::anyhow!(
            "first_commit_created_entity not implemented for WASM"
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.all_commit_created_entity_ids",
        skip(self, commit),
        fields(
            db = %db.db,
            branch = ?db.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier
        ),
        err
    )]
    pub async fn all_commit_created_entity_ids<T: ToTDBInstance>(
        &self,
        db: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Vec<EntityID>> {
        self.commit_added_entities_ids::<T>(db, commit, Some(1000))
            .await
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn all_commit_created_entity_ids<T: ToTDBInstance>(
        &self,
        _db: &BranchSpec,
        _commit: &LogEntry,
    ) -> anyhow::Result<Vec<EntityID>> {
        // WASM stub
        Err(anyhow::anyhow!(
            "all_commit_created_entity_ids not implemented for WASM"
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[instrument(
        name = "terminus.log.all_commit_created_entities",
        skip(self, commit, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            commit_id = %commit.identifier
        ),
        err
    )]
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

    #[cfg(target_arch = "wasm32")]
    pub async fn all_commit_created_entities<T: ToTDBInstance>(
        &self,
        _spec: &BranchSpec,
        _commit: &LogEntry,
        _deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<T>> {
        // WASM stub
        Err(anyhow::anyhow!(
            "all_commit_created_entities not implemented for WASM"
        ))
    }
}
