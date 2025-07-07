//! Schema-related operations

use std::collections::{HashMap, HashSet};
use tap::Pipe;
use {
    crate::{document::DocumentInsertArgs, document::DocumentType, Schema},
    ::log::debug,
    anyhow::Context,
    tap::{Tap, TapFallible},
    terminusdb_schema::{ToTDBSchema, ToTDBSchemas},
};
use crate::{ResponseWithHeaders, TDBInsertInstanceResult};

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
    pub async fn insert_schema(
        &self,
        schema: &Schema,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<&Self> {
        Ok(self.insert_schema_instances(vec!(schema.clone()), args).await?.pipe(|_| self))
    }

    /// insert only the given Schemas. because the arguments are schema instances.
    /// we cannot derive the schema tree dependencies from them
    pub async fn insert_schema_instances(
        &self,
        schemas: Vec<Schema>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        debug!("inserting schema instances: {:#?}", schemas.iter().map(|s| s.class_name()).collect::<Vec<_>>());
        self.insert_documents(schemas.iter().collect(), args.as_schema()).await
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
    #[pseudonym::alias(insert_schema_trees)]
    pub async fn insert_schemas<T: ToTDBSchemas>(
        &self,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<()> {
        self.ensure_database(&args.spec.db)
            .await
            .context("ensuring database")?;

        let schemas = T::to_schemas();

        let count = schemas.len();

        debug!("inserting {} schemas", count);

        self.insert_schema_instances(schemas, args.as_schema())
            .await
            .context("insert_documents()")?;

        debug!("inserted {} schemas into TerminusDB", count);

        Ok(())
    }
}