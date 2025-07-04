use crate::{BranchSpec, TerminusDBHttpClient};
use terminusdb_schema::{FromTDBInstance, InstanceFromJson, TerminusDBModel, ToTDBSchema};
use terminusdb_woql2::prelude::Query;
use terminusdb_woql_builder::builder::WoqlBuilder;
use terminusdb_woql_builder::prelude::{node, Var};
use terminusdb_woql_builder::vars;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use tap::Pipe;

/// contract of a self-contained query
pub trait InstanceQueryable {
    /// the main user model it is returning;
    /// this can also be an ad-hoc deserializable type and is not neccessarily a TerminusDB model
    type Model: TerminusDBModel + InstanceFromJson;

    /// running the query with an adapter
    async fn apply(
        &self,
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<Self::Model>> {
        let query = self.query(limit, offset);

        let v_id = vars!("Subject");
        let v_doc = vars!("doc");

        let res = client
            .query::<HashMap<String, serde_json::Value>>(spec.clone().into(), query)
            .await?;

        res.bindings
            .into_iter()
            .filter_map(|mut b| {
                b.remove(&*v_doc)
                    .map(|v| <Self::Model as FromTDBInstance>::from_json(v))
            })
            .collect()
    }

    /// returning the query as a builder
    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder;

    /// returning the query as a WOQL Query enum
    fn query(&self, limit: Option<usize>, offset: Option<usize>) -> Query {
        let v_id = vars!("Subject");
        let v_doc = vars!("doc");

        WoqlBuilder::new()
            // the triple was neccessary instead of the IsA
            .triple(
                v_id.clone(),
                "rdf:type",
                node(format!(
                    "@schema:{}",
                    <Self::Model as ToTDBSchema>::schema_name()
                )),
            )
            // generate the implementation-specific query
            .pipe(|q| self.build(v_id.clone(), q))
            // important: this seems to HAVE to come after the triple
            .read_document(v_id.clone(), v_doc.clone())
            // .isa(v_id.clone(), node(T::schema_name())) // this didnt work
            .select(vec![v_doc.clone()])
            .pipe(|q| match offset {
                None => q,
                Some(o) => q.start(o as u64),
            })
            .pipe(|q| match limit {
                None => q,
                Some(l) => q.limit(l as u64),
            })
            .finalize()
    }
}

/// default model litsing query that uses the instance query but adds no extra conditions
pub struct ListModels<T> {
    _ty: PhantomData<T>,
}

impl<T> Default for ListModels<T> {
    fn default() -> Self {
        Self {
            _ty: Default::default(),
        }
    }
}

impl<T: TerminusDBModel + InstanceFromJson> InstanceQueryable for ListModels<T> {
    type Model = T;

    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder {
        builder
    }
}

pub trait CanQuery<Q: InstanceQueryable> {
    fn get_query(&self) -> anyhow::Result<Q>;
}

#[derive(Default)]
pub enum QueryType {
    /// whether the result is reading some model through .read_document()
    #[default]
    Instance,
    /// or whether the returned type is ad-hoc created by triples and selects
    Custom,
}
