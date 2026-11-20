//! Schema-related operations

use crate::{ResponseWithHeaders, TDBInsertInstanceResult};
use std::collections::HashMap;
use tap::Pipe;
use {
    crate::{document::DocumentInsertArgs, Schema},
    ::tracing::{debug, instrument},
    anyhow::Context as AnyhowContext,
    terminusdb_schema::{json::ToJson, Context as TDBContext, ToTDBSchema, ToTDBSchemas},
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
    #[instrument(
        name = "terminus.schema.insert_entity",
        skip(self, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %S::schema_name()
        ),
        err
    )]
    #[pseudonym::alias(schema, insert_schema_tree)]
    pub async fn insert_entity_schema<S: ToTDBSchema>(
        &self,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<()> {
        self.insert_schema_trees::<(S,)>(args).await
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
    #[instrument(
        name = "terminus.schema.insert_raw",
        skip(self, schema, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            schema_type = %schema.class_name()
        ),
        err
    )]
    pub async fn insert_schema(
        &self,
        schema: &Schema,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<&Self> {
        Ok(self
            .insert_schema_instances(vec![schema.clone()], args)
            .await?
            .pipe(|_| self))
    }

    /// insert only the given Schemas. because the arguments are schema instances.
    /// we cannot derive the schema tree dependencies from them
    #[instrument(
        name = "terminus.schema.insert_instances",
        skip(self, schemas, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            schema_count = schemas.len()
        ),
        err
    )]
    pub async fn insert_schema_instances(
        &self,
        schemas: Vec<Schema>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        debug!(
            "inserting schema instances: {:#?}",
            schemas.iter().map(|s| s.class_name()).collect::<Vec<_>>()
        );
        self.insert_documents(schemas.iter().collect(), args.as_schema())
            .await
    }

    /// Inserts schemas for multiple strongly-typed models using tuple types.
    ///
    /// This method provides a type-safe way to insert schemas for multiple models
    /// at once using tuple type parameters. The tuple can contain up to 8 different
    /// types that implement `ToTDBSchema`.
    ///
    /// # Type Parameters
    /// * `T` - A tuple of types that implement `ToTDBSchema` (e.g., `(Person, Company, Product)`)
    ///
    /// # Arguments
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Example
    /// ```rust,ignore
    /// #[derive(TerminusDBModel)]
    /// struct Person { name: String, age: i32 }
    ///
    /// #[derive(TerminusDBModel)]
    /// struct Company { name: String, employees: Vec<Person> }
    ///
    /// #[derive(TerminusDBModel)]
    /// struct Product { name: String, price: f64 }
    ///
    /// // Insert schemas for multiple types at once
    /// client.insert_schemas::<(Person, Company, Product)>(args).await?;
    /// ```
    ///
    /// # Alternative: Using the `schemas!` macro
    /// ```rust,ignore
    /// use terminusdb_schema::schemas;
    ///
    /// // More flexible approach using macro
    /// let schemas = schemas!(Person, Company, Product);
    /// client.insert_documents(schemas, args.as_schema()).await?;
    /// ```
    ///
    /// # Note
    /// The database will be automatically created if it doesn't exist.
    /// For more than 8 types, consider using the `schemas!` macro approach instead.
    #[instrument(
        name = "terminus.schema.insert_multiple",
        skip(self, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch
        ),
        err
    )]
    #[pseudonym::alias(insert_schema_trees)]
    pub async fn insert_schemas<T: ToTDBSchemas>(
        &self,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<()> {
        self.ensure_database(&args.spec.db)
            .await
            .with_context(|| "ensuring database")?;

        let schemas = T::to_schemas();

        let count = schemas.len();

        debug!("inserting {} schemas", count);

        self.insert_schema_instances(schemas, args.as_schema())
            .await
            .with_context(|| "insert_documents()")?;

        debug!("inserted {} schemas into TerminusDB", count);

        Ok(())
    }

    /// Inserts a schema context and schemas into the database.
    ///
    /// The context is committed first, on its own, and the schemas follow in a
    /// second commit — prefixed property keys only resolve against a context
    /// that is already committed (see the note in the body).
    ///
    /// This is essential for XSD-derived schemas where namespacing prevents conflicts
    /// when multiple schemas have overlapping class names. The Context defines:
    /// - `@schema`: The namespace for resolving class IDs (e.g., `http://example.com/book#`)
    /// - `@base`: The namespace for instance IDs (e.g., `http://example.com/book/data/`)
    ///
    /// # Arguments
    /// * `context` - The schema context with namespace configuration
    /// * `schemas` - The schema definitions to insert
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Example
    /// ```rust,ignore
    /// use terminusdb_xsd::XsdModel;
    ///
    /// let model = XsdModel::from_file("book.xsd", None::<&str>)?;
    /// let context = model.context().clone();
    /// let schemas = model.schemas().to_vec();
    ///
    /// // Insert with namespace context - class "BookType" resolves to
    /// // "http://example.com/book#BookType" instead of the default namespace
    /// client.insert_schema_with_context(context, schemas, args).await?;
    /// ```
    ///
    /// # Multiple Schemas with Same Class Names
    /// When inserting multiple XSD schemas that have overlapping class names,
    /// use different contexts to prevent conflicts:
    /// ```rust,ignore
    /// // First schema: http://example.com/book#BookType
    /// client.insert_schema_with_context(book_context, book_schemas, args).await?;
    ///
    /// // Second schema: http://example.com/library#BookType
    /// client.insert_schema_with_context(library_context, library_schemas, args).await?;
    /// ```
    #[instrument(
        name = "terminus.schema.insert_with_context",
        skip(self, context, schemas, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            schema_namespace = %context.schema,
            schema_count = schemas.len()
        ),
        err
    )]
    pub async fn insert_schema_with_context(
        &self,
        context: TDBContext,
        schemas: Vec<Schema>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        self.ensure_database(&args.spec.db)
            .await
            .with_context(|| "ensuring database")?;

        debug!(
            "inserting schema context {} with {} schemas",
            context.schema,
            schemas.len()
        );

        // The context goes in its own commit, BEFORE the schemas.
        //
        // The server expands a prefixed key (e.g. `ditaarch:DITAArchVersion`) by
        // looking the prefix up in the context that is *already committed*
        // (`context_prefix_expand/3` in src/core/query/jsonld.pl, which throws
        // key_has_unknown_prefix otherwise). A context sent in the same batch as
        // the documents that rely on it is not yet visible to that lookup, so
        // batching them fails with api:UnknownPrefixError even though the prefix
        // is right there in the payload. Two commits is the only ordering that
        // works; verified against v12.1.0-rc3.
        let context_json = context.to_json();
        self.insert_documents(vec![&context_json], args.clone().as_schema())
            .await
            .with_context(|| "inserting schema context")?;

        // Use to_namespaced_json() to fully qualify class references in properties
        // and inheritance when a namespace base is present (e.g., XSD-derived schemas).
        // This is necessary because TerminusDB doesn't expand short class names using
        // the Context's @schema - it uses them literally, defaulting to terminusdb:///schema#.
        let schema_jsons: Vec<_> = schemas.iter().map(|s| s.to_namespaced_json()).collect();
        let schema_refs: Vec<&serde_json::Value> = schema_jsons.iter().collect();

        let result = self.insert_documents(schema_refs, args.as_schema()).await?;

        debug!(
            "inserted context + {} schemas into TerminusDB",
            schemas.len()
        );

        Ok(result)
    }
}
