//! Path builder for constructing WOQL queries with type-safe state machine
//!
//! This module provides a builder pattern for constructing complex path traversals
//! that compile down to efficient WOQL queries. The builder uses a state machine
//! approach to ensure type safety and enable unlimited chain length.
//!
//! # Example Usage
//! ```rust
//! // Simple forward chain
//! let query = PathStart::new().node::<User>().forward().node::<Post>().finalize();
//!
//! // Complex chain with mixed directions
//! let query = PathStart::new()
//!     .node::<User>()
//!     .forward()
//!     .node::<Post>()
//!     .backward()
//!     .node::<Comment>()
//!     .finalize();
//! ```

use crate::prelude::Triple;
use crate::query::Query;
use std::marker::PhantomData;

/// Starting point for path construction
#[derive(Debug)]
pub struct PathStart {
    variable_counter: u32,
}

/// Half-path state: has a node type but no direction specified
#[derive(Debug)]
pub struct HalfPath<T> {
    node_type: PhantomData<T>,
    current_var: String,
    accumulated_queries: Vec<Query>,
    variable_counter: u32,
    custom_variable: Option<String>,
}

/// Half-path with direction specified, waiting for target node
#[derive(Debug)]
pub struct HalfPathDirection<T> {
    source_type: PhantomData<T>,
    source_var: String,
    direction: PathDirection,
    field_name: Option<String>,
    accumulated_queries: Vec<Query>,
    variable_counter: u32,
}

/// Complete path segment from one node to another
#[derive(Debug)]
pub struct FullPath<From, To> {
    from_type: PhantomData<From>,
    to_type: PhantomData<To>,
    from_var: String,
    to_var: String,
    accumulated_queries: Vec<Query>,
    variable_counter: u32,
}

/// Direction of path traversal
#[derive(Debug, Clone, PartialEq)]
pub enum PathDirection {
    Forward,
    Backward,
}

impl PathStart {
    /// Create a new path builder
    pub fn new() -> Self {
        Self {
            variable_counter: 0,
        }
    }

    /// Start a path with the specified node type
    pub fn node<T>(mut self) -> HalfPath<T> {
        self.variable_counter += 1;
        let var_name = format!(
            "{}_1",
            std::any::type_name::<T>()
                .split("::")
                .last()
                .unwrap_or("Node")
        );

        HalfPath {
            node_type: PhantomData,
            current_var: var_name,
            accumulated_queries: Vec::new(),
            variable_counter: self.variable_counter,
            custom_variable: None,
        }
    }

    /// Start a path with a custom variable name
    pub fn variable<T>(mut self, name: &str) -> HalfPath<T> {
        self.variable_counter += 1;

        HalfPath {
            node_type: PhantomData,
            current_var: name.to_string(),
            accumulated_queries: Vec::new(),
            variable_counter: self.variable_counter,
            custom_variable: Some(name.to_string()),
        }
    }
}

impl<T> HalfPath<T> {
    /// Set a custom variable name for this node
    pub fn variable(mut self, name: &str) -> Self {
        self.custom_variable = Some(name.to_string());
        self.current_var = name.to_string();
        self
    }

