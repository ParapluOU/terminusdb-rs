//! The DataFusion [`ContextProvider`] backed by our [`Catalog`].
//!
//! This is the bridge that lets `SqlToRel` resolve names and types against the
//! TerminusDB schema. We provide only a logical [`TableSource`] per table (via the
//! ready-made [`LogicalTableSource`]) — no physical `TableProvider`, no execution.
//! All function/variable hooks return `None`/empty in v1.

use std::sync::Arc;

use datafusion_common::arrow::datatypes::DataType;
use datafusion_common::config::ConfigOptions;
use datafusion_common::{DataFusionError, Result as DFResult, TableReference};
use datafusion_expr::logical_plan::builder::LogicalTableSource;
use datafusion_expr::planner::ContextProvider;
use datafusion_expr::{AggregateUDF, HigherOrderUDF, ScalarUDF, TableSource, WindowUDF};

use crate::catalog::Catalog;

/// A [`ContextProvider`] borrowing a [`Catalog`] for the duration of one planning
/// call.
pub struct TdbContextProvider<'a> {
    catalog: &'a Catalog,
}

impl<'a> TdbContextProvider<'a> {
    pub(crate) fn new(catalog: &'a Catalog) -> Self {
        Self { catalog }
    }
}

impl ContextProvider for TdbContextProvider<'_> {
    fn get_table_source(&self, name: TableReference) -> DFResult<Arc<dyn TableSource>> {
        // DataFusion normalises unquoted identifiers to lowercase, matching our
        // mangling, so `name.table()` is the catalog key.
        let key = name.table();
        if let Some(table) = self.catalog.table(key) {
            return Ok(Arc::new(LogicalTableSource::new(table.arrow.clone())));
        }
        // A class that exists in the schema but is not a v1 table: surface the
        // recorded reason rather than a bare "not found".
        if let Some(reason) = self.catalog.omitted_table_reason(key) {
            return Err(DataFusionError::Plan(format!(
                "table `{key}` exists in the schema but is unsupported ({reason})"
            )));
        }
        Err(DataFusionError::Plan(format!("table `{key}` not found")))
    }

    fn get_function_meta(&self, _name: &str) -> Option<Arc<ScalarUDF>> {
        None
    }

    fn get_higher_order_meta(&self, _name: &str) -> Option<Arc<HigherOrderUDF>> {
        None
    }

    fn get_aggregate_meta(&self, _name: &str) -> Option<Arc<AggregateUDF>> {
        None
    }

    fn get_window_meta(&self, _name: &str) -> Option<Arc<WindowUDF>> {
        None
    }

    fn get_variable_type(&self, _variable_names: &[String]) -> Option<DataType> {
        None
    }

    fn options(&self) -> &ConfigOptions {
        self.catalog.options()
    }

    fn udf_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn higher_order_function_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn udaf_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn udwf_names(&self) -> Vec<String> {
        Vec::new()
    }
}
