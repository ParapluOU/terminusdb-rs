use crate::value::{IntoWoql2, Var, WoqlInput};
use terminusdb_schema::{GraphType, ToTDBSchema};
// Import Query from the prelude
use terminusdb_woql2::prelude::{
    // Import Triple Ops
    AddTriple as Woql2AddTriple,
    AddedTriple as Woql2AddedTriple,
    And as Woql2And,
    Concatenate as Woql2Concatenate,
    Count as Woql2Count,
    DeleteDocument as Woql2DeleteDocument,
    DeleteTriple as Woql2DeleteTriple,
    // Import Control Ops
    Distinct as Woql2Distinct,
    // Import Collection Ops
    Dot as Woql2Dot,
    Equals as Woql2Equals,
    // Import Eval
    Eval as Woql2Eval,
    // Import From and Into
    From as Woql2From,
    Greater as Woql2Greater,
    // Import Aggregation/Grouping Ops
    GroupBy as Woql2GroupBy,
    If as Woql2If,
    Immediately as Woql2Immediately,
    InsertDocument as Woql2InsertDocument,
    Into as Woql2Into,
    // Import schema/type query types
    IsA as Woql2IsA,
    Join as Woql2Join,
    Length as Woql2Length,
    Less as Woql2Less,
    Like as Woql2Like,
    Limit as Woql2Limit,
    Lower as Woql2Lower,
    Member as Woql2Member,
    Not as Woql2Not,
    // Import remaining Control Ops
    Once as Woql2Once,
    Or as Woql2Or,
    Order as Woql2Order,
    // Import Ordering Ops
    OrderBy as Woql2OrderBy,
    OrderTemplate as Woql2OrderTemplate,
    Pad as Woql2Pad,
    // Import Path Op
    Path as Woql2Path,
    Query as Woql2Query,
    // Import Document Operations
    ReadDocument as Woql2ReadDocument,
    Regexp as Woql2Regexp,
    Select as Woql2Select,
    Split as Woql2Split,
    Start as Woql2Start,
    Substring as Woql2Substring,
    Subsumption as Woql2Subsumption,
    Sum as Woql2Sum,
    // Import String Operations
    Trim as Woql2Trim,
    Triple as Woql2Triple,
    // Import Misc Ops
    TripleCount as Woql2TripleCount,
    True as Woql2True,
    TypeOf as Woql2TypeOf,
    Typecast as Woql2Typecast,
    UpdateDocument as Woql2UpdateDocument,
    Upper as Woql2Upper,
    // Import Using
    Using as Woql2Using,
    // Import WoqlOptional
    WoqlOptional,
};
// Import expression types and the finalizer trait
use crate::expression::{ArithmeticExpression, FinalizeWoqlExpr};
use crate::path::{FinalizeWoqlPath, PathPattern};
use crate::prelude::node;
// Import path types

/// Builds a WOQL query using a fluent interface.
#[derive(Debug, Default, Clone)]
pub struct WoqlBuilder {
    // The query being constructed. Implicit chaining usually results in `And`.
    query: Option<Woql2Query>,
}

impl WoqlBuilder {
    /// Creates a new, empty WoqlBuilder.
    pub fn new() -> Self {
        WoqlBuilder::default()
    }

    /// Adds a triple pattern to the query.
    /// If a query already exists, it combines the existing query and the new triple
    /// using an `And` operation.
    pub fn triple<S, P, O>(mut self, subject: S, predicate: P, object: O) -> Self
    where
        S: IntoWoql2,
        P: IntoWoql2,
        O: IntoWoql2,
    {
        let triple_query = Woql2Query::Triple(Woql2Triple {
            subject: subject.into_woql2_node_value(),
            predicate: predicate.into_woql2_node_value(),
            object: object.into_woql2_value(),
            graph: GraphType::Instance,
        });

        self.query = match self.query.take() {
            // take() replaces self.query with None
            None => Some(triple_query), // No existing query, just use the triple
            Some(existing_query) => {
                // If the existing query is already an And, append to it.
                // Otherwise, create a new And.
                // This flattens simple chaining: T1.T2.T3 -> And(T1, T2, T3)
                match existing_query {
                    Woql2Query::And(mut and_query) => {
                        and_query.and.push(triple_query);
                        Some(Woql2Query::And(and_query))
                    }
                    other => Some(Woql2Query::And(Woql2And {
                        and: vec![other, triple_query],
                    })),
                }
            }
        };
        self
    }

    /// Combines the current query with other queries using logical AND.
    /// Replaces the current builder's query with the combined AND query.
    pub fn and(self, others: impl IntoIterator<Item = WoqlBuilder>) -> Self {
        let mut all_queries = vec![self.finalize()]; // Finalize self first
        all_queries.extend(others.into_iter().map(|b| b.finalize()));
        WoqlBuilder {
            query: Some(Woql2Query::And(Woql2And { and: all_queries })),
        }
    }

    /// Combines the current query with other queries using logical OR.
    /// Replaces the current builder's query with the combined OR query.
    pub fn or(self, others: impl IntoIterator<Item = WoqlBuilder>) -> Self {
        let mut all_queries = vec![self.finalize()]; // Finalize self first
        all_queries.extend(others.into_iter().map(|b| b.finalize()));
        WoqlBuilder {
            query: Some(Woql2Query::Or(Woql2Or { or: all_queries })),
        }
    }

    /// Negates the current query.
    /// Replaces the current builder's query with the NOT query.
    pub fn not(self) -> Self {
        let final_query = self.finalize();
        // Cannot negate True directly in WOQL, maybe return empty/False builder?
        // For now, let's allow Not(True) although it's unusual.
        WoqlBuilder {
            query: Some(Woql2Query::Not(Woql2Not {
                query: Box::new(final_query),
            })),
        }
    }

    /// Limits the number of results returned by the query.
    pub fn limit(self, limit_value: u64) -> Self {
        let final_query = self.finalize(); // Get the currently built query
        WoqlBuilder {
            query: Some(Woql2Query::Limit(Woql2Limit {
                limit: limit_value,
                query: Box::new(final_query),
            })),
        }
    }

    /// Sets the starting offset for the results returned by the query.
    pub fn start(self, start_value: u64) -> Self {
        let final_query = self.finalize(); // Get the currently built query
        WoqlBuilder {
            query: Some(Woql2Query::Start(Woql2Start {
                start: start_value,
                query: Box::new(final_query),
            })),
        }
    }

    /// Selects specific variables to include in the query results.
    /// Takes an iterator of `Var` structs.
    pub fn select(self, variables: impl IntoIterator<Item = Var>) -> Self {
        let final_query = self.finalize(); // Get the currently built query
        let var_names: Vec<String> = variables
            .into_iter()
            .map(|v| v.name().to_string())
            .collect();
        WoqlBuilder {
            query: Some(Woql2Query::Select(Woql2Select {
                variables: var_names,
                query: Box::new(final_query),
            })),
        }
    }

    /// Adds an equality comparison (`left == right`).
    /// Operands must resolve to DataValue (Literal or Variable).
    pub fn eq<L, R>(self, left: L, right: R) -> Self
    where
        L: IntoWoql2, // Must be convertible to DataValue
        R: IntoWoql2, // Must be convertible to DataValue
    {
        let eq_query = Woql2Query::Equals(Woql2Equals {
            left: left.into_woql2_value(),
            right: right.into_woql2_value(),
        });
        self.add_query_component(eq_query)
    }

    /// Adds a less than comparison (`left < right`).
    /// Operands must resolve to DataValue (Literal or Variable).
    pub fn less<L, R>(self, left: L, right: R) -> Self
    where
        L: IntoWoql2,
        R: IntoWoql2,
    {
        let less_query = Woql2Query::Less(Woql2Less {
            left: left.into_woql2_data_value(),
            right: right.into_woql2_data_value(),
        });
        self.add_query_component(less_query)
    }

    /// Adds a greater than comparison (`left > right`).
    /// Operands must resolve to DataValue (Literal or Variable).
    pub fn greater<L, R>(self, left: L, right: R) -> Self
    where
        L: IntoWoql2,
        R: IntoWoql2,
    {
        let greater_query = Woql2Query::Greater(Woql2Greater {
            left: left.into_woql2_data_value(),
            right: right.into_woql2_data_value(),
        });
        self.add_query_component(greater_query)
    }

    /// Consumes the builder and returns the constructed WOQL query.
    /// If no operations were added, it returns a `True` query.
    pub fn finalize(self) -> Woql2Query {
        self.query.unwrap_or_else(|| Woql2Query::True(Woql2True {}))
    }

    fn add_query_component(mut self, component: Woql2Query) -> Self {
        self.query = match self.query.take() {
            None => Some(component),
            Some(existing_query) => match existing_query {
                Woql2Query::And(mut and_query) => {
                    and_query.and.push(component);
                    Some(Woql2Query::And(and_query))
                }
                other => Some(Woql2Query::And(Woql2And {
                    and: vec![other, component],
                })),
            },
        };
        self
    }

    /// Makes the current query optional. If it fails, the overall query can still succeed (without bindings from this part).
    pub fn opt(self) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::WoqlOptional(WoqlOptional {
                query: Box::new(final_query),
            })),
        }
    }

    /// Alias for `opt()`.
    pub fn optional(self) -> Self {
        self.opt()
    }

    /// Creates a conditional query.
    /// If the `test_builder` query succeeds, the `then_builder` query is executed.
    /// Otherwise, the `else_builder` query is executed.
    /// This *replaces* any query previously built in `self`.
    pub fn if_then_else(
        test_builder: WoqlBuilder,
        then_builder: WoqlBuilder,
        else_builder: WoqlBuilder,
    ) -> Self {
        let test_query = test_builder.finalize();
        let then_query = then_builder.finalize();
        let else_query = else_builder.finalize();

        WoqlBuilder {
            query: Some(Woql2Query::If(Woql2If {
                test: Box::new(test_query),
                then_query: Box::new(then_query),
                else_query: Box::new(else_query),
            })),
        }
    }

    /// Creates a conditional query with only a `then` branch (implicitly `True` for `else`).
    /// If the `test_builder` query succeeds, the `then_builder` query is executed.
    pub fn when(test_builder: WoqlBuilder, then_builder: WoqlBuilder) -> Self {
        Self::if_then_else(test_builder, then_builder, WoqlBuilder::new()) // Use empty builder -> True for else
    }

    /// Adds a check if `element` is of type `type_of`.
    /// Both arguments must resolve to NodeValue (IRI or Variable).
    pub fn isa<E, T>(self, element: E, type_of: T) -> Self
    where
        E: IntoWoql2, // Must be NodeValue
        T: IntoWoql2, // Must be NodeValue
    {
        let isa_query = Woql2Query::IsA(Woql2IsA {
            element: element.into_woql2_node_value(),
            type_of: type_of.into_woql2_node_value(),
        });
        self.add_query_component(isa_query)
    }

    /// custom wrapper around triple and rdf:type to determine whether
    /// a subject is of a specific type. We use this because IsA doesn't seem
    /// to always work
    pub fn isa2<T: ToTDBSchema>(self, subject: &(impl IntoWoql2 + Clone)) -> Self {
        self.triple(
            subject.clone(),
            node("rdf:type"),
            node(format!("@schema:{}", T::schema_name())),
        )
    }

    /// Adds a check if `child` class is subsumed by (is a subclass of) `parent` class.
    /// Both arguments must resolve to NodeValue (IRI or Variable).
    pub fn subsumption<C, P>(self, child: C, parent: P) -> Self
    where
        C: IntoWoql2, // Must be NodeValue
        P: IntoWoql2, // Must be NodeValue
    {
        let sub_query = Woql2Query::Subsumption(Woql2Subsumption {
            child: child.into_woql2_node_value(),
            parent: parent.into_woql2_node_value(),
        });
        self.add_query_component(sub_query)
    }

    /// Adds a check for the type of `value`, binding the result to `type_uri`.
    /// `value` can be any Value, `type_uri` must be NodeValue (usually a Variable).
    pub fn type_of<V, T>(self, value: V, type_uri: T) -> Self
    where
        V: IntoWoql2, // Can be any Value
        T: IntoWoql2, // Must be NodeValue (typically Var)
    {
        let type_of_query = Woql2Query::TypeOf(Woql2TypeOf {
            value: value.into_woql2_value(),
            type_uri: type_uri.into_woql2_node_value(),
        });
        self.add_query_component(type_of_query)
    }

    /// Adds a type casting operation.
    /// `value` to cast (any Value), `type_uri` to cast to (NodeValue),
    /// `result_value` to bind the result (any Value, typically Var).
    pub fn typecast<V, T, R>(self, value: V, type_uri: T, result_value: R) -> Self
    where
        V: IntoWoql2, // Value to cast
        T: IntoWoql2, // Type URI (NodeValue)
        R: IntoWoql2, // Result variable/value
    {
        let cast_query = Woql2Query::Typecast(Woql2Typecast {
            value: value.into_woql2_value(),
            type_uri: type_uri.into_woql2_node_value(),
            result_value: result_value.into_woql2_value(),
        });
        self.add_query_component(cast_query)
    }

    /// Specifies the database/repository context for the query.
    /// Wraps the current query.
    pub fn using(self, collection_uri: impl Into<String>) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::Using(Woql2Using {
                collection: collection_uri.into(),
                query: Box::new(final_query),
            })),
        }
    }

    /// Specifies the *read* graph for the query (e.g., "schema/main", "instance/main").
    /// Wraps the current query.
    pub fn from(self, graph_uri: impl Into<String>) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::From(Woql2From {
                graph: graph_uri.into(),
                query: Box::new(final_query),
            })),
        }
    }

    /// Specifies the *write* graph for the query (e.g., "schema/main", "instance/main").
    /// Wraps the current query.
    pub fn into(self, graph_uri: impl Into<String>) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::Into(Woql2Into {
                graph: graph_uri.into(),
                query: Box::new(final_query),
            })),
        }
    }

    // --- String Operations ---

    /// Trims whitespace from `untrimmed`, binding the result to `trimmed`.
    pub fn trim<U, T>(self, untrimmed: U, trimmed: T) -> Self
    where
        U: IntoWoql2,
        T: IntoWoql2,
    {
        let trim_query = Woql2Query::Trim(Woql2Trim {
            untrimmed: untrimmed.into_woql2_data_value(),
            trimmed: trimmed.into_woql2_data_value(),
        });
        self.add_query_component(trim_query)
    }

    /// Converts `mixed_case` string to lowercase, binding the result to `lower_case`.
    pub fn lower<M, L>(self, mixed_case: M, lower_case: L) -> Self
    where
        M: IntoWoql2,
        L: IntoWoql2,
    {
        let lower_query = Woql2Query::Lower(Woql2Lower {
            mixed: mixed_case.into_woql2_data_value(),
            lower: lower_case.into_woql2_data_value(),
        });
        self.add_query_component(lower_query)
    }

    /// Converts `mixed_case` string to uppercase, binding the result to `upper_case`.
    pub fn upper<M, U>(self, mixed_case: M, upper_case: U) -> Self
    where
        M: IntoWoql2,
        U: IntoWoql2,
    {
        let upper_query = Woql2Query::Upper(Woql2Upper {
            mixed: mixed_case.into_woql2_data_value(),
            upper: upper_case.into_woql2_data_value(),
        });
        self.add_query_component(upper_query)
    }

    /// Pads `input_string` with `pad_char` `times` times, binding the result to `result_string`.
    pub fn pad<S, C, T, R>(self, input_string: S, pad_char: C, times: T, result_string: R) -> Self
    where
        S: IntoWoql2,
        C: IntoWoql2,
        T: IntoWoql2, // Should resolve to Integer DataValue
        R: IntoWoql2,
    {
        let pad_query = Woql2Query::Pad(Woql2Pad {
            string: input_string.into_woql2_data_value(),
            char: pad_char.into_woql2_data_value(),
            times: times.into_woql2_data_value(),
            result_string: result_string.into_woql2_data_value(),
        });
        self.add_query_component(pad_query)
    }

    /// Splits `input_string` by `pattern`, binding the resulting list to `result_list`.
    pub fn split<S, P, L>(self, input_string: S, pattern: P, result_list: L) -> Self
    where
        S: IntoWoql2,
        P: IntoWoql2,
        L: IntoWoql2, // Should resolve to List DataValue (usually a Var)
    {
        let split_query = Woql2Query::Split(Woql2Split {
            string: input_string.into_woql2_data_value(),
            pattern: pattern.into_woql2_data_value(),
            list: result_list.into_woql2_data_value(),
        });
        self.add_query_component(split_query)
    }

    /// Joins elements of `input_list` with `separator`, binding the result to `result_string`.
    pub fn join<L, S, R>(self, input_list: L, separator: S, result_string: R) -> Self
    where
        L: IntoWoql2, // Should resolve to List DataValue
        S: IntoWoql2,
        R: IntoWoql2,
    {
        let join_query = Woql2Query::Join(Woql2Join {
            list: input_list.into_woql2_data_value(),
            separator: separator.into_woql2_data_value(),
            result_string: result_string.into_woql2_data_value(),
        });
        self.add_query_component(join_query)
    }

    /// Concatenates elements of `input_list`, binding the result to `result_string`.
    /// Alias: `concat`.
    pub fn concatenate<L, R>(self, input_list: L, result_string: R) -> Self
    where
        L: IntoWoql2, // Should resolve to List DataValue
        R: IntoWoql2,
    {
        let concat_query = Woql2Query::Concatenate(Woql2Concatenate {
            list: input_list.into_woql2_data_value(),
            result_string: result_string.into_woql2_data_value(),
        });
        self.add_query_component(concat_query)
    }

    /// Alias for `concatenate`.
    pub fn concat<L, R>(self, input_list: L, result_string: R) -> Self
    where
        L: IntoWoql2,
        R: IntoWoql2,
    {
        self.concatenate(input_list, result_string)
    }

    /// Extracts a `substring` from `input_string`.
    /// `before`, `length`, `after` define the substring boundaries.
    pub fn substring<S, B, L, A, Sub>(
        self,
        input_string: S,
        before: B,
        length: L,
        after: A,
        substring: Sub,
    ) -> Self
    where
        S: IntoWoql2,
        B: IntoWoql2, // Should resolve to Integer DataValue
        L: IntoWoql2, // Should resolve to Integer DataValue
        A: IntoWoql2, // Should resolve to Integer DataValue
        Sub: IntoWoql2,
    {
        let sub_query = Woql2Query::Substring(Woql2Substring {
            string: input_string.into_woql2_data_value(),
            before: before.into_woql2_data_value(),
            length: length.into_woql2_data_value(),
            after: after.into_woql2_data_value(),
            substring: substring.into_woql2_data_value(),
        });
        self.add_query_component(sub_query)
    }

    /// Matches `input_string` against `pattern` (regex). Binds results to `result_list` (optional).
    /// Note: WOQL uses PCRE style regex.
    pub fn regexp<P, S, R>(self, pattern: P, input_string: S, result_list: Option<R>) -> Self
    where
        P: IntoWoql2,
        S: IntoWoql2,
        R: IntoWoql2, // Should resolve to List DataValue
    {
        let regexp_query = Woql2Query::Regexp(Woql2Regexp {
            pattern: pattern.into_woql2_data_value(),
            string: input_string.into_woql2_data_value(),
            result: result_list.map(|r| r.into_woql2_data_value()),
        });
        self.add_query_component(regexp_query)
    }

    /// Calculates the similarity between `left` and `right` strings, binding the score to `similarity`.
    /// The similarity score is a number between -1 and 1.
    pub fn like<L, R, Sim>(self, left: L, right: R, similarity: Sim) -> Self
    where
        L: IntoWoql2,
        R: IntoWoql2,
        Sim: IntoWoql2, // Should resolve to Float DataValue
    {
        let like_query = Woql2Query::Like(Woql2Like {
            left: left.into_woql2_data_value(),
            right: right.into_woql2_data_value(),
            similarity: similarity.into_woql2_data_value(),
        });
        self.add_query_component(like_query)
    }

    // --- Collection Operations ---

    /// Checks if `element` is a member of `list`.
    /// Both arguments must resolve to DataValue (Literal or Variable).
    pub fn member<E, L>(self, element: E, list: L) -> Self
    where
        E: IntoWoql2,
        L: IntoWoql2,
    {
        let member_query = Woql2Query::Member(Woql2Member {
            member: element.into_woql2_data_value(),
            list: list.into_woql2_data_value(),
        });
        self.add_query_component(member_query)
    }

    /// Accesses the value associated with `field` within the `document`,
    /// binding the result to `value`.
    /// All arguments must resolve to DataValue (Literal or Variable).
    pub fn dot<D, F, V>(self, document: D, field: F, value: V) -> Self
    where
        D: IntoWoql2,
        F: IntoWoql2,
        V: IntoWoql2,
    {
        let dot_query = Woql2Query::Dot(Woql2Dot {
            document: document.into_woql2_data_value(),
            field: field.into_woql2_data_value(),
            value: value.into_woql2_data_value(),
        });
        self.add_query_component(dot_query)
    }

    // --- Document Operations ---

    /// Reads a document specified by `identifier` and binds it to `document_var`.
    pub fn read_document<Id, DocVar>(self, identifier: Id, document_var: DocVar) -> Self
    where
        Id: IntoWoql2,     // Should resolve to NodeValue (IRI or Var)
        DocVar: IntoWoql2, // Should resolve to Value (usually Var)
    {
        let read_doc_query = Woql2Query::ReadDocument(Woql2ReadDocument {
            identifier: identifier.into_woql2_node_value(),
            document: document_var.into_woql2_value(),
        });
        self.add_query_component(read_doc_query)
    }

    /// Inserts `document_value` into the database.
    /// Optionally binds the IRI of the inserted document to `new_identifier_var`.
    pub fn insert_document<DocVal, NewIdVar>(
        self,
        document_value: DocVal,
        new_identifier_var: Option<NewIdVar>,
    ) -> Self
    where
        DocVal: IntoWoql2,   // Should resolve to Value (JSON literal or Var)
        NewIdVar: IntoWoql2, // Should resolve to NodeValue (Var)
    {
        let insert_doc_query = Woql2Query::InsertDocument(Woql2InsertDocument {
            document: document_value.into_woql2_value(),
            identifier: new_identifier_var.map(|v| v.into_woql2_node_value()),
        });
        self.add_query_component(insert_doc_query)
    }

    /// Updates the document identified within `document_value`.
    /// `document_value` must contain an `@id` or have a key defined by its `@type`.
    /// Optionally binds the IRI of the updated document to `updated_identifier_var`.
    pub fn update_document<DocVal, UpdatedIdVar>(
        self,
        document_value: DocVal,
        updated_identifier_var: Option<UpdatedIdVar>,
    ) -> Self
    where
        DocVal: IntoWoql2,       // Should resolve to Value (JSON literal or Var)
        UpdatedIdVar: IntoWoql2, // Should resolve to NodeValue (Var)
    {
        let update_doc_query = Woql2Query::UpdateDocument(Woql2UpdateDocument {
            document: document_value.into_woql2_value(),
            identifier: updated_identifier_var.map(|v| v.into_woql2_node_value()),
        });
        self.add_query_component(update_doc_query)
    }

    /// Deletes the document specified by `identifier`.
    pub fn delete_document<Id>(self, identifier: Id) -> Self
    where
        Id: IntoWoql2, // Should resolve to NodeValue (IRI or Var)
    {
        let delete_doc_query = Woql2Query::DeleteDocument(Woql2DeleteDocument {
            identifier: identifier.into_woql2_node_value(),
        });
        self.add_query_component(delete_doc_query)
    }

    // --- Triple & Data Manipulation ---

    /// Adds a triple to be inserted into the database.
    /// Chains with previous operations using `And`.
    pub fn add_triple<S, P, O>(self, subject: S, predicate: P, object: O) -> Self
    where
        S: IntoWoql2,
        P: IntoWoql2,
        O: IntoWoql2,
    {
        let add_triple_query = Woql2Query::AddTriple(Woql2AddTriple {
            subject: subject.into_woql2_node_value(),
            predicate: predicate.into_woql2_node_value(),
            object: object.into_woql2_value(),
            graph: GraphType::Instance, // Add graph support later if needed
        });
        self.add_query_component(add_triple_query)
    }

    /// Adds a triple pattern to be deleted from the database.
    /// Chains with previous operations using `And`.
    pub fn delete_triple<S, P, O>(self, subject: S, predicate: P, object: O) -> Self
    where
        S: IntoWoql2,
        P: IntoWoql2,
        O: IntoWoql2,
    {
        let delete_triple_query = Woql2Query::DeleteTriple(Woql2DeleteTriple {
            subject: subject.into_woql2_node_value(),
            predicate: predicate.into_woql2_node_value(),
            object: object.into_woql2_value(),
            graph: GraphType::Instance, // Add graph support later if needed
        });
        self.add_query_component(delete_triple_query)
    }

    /// Adds a pattern matching triples that were *added* in the current commit context.
    /// Typically used within a `using("commit/...")` context.
    /// Optionally specifies a graph.
    pub fn added_triple<S, P, O>(
        self,
        subject: S,
        predicate: P,
        object: O,
        graph: Option<impl Into<GraphType>>,
    ) -> Self
    where
        S: IntoWoql2,
        P: IntoWoql2,
        O: IntoWoql2,
    {
        let added_triple_query = Woql2Query::AddedTriple(Woql2AddedTriple {
            subject: subject.into_woql2_node_value(),
            predicate: predicate.into_woql2_node_value(),
            object: object.into_woql2_value(),
            graph: graph.map(|g| g.into()).unwrap_or_default(),
        });
        self.add_query_component(added_triple_query)
    }

    // --- Mathematical Operations ---

    /// Evaluates an `ArithmeticExpression` and binds the result to `result_value`.
    pub fn eval<Expr, Res>(self, expression: Expr, result_value: Res) -> Self
    where
        Expr: Into<ArithmeticExpression>, // Expression can be built from various types
        Res: Into<ArithmeticExpression>,  // Result must also be convertible (Var or Literal)
    {
        let final_expr = expression.into(); // Convert to builder's ArithmeticExpression
        let final_res = result_value.into();

        let eval_query = Woql2Query::Eval(Woql2Eval {
            expression: final_expr.finalize_expr(), // Finalize expression to woql2 type
            result_value: final_res.finalize_val(), // Finalize result to woql2 type
        });
        self.add_query_component(eval_query)
    }

    // --- Aggregation & Grouping ---

    /// Groups results of `subquery` by the `group_vars`.
    /// The `template` defines the structure of the output for each group.
    /// Binds the resulting list of grouped templates to `grouped_result_var`.
    pub fn group_by<Template, GroupVar, ResultVar>(
        self,
        template: Template,
        group_vars: impl IntoIterator<Item = GroupVar>,
        grouped_result_var: ResultVar,
    ) -> Self
    where
        Template: IntoWoql2,  // Usually a JSON-LD template as Value::Dictionary or Var
        GroupVar: Into<Var>,  // Variable names to group by
        ResultVar: IntoWoql2, // Variable to bind the resulting list
    {
        let final_subquery = self.finalize(); // The query to group
        let group_var_names: Vec<String> = group_vars
            .into_iter()
            .map(|v| v.into().name().to_string())
            .collect();

        let group_by_query = Woql2Query::GroupBy(Woql2GroupBy {
            template: template.into_woql2_value(),
            group_by: group_var_names,
            grouped_value: grouped_result_var.into_woql2_value(),
            query: Box::new(final_subquery),
        });

        // GroupBy typically replaces the current query context
        WoqlBuilder {
            query: Some(group_by_query),
        }
    }

    /// Counts the results of the current query and binds it to `count_var`.
    pub fn count<CountVar>(self, count_var: CountVar) -> Self
    where
        CountVar: IntoWoql2, // Variable to bind the count to (DataValue)
    {
        let final_subquery = self.finalize(); // The query to count
        let count_query = Woql2Query::Count(Woql2Count {
            query: Box::new(final_subquery),
            count: count_var.into_woql2_data_value(),
        });
        // Count typically replaces the current query context
        WoqlBuilder {
            query: Some(count_query),
        }
    }

    /// Calculates the sum of `input_list_var` and binds it to `result_var`.
    /// This is a standalone operation, doesn't chain with `And`.
    pub fn sum<ListVar, ResultVar>(input_list_var: ListVar, result_var: ResultVar) -> Self
    where
        ListVar: IntoWoql2,   // Variable bound to a list of numbers
        ResultVar: IntoWoql2, // Variable to bind the sum
    {
        let sum_query = Woql2Query::Sum(Woql2Sum {
            list: input_list_var.into_woql2_data_value(),
            result: result_var.into_woql2_data_value(),
        });
        // Standalone operation
        WoqlBuilder {
            query: Some(sum_query),
        }
    }

    /// Calculates the length of `input_list_var` and binds it to `length_var`.
    /// This is a standalone operation, doesn't chain with `And`.
    pub fn length<ListVar, LengthVar>(input_list_var: ListVar, length_var: LengthVar) -> Self
    where
        ListVar: IntoWoql2,   // Variable bound to a list
        LengthVar: IntoWoql2, // Variable to bind the length
    {
        let length_query = Woql2Query::Length(Woql2Length {
            list: input_list_var.into_woql2_data_value(),
            length: length_var.into_woql2_data_value(),
        });
        // Standalone operation
        WoqlBuilder {
            query: Some(length_query),
        }
    }

    // --- Ordering Results ---

    /// Orders the results of the current query.
    /// Takes an iterator of tuples `(Var, Order)`, where `Order` is `woql2::order::Order` (`Asc` or `Desc`).
    pub fn order_by<OrdVar>(self, ordering: impl IntoIterator<Item = (OrdVar, Woql2Order)>) -> Self
    where
        OrdVar: Into<Var>,
    {
        let final_subquery = self.finalize(); // The query to order

        let order_templates: Vec<Woql2OrderTemplate> = ordering
            .into_iter()
            .map(|(var, order)| Woql2OrderTemplate {
                variable: var.into().name().to_string(),
                order,
            })
            .collect();

        let order_by_query = Woql2Query::OrderBy(Woql2OrderBy {
            ordering: order_templates,
            query: Box::new(final_subquery),
        });

        // OrderBy replaces the current query context
        WoqlBuilder {
            query: Some(order_by_query),
        }
    }

    // --- Path Queries ---

    /// Creates a path query.
    /// Finds paths from `subject` to `object` following the `pattern`.
    /// Optionally binds the path itself to `path_var`.
    /// This replaces the current query in the builder.
    pub fn path<S, O, PVar>(
        subject: S,
        pattern: PathPattern,
        object: O,
        path_var: Option<PVar>,
    ) -> Self
    where
        S: IntoWoql2,
        O: IntoWoql2,
        PVar: IntoWoql2, // Variable to bind the path list
    {
        let path_query = Woql2Query::Path(Woql2Path {
            subject: subject.into_woql2_value(),
            pattern: pattern.finalize_path(),
            object: object.into_woql2_value(),
            path: path_var.map(|v| v.into_woql2_value()),
        });
        // Standalone operation
        WoqlBuilder {
            query: Some(path_query),
        }
    }

    // --- Control Flow ---

    /// Wraps the current query in `Once`, ensuring it returns at most one solution.
    pub fn once(self) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::Once(Woql2Once {
                query: Box::new(final_query),
            })),
        }
    }

    /// Wraps the current query in `Immediately`, attempting to perform side-effects eagerly.
    /// Use with caution.
    pub fn immediately(self) -> Self {
        let final_query = self.finalize();
        WoqlBuilder {
            query: Some(Woql2Query::Immediately(Woql2Immediately {
                query: Box::new(final_query),
            })),
        }
    }

    // --- Miscellaneous Operations (Potentially, or Control Flow?) ---

    /// Ensures that the results for the specified variables are unique.
    /// Takes an iterator of variables (`Var`).
    pub fn distinct(self, variables: impl IntoIterator<Item = Var>) -> Self {
        let final_query = self.finalize(); // Get the currently built query
        let var_names: Vec<String> = variables
            .into_iter()
            .map(|v| v.name().to_string())
            .collect();
        WoqlBuilder {
            query: Some(Woql2Query::Distinct(Woql2Distinct {
                variables: var_names,
                query: Box::new(final_query),
            })),
        }
    }

    /// Returns the count of triples in the specified resource (graph IRI).
    /// This is a standalone query operation.
    pub fn triple_count(resource: impl Into<String>, count_var: impl IntoWoql2) -> Self {
        let triple_count_query = Woql2Query::TripleCount(Woql2TripleCount {
            resource: resource.into(),
            count: count_var.into_woql2_data_value(),
        });
        WoqlBuilder {
            query: Some(triple_count_query),
        }
    }
}