    /// Specify forward direction (=>)
    pub fn forward(self) -> HalfPathDirection<T> {
        HalfPathDirection {
            source_type: self.node_type,
            source_var: self.current_var,
            direction: PathDirection::Forward,
            field_name: None,
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Specify backward direction (<=)
    pub fn backward(self) -> HalfPathDirection<T> {
        HalfPathDirection {
            source_type: self.node_type,
            source_var: self.current_var,
            direction: PathDirection::Backward,
            field_name: None,
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Specify explicit field name
    pub fn field(self, field: &str) -> HalfPathDirection<T> {
        HalfPathDirection {
            source_type: self.node_type,
            source_var: self.current_var,
            direction: PathDirection::Forward, // Fields are always forward
            field_name: Some(field.to_string()),
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Finalize a single node (terminal case)
    pub fn finalize(mut self) -> Query {
        // Generate type constraint for single node
        let type_constraint = Query::Triple(Triple {
            subject: crate::macros::into_node_value(self.current_var.clone()),
            predicate: crate::macros::into_node_value("rdf:type"),
            object: crate::macros::into_value(format!(
                "@schema:{}",
                std::any::type_name::<T>()
                    .split("::")
                    .last()
                    .unwrap_or("Node")
            )),
            graph: Some(terminusdb_schema::GraphType::Instance),
        });

        self.accumulated_queries.push(type_constraint);

        if self.accumulated_queries.len() == 1 {
            self.accumulated_queries.into_iter().next().unwrap()
        } else {
            Query::And(crate::query::And {
                and: self.accumulated_queries,
            })
        }
    }
}

impl<T> HalfPathDirection<T> {
    /// Add target node to complete the path segment
    pub fn node<U>(mut self) -> FullPath<T, U> {
        self.variable_counter += 1;
        let target_var = format!(
            "{}_1",
            std::any::type_name::<U>()
                .split("::")
                .last()
                .unwrap_or("Node")
        );
        self._build_path::<U>(target_var)
    }

    /// Add target node with custom variable name
    pub fn variable<U>(mut self, name: &str) -> FullPath<T, U> {
        self.variable_counter += 1;
        self._build_path::<U>(name.to_string())
    }

    /// Internal method to build the path with the specified target variable
    fn _build_path<U>(mut self, target_var: String) -> FullPath<T, U> {
        // Generate the relationship triple based on direction and field
        let relation_triple = match self.direction {
            PathDirection::Forward => {
                let pred = self.field_name.unwrap_or_else(|| {
                    // Auto-generate field name (simple pluralization)
                    format!(
                        "{}s",
                        std::any::type_name::<U>()
                            .split("::")
                            .last()
                            .unwrap_or("Node")
                    )
                });
                Query::Triple(Triple {
                    subject: crate::macros::into_node_value(self.source_var.clone()),
                    predicate: crate::macros::into_node_value(pred),
                    object: crate::macros::into_value(target_var.clone()),
                    graph: Some(terminusdb_schema::GraphType::Instance),
                })
            }
            PathDirection::Backward => {
                let pred = self.field_name.unwrap_or_else(|| {
                    // For backward direction, use target type for field name
                    // A <= B means A.Bs <- B, so use B's type
                    format!(
                        "{}s",
                        std::any::type_name::<U>()
                            .split("::")
                            .last()
                            .unwrap_or("Node")
                    )
                });
                Query::Triple(Triple {
                    subject: crate::macros::into_node_value(self.source_var.clone()),
                    predicate: crate::macros::into_node_value(pred),
                    object: crate::macros::into_value(target_var.clone()),
                    graph: Some(terminusdb_schema::GraphType::Instance),
                })
            }
        };

        // Add type constraints for both nodes
        let source_type = Query::Triple(Triple {
            subject: crate::macros::into_node_value(self.source_var.clone()),
            predicate: crate::macros::into_node_value("rdf:type"),
            object: crate::macros::into_value(format!(
                "@schema:{}",
                std::any::type_name::<T>()
                    .split("::")
                    .last()
                    .unwrap_or("Node")
            )),
            graph: Some(terminusdb_schema::GraphType::Instance),
        });

        let target_type = Query::Triple(Triple {
            subject: crate::macros::into_node_value(target_var.clone()),
            predicate: crate::macros::into_node_value("rdf:type"),
            object: crate::macros::into_value(format!(
                "@schema:{}",
                std::any::type_name::<U>()
                    .split("::")
                    .last()
                    .unwrap_or("Node")
            )),
            graph: Some(terminusdb_schema::GraphType::Instance),
        });

        let mut queries = self.accumulated_queries;
        queries.push(relation_triple);
        queries.push(source_type);
        queries.push(target_type);

        FullPath {
            from_type: PhantomData,
            to_type: PhantomData,
            from_var: self.source_var,
            to_var: target_var,
            accumulated_queries: queries,
            variable_counter: self.variable_counter,
        }
    }
}

impl<From, To> FullPath<From, To> {
    /// Finalize the path and return the WOQL query
    pub fn finalize(self) -> Query {
        if self.accumulated_queries.len() == 1 {
            self.accumulated_queries.into_iter().next().unwrap()
        } else {
            Query::And(crate::query::And {
                and: self.accumulated_queries,
            })
        }
    }

    /// Continue the path with forward direction
    pub fn forward(self) -> HalfPathDirection<To> {
        HalfPathDirection {
            source_type: self.to_type,
            source_var: self.to_var,
            direction: PathDirection::Forward,
            field_name: None,
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Continue the path with backward direction
    pub fn backward(self) -> HalfPathDirection<To> {
        HalfPathDirection {
            source_type: self.to_type,
            source_var: self.to_var,
            direction: PathDirection::Backward,
            field_name: None,
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Continue the path with explicit field
    pub fn field(self, field: &str) -> HalfPathDirection<To> {
        HalfPathDirection {
            source_type: self.to_type,
            source_var: self.to_var,
            direction: PathDirection::Forward,
            field_name: Some(field.to_string()),
            accumulated_queries: self.accumulated_queries,
            variable_counter: self.variable_counter,
        }
    }

    /// Access the final variable name (useful for chaining with other queries)
    pub fn final_variable(&self) -> &str {
        &self.to_var
    }

    /// Access the source variable name
    pub fn source_variable(&self) -> &str {
        &self.from_var
    }
}

impl Default for PathStart {
    fn default() -> Self {
        Self::new()
    }
}
