//! Schema-related operations

use {
    crate::{document::DocumentInsertArgs, document::DocumentType, Schema},
    ::log::debug,
    anyhow::Context,
    tap::{Tap, TapFallible},
    terminusdb_schema::ToTDBSchema,
};

/// Schema management methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Inserts the schema for a strongly-typed model into the database.
    ///
    /// This function automatically generates and inserts the schema definition
    /// for a type that implements `ToTDBSchema` (typically via `#[derive(TerminusDBModel)]`).
    ///
    /// # Type Parameters
    /// * `S` - A type that implements `ToTDBSchema` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// // Insert the schema for the User type
    /// client.insert_entity_schema::<User>(args).await?;
    /// 
    /// // Now you can insert User instances
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// client.insert_instance(&user, args).await?;
    /// ```
    ///
    /// # Note
    /// The database will be automatically created if it doesn't exist.
    #[pseudonym::alias(schema)]
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

    /// Inserts a raw schema definition into the database.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`insert_entity_schema`](Self::insert_entity_schema) for typed model schemas
    ///
    /// This function inserts a manually constructed schema definition. It's typically
    /// used for advanced scenarios or when working with dynamic schemas.
    ///
    /// # Arguments
    /// * `schema` - The schema definition to insert
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_schema::Schema;
    /// 
    /// let schema = Schema::Class { /* schema definition */ };
    /// client.insert_schema(&schema, args).await?;
    /// ```
    pub async fn insert_schema(
        &self,
        schema: &Schema,
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
}