//! Macros for simplified WOQL query construction
//!
//! These macros provide a more ergonomic way to construct WOQL queries
//! as an alternative to the woql-builder approach.

/// Create a Variable value
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let x = var!(x); // Creates Value::Variable("x".to_string())
/// let person = var!("Person"); // Creates Value::Variable("Person".to_string())
/// ```
#[macro_export]
macro_rules! var {
    ($name:ident) => {
        $crate::value::Value::Variable(stringify!($name).to_string())
    };
    ($name:expr) => {
        $crate::value::Value::Variable($name.to_string())
    };
}

/// Create a NodeValue Variable
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let x = node_var!(x); // Creates NodeValue::Variable("x".to_string())
/// ```
#[macro_export]
macro_rules! node_var {
    ($name:ident) => {
        $crate::value::NodeValue::Variable(stringify!($name).to_string())
    };
    ($name:expr) => {
        $crate::value::NodeValue::Variable($name.to_string())
    };
}

/// Create a Node value
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let person = node!("Person"); // Creates Value::Node("Person".to_string())
/// let rdf_type = node!("rdf:type"); // Creates Value::Node("rdf:type".to_string())
/// ```
#[macro_export]
macro_rules! node {
    ($uri:expr) => {
        $crate::value::Value::Node($uri.to_string())
    };
}

/// Create a NodeValue Node
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let person = node_value!("Person"); // Creates NodeValue::Node("Person".to_string())
/// ```
#[macro_export]
macro_rules! node_value {
    ($uri:expr) => {
        $crate::value::NodeValue::Node($uri.to_string())
    };
}

/// Create a schema type name URI
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let person_type = typename!(Person); // Creates "@schema:Person"
/// let custom_type = typename!("CustomModel"); // Creates "@schema:CustomModel"
/// ```
#[macro_export]
macro_rules! typename {
    ($type:ident) => {
        format!("@schema:{}", stringify!($type))
    };
    ($type:expr) => {
        format!("@schema:{}", $type)
    };
}

/// Create a Data value with automatic type conversion
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let age = data!(42); // Creates Value::Data with integer
/// let name = data!("John"); // Creates Value::Data with string
/// let score = data!(3.14); // Creates Value::Data with float
/// ```
#[macro_export]
macro_rules! data {
    ($value:expr) => {
        $crate::value::Value::Data($crate::macros::into_xsd_type($value))
    };
}

/// Create a DateTime value
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let dt = datetime!("2024-01-01T00:00:00Z"); // Creates Value::Data(XsdAnySimpleType::DateTime)
/// let dt2 = datetime!("2024-12-31T23:59:59Z");
/// ```
#[macro_export]
macro_rules! datetime {
    ($value:expr) => {
        $crate::value::Value::Data(terminusdb_schema::XSDAnySimpleType::DateTime(
            chrono::DateTime::parse_from_rfc3339($value)
                .expect("Invalid datetime format, expected RFC3339 (e.g., '2024-01-01T00:00:00Z')")
                .with_timezone(&chrono::Utc),
        ))
    };
}

/// Create a List value
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let items = list![data!(1), data!(2), data!(3)];
/// let mixed = list![var!(x), node!("item"), data!("text")];
/// ```
#[macro_export]
macro_rules! list {
    [$($item:expr),* $(,)?] => {
        $crate::value::Value::List(vec![$($item),*])
    };
}

/// Create a Triple query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let t = triple!(var!(x), "rdf:type", "Person");
/// let t2 = triple!(node_var!(x), node_value!("name"), var!(name));
/// ```
#[macro_export]
macro_rules! triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::Triple($crate::triple::Triple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::Triple($crate::triple::Triple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: $graph,
        })
    };
}

/// Create an And query with multiple sub-queries
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = and!(
///     triple!(var!(x), "rdf:type", "Person"),
///     triple!(var!(x), "name", var!(name))
/// );
/// ```
#[macro_export]
macro_rules! and {
    ($($query:expr),+ $(,)?) => {
        $crate::query::Query::And($crate::query::And {
            and: vec![$($query),+]
        })
    };
}

/// Create an Or query with multiple sub-queries
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = or!(
///     triple!(var!(x), "rdf:type", "Person"),
///     triple!(var!(x), "rdf:type", "Organization")
/// );
/// ```
#[macro_export]
macro_rules! or {
    ($($query:expr),+ $(,)?) => {
        $crate::query::Query::Or($crate::query::Or {
            or: vec![$($query),+]
        })
    };
}

/// Create a Not query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = not!(triple!(var!(x), "archived", data!(true)));
/// ```
#[macro_export]
macro_rules! not {
    ($query:expr) => {
        $crate::query::Query::Not($crate::query::Not {
            query: Box::new($query),
        })
    };
}

/// Create a Select query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = select!([x, name], and!(
///     triple!(var!(x), "rdf:type", "Person"),
///     triple!(var!(x), "name", var!(name))
/// ));
/// ```
#[macro_export]
macro_rules! select {
    ([$($var:ident),+ $(,)?], $query:expr) => {
        $crate::query::Query::Select($crate::control::Select {
            variables: vec![$(stringify!($var).to_string()),+],
            query: Box::new($query),
        })
    };
    ([$($var:expr),+ $(,)?], $query:expr) => {
        $crate::query::Query::Select($crate::control::Select {
            variables: vec![$($var.to_string()),+],
            query: Box::new($query),
        })
    };
}

/// Create an Equals comparison
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = eq!(var!(x), data!(42));
/// ```
#[macro_export]
macro_rules! eq {
    ($left:expr, $right:expr) => {
        $crate::query::Query::Equals($crate::compare::Equals {
            left: $crate::macros::into_value($left),
            right: $crate::macros::into_value($right),
        })
    };
}

/// Create a Greater comparison
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = greater!(var!(age), data!(18));
/// ```
#[macro_export]
macro_rules! greater {
    ($left:expr, $right:expr) => {
        $crate::query::Query::Greater($crate::compare::Greater {
            left: $crate::macros::into_data_value($left),
            right: $crate::macros::into_data_value($right),
        })
    };
}

/// Create a Less comparison
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = less!(var!(age), data!(65));
/// ```
#[macro_export]
macro_rules! less {
    ($left:expr, $right:expr) => {
        $crate::query::Query::Less($crate::compare::Less {
            left: $crate::macros::into_data_value($left),
            right: $crate::macros::into_data_value($right),
        })
    };
}

/// Create a Path query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// use woql2::path::PathPattern;
/// let pattern = PathPattern::Predicate(...);
/// let q = path!(var!(start), pattern, var!(end));
/// ```
#[macro_export]
macro_rules! path {
    ($subject:expr, $pattern:expr, $object:expr) => {
        $crate::query::Query::Path($crate::query::Path {
            subject: $crate::macros::into_value($subject),
            pattern: $pattern,
            object: $crate::macros::into_value($object),
            path: None,
        })
    };
    ($subject:expr, $pattern:expr, $object:expr, $path:expr) => {
        $crate::query::Query::Path($crate::query::Path {
            subject: $crate::macros::into_value($subject),
            pattern: $pattern,
            object: $crate::macros::into_value($object),
            path: Some($crate::macros::into_value($path)),
        })
    };
}

/// Create a Limit query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = limit!(10, triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! limit {
    ($limit:expr, $query:expr) => {
        $crate::query::Query::Limit($crate::misc::Limit {
            limit: $limit,
            query: Box::new($query),
        })
    };
}

/// Create an Eval query for arithmetic expressions
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// use woql2::expression::ArithmeticExpression;
/// let expr = ArithmeticExpression::Plus(...);
/// let q = eval!(expr, var!(result));
/// ```
#[macro_export]
macro_rules! eval {
    ($expr:expr, $result:expr) => {
        $crate::query::Query::Eval($crate::query::Eval {
            expression: $expr,
            result_value: $crate::macros::into_arithmetic_value($result),
        })
    };
}

/// Create a ReadDocument query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = read_doc!(node!("doc:123"), var!(doc));
/// ```
#[macro_export]
macro_rules! read_doc {
    ($id:expr, $doc:expr) => {
        $crate::query::Query::ReadDocument($crate::document::ReadDocument {
            identifier: $crate::macros::into_node_value($id),
            document: $crate::macros::into_value($doc),
        })
    };
}

/// Create an InsertDocument query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = insert_doc!(var!(doc));
/// let q2 = insert_doc!(var!(doc), node!("doc:123"));
/// ```
#[macro_export]
macro_rules! insert_doc {
    ($doc:expr) => {
        $crate::query::Query::InsertDocument($crate::document::InsertDocument {
            document: $crate::macros::into_value($doc),
            identifier: None,
        })
    };
    ($doc:expr, $id:expr) => {
        $crate::query::Query::InsertDocument($crate::document::InsertDocument {
            document: $crate::macros::into_value($doc),
            identifier: Some($crate::macros::into_node_value($id)),
        })
    };
}

/// Create an UpdateDocument query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = update_doc!(var!(doc));
/// let q2 = update_doc!(var!(doc), node!("doc:123"));
/// ```
#[macro_export]
macro_rules! update_doc {
    ($doc:expr) => {
        $crate::query::Query::UpdateDocument($crate::document::UpdateDocument {
            document: $crate::macros::into_value($doc),
            identifier: None,
        })
    };
    ($doc:expr, $id:expr) => {
        $crate::query::Query::UpdateDocument($crate::document::UpdateDocument {
            document: $crate::macros::into_value($doc),
            identifier: Some($crate::macros::into_node_value($id)),
        })
    };
}

/// Create a DeleteDocument query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = delete_doc!(node!("doc:123"));
/// ```
#[macro_export]
macro_rules! delete_doc {
    ($id:expr) => {
        $crate::query::Query::DeleteDocument($crate::document::DeleteDocument {
            identifier: $crate::macros::into_node_value($id),
        })
    };
}

/// Create an If-Then-Else query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = if_then_else!(
///     greater!(var!(age), data!(18)),
///     triple!(var!(x), "status", "adult"),
///     triple!(var!(x), "status", "minor")
/// );
/// ```
#[macro_export]
macro_rules! if_then_else {
    ($if_query:expr, $then_query:expr, $else_query:expr) => {
        $crate::query::Query::If($crate::control::If {
            test: Box::new($if_query),
            then_query: Box::new($then_query),
            else_query: Box::new($else_query),
        })
    };
}

// ===== SHORTCUT MACROS =====
// These provide convenient shortcuts for common WOQL patterns

/// Shortcut for creating rdf:type triples
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = type!(var!(x), "Person"); // equivalent to triple!(var!(x), "rdf:type", "Person")
/// let q2 = type!(var!(x), var!(type)); // with variable type
/// let q3 = type!(var!(x), Person); // identifier automatically wrapped: triple!(var!(x), "rdf:type", "@schema:Person")
/// let q4 = type!(var!(x), typename!(Person)); // explicit typename! usage
/// ```
#[macro_export]
macro_rules! type_ {
    // Pattern for identifiers - automatically wrap with typename!
    ($subject:expr, $type:ident) => {
        triple!($subject, "rdf:type", typename!($type))
    };
    // Pattern for expressions - use as-is
    ($subject:expr, $type:expr) => {
        triple!($subject, "rdf:type", $type)
    };
}

/// Shortcut for isa type checking
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = isa!(var!(x), "Person"); // checks if x is of type Person
/// let q2 = isa!(var!(x), Person); // identifier automatically wrapped to check against "@schema:Person"
/// let q3 = isa!(var!(x), typename!(Person)); // explicit typename! usage
/// ```
#[macro_export]
macro_rules! isa {
    // Pattern for identifiers - automatically wrap with typename!
    ($element:expr, $type:ident) => {
        $crate::query::Query::IsA($crate::compare::IsA {
            element: $crate::macros::into_node_value($element),
            type_of: $crate::macros::into_node_value(typename!($type)),
        })
    };
    // Pattern for expressions - use as-is
    ($element:expr, $type:expr) => {
        $crate::query::Query::IsA($crate::compare::IsA {
            element: $crate::macros::into_node_value($element),
            type_of: $crate::macros::into_node_value($type),
        })
    };
}

/// Shortcut for optional queries
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = optional!(triple!(var!(x), "email", var!(email)));
/// ```
#[macro_export]
macro_rules! optional {
    ($query:expr) => {
        $crate::query::Query::WoqlOptional($crate::control::WoqlOptional {
            query: Box::new($query),
        })
    };
}

/// Shortcut for distinct with all variables from a select
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = distinct_vars!([x, y], triple!(var!(x), "knows", var!(y)));
/// ```
#[macro_export]
macro_rules! distinct_vars {
    ([$($var:ident),+ $(,)?], $query:expr) => {
        $crate::query::Query::Distinct($crate::control::Distinct {
            variables: vec![$(stringify!($var).to_string()),+],
            query: Box::new($query),
        })
    };
    ([$($var:expr),+ $(,)?], $query:expr) => {
        $crate::query::Query::Distinct($crate::control::Distinct {
            variables: vec![$($var.to_string()),+],
            query: Box::new($query),
        })
    };
}

/// Shortcut for count queries
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = count_into!(triple!(var!(x), "rdf:type", "Person"), var!(count));
/// ```
#[macro_export]
macro_rules! count_into {
    ($query:expr, $count_var:expr) => {
        $crate::query::Query::Count($crate::misc::Count {
            query: Box::new($query),
            count: $crate::macros::into_data_value($count_var),
        })
    };
}

/// Alias for count_into! macro
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = count!(triple!(var!(x), "rdf:type", "Person"), var!(count));
/// ```
#[macro_export]
macro_rules! count {
    ($query:expr, $count_var:expr) => {
        count_into!($query, $count_var)
    };
}

/// Create a Start query for pagination
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = start!(10, triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! start {
    ($start:expr, $query:expr) => {
        $crate::query::Query::Start($crate::misc::Start {
            start: $start as u64,
            query: Box::new($query),
        })
    };
}

/// Shortcut for typecast operations
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = cast!(var!(x), "xsd:integer", var!(int_x));
/// ```
#[macro_export]
macro_rules! cast {
    ($value:expr, $type:expr, $result:expr) => {
        $crate::query::Query::Typecast($crate::compare::Typecast {
            value: $crate::macros::into_value($value),
            type_uri: $crate::macros::into_node_value($type),
            result_value: $crate::macros::into_value($result),
        })
    };
}

/// Shortcut for sum operations on lists
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = sum!(var!(numbers), var!(total));
/// ```
#[macro_export]
macro_rules! sum {
    ($list:expr, $result:expr) => {
        $crate::query::Query::Sum($crate::collection::Sum {
            list: $crate::macros::into_list_or_variable($list),
            result: $crate::macros::into_arithmetic_value($result),
        })
    };
}

/// Shortcut for concatenating strings
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = concat!(list![data!("Hello"), data!(" "), data!("World")], var!(result));
/// ```
#[macro_export]
macro_rules! concat {
    ($list:expr, $result:expr) => {
        $crate::query::Query::Concatenate($crate::string::Concatenate {
            list: $crate::macros::into_list_or_variable($list),
            result_string: $crate::macros::into_data_value($result),
        })
    };
}

/// Shortcut for member checking
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = member!(var!(item), var!(list));
/// ```
#[macro_export]
macro_rules! member {
    ($member:expr, $list:expr) => {
        $crate::query::Query::Member($crate::collection::Member {
            member: $crate::macros::into_data_value($member),
            list: $crate::macros::into_list_or_variable($list),
        })
    };
}

/// Shortcut for immediately queries (commit after each solution)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = immediately!(insert_doc!(var!(doc)));
/// ```
#[macro_export]
macro_rules! immediately {
    ($query:expr) => {
        $crate::query::Query::Immediately($crate::control::Immediately {
            query: Box::new($query),
        })
    };
}

/// Shortcut for link triples (object references)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = link!(var!(person), "friend", var!(friend));
/// ```
#[macro_export]
macro_rules! link {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::Link($crate::triple::Link {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::Link($crate::triple::Link {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: $graph,
        })
    };
}

/// Shortcut for data triples (literal values)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = data_triple!(var!(person), "age", data!(25));
/// ```
#[macro_export]
macro_rules! data_triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::Data($crate::triple::Data {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::Data($crate::triple::Data {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: $graph,
        })
    };
}

/// Shortcut for regex matching
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = regex!("^[A-Z]", var!(name)); // match names starting with uppercase
/// let q2 = regex!("\\d+", var!(text), var!(matches)); // capture matches
/// ```
#[macro_export]
macro_rules! regex {
    ($pattern:expr, $string:expr) => {
        $crate::query::Query::Regexp($crate::string::Regexp {
            pattern: $crate::macros::into_data_value($pattern),
            string: $crate::macros::into_data_value($string),
            result: Some($crate::macros::into_data_value(var!(_regex_result))),
        })
    };
    ($pattern:expr, $string:expr, $result:expr) => {
        $crate::query::Query::Regexp($crate::string::Regexp {
            pattern: $crate::macros::into_data_value($pattern),
            string: $crate::macros::into_data_value($string),
            result: Some($crate::macros::into_data_value($result)),
        })
    };
}

/// Shortcut for trim operations
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = trim!(var!(input), var!(output));
/// ```
#[macro_export]
macro_rules! trim {
    ($input:expr, $output:expr) => {
        $crate::query::Query::Trim($crate::string::Trim {
            untrimmed: $crate::macros::into_data_value($input),
            trimmed: $crate::macros::into_data_value($output),
        })
    };
}

/// True query constant
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = true_!(); // always succeeds
/// ```
#[macro_export]
macro_rules! true_ {
    () => {
        $crate::query::Query::True($crate::query::True {})
    };
}

/// String operation that checks if a string starts with a pattern
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = starts_with!(var!(name), "Dr.");
/// let q2 = starts_with!(var!(title), data!("Chapter"));
/// ```
#[macro_export]
macro_rules! starts_with {
    ($string:expr, $prefix:expr) => {
        regex!(data!(format!("^{}.*", $prefix)), $string)
    };
}

/// String operation that checks if a string ends with a pattern
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = ends_with!(var!(email), "@example.com");
/// let q2 = ends_with!(var!(filename), ".pdf");
/// ```
#[macro_export]
macro_rules! ends_with {
    ($string:expr, $suffix:expr) => {
        regex!(data!(format!(".*{}$", $suffix)), $string)
    };
}

/// String operation that checks if a string contains a pattern
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = contains!(var!(description), "important");
/// let q2 = contains!(var!(text), "search term");
/// ```
#[macro_export]
macro_rules! contains {
    ($string:expr, $pattern:expr) => {
        regex!(data!(format!(".*{}.*", $pattern)), $string)
    };
}

/// Get today's date in ISO 8601 format
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = compare!((var!(date)) == (today!()));
/// let q2 = after!(var!(created), today!());
/// ```
#[macro_export]
macro_rules! today {
    () => {
        $crate::value::Value::Data(terminusdb_schema::XSDAnySimpleType::DateTime(
            chrono::Utc::now(),
        ))
    };
}

/// Check if a date is after another date
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = after!(var!(end_date), datetime!("2024-01-01T00:00:00Z"));
/// let q2 = after!(var!(created), today!());
/// ```
#[macro_export]
macro_rules! after {
    ($date:expr, $reference:expr) => {
        compare!(($date) > ($reference))
    };
}

/// Check if a date is before another date
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = before!(var!(start_date), datetime!("2024-12-31T23:59:59Z"));
/// let q2 = before!(var!(deadline), today!());
/// ```
#[macro_export]
macro_rules! before {
    ($date:expr, $reference:expr) => {
        compare!(($date) < ($reference))
    };
}

/// Check if a date is between two dates (inclusive)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = in_between!(var!(date), datetime!("2024-01-01T00:00:00Z"), datetime!("2024-12-31T23:59:59Z"));
/// let q2 = in_between!(var!(event_date), today!(), datetime!("2025-01-01T00:00:00Z"));
/// ```
#[macro_export]
macro_rules! in_between {
    ($date:expr, $start:expr, $end:expr) => {
        and!(compare!(($date) >= ($start)), compare!(($date) <= ($end)))
    };
}

/// Check if today's date is between two dates (inclusive)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = today_in_between!(datetime!("2024-01-01T00:00:00Z"), datetime!("2024-12-31T23:59:59Z"));
/// let q2 = today_in_between!(var!(start_date), var!(end_date));
/// ```
#[macro_export]
macro_rules! today_in_between {
    ($start:expr, $end:expr) => {
        in_between!(today!(), $start, $end)
    };
}

/// Compare macro using Rust comparison operators
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = compare!((var!(age)) > (data!(18)));      // Greater
/// let q2 = compare!((var!(age)) < (data!(65)));     // Less
/// let q3 = compare!((var!(x)) == (var!(y)));        // Equals
/// let q4 = compare!((var!(age)) >= (data!(18)));    // Greater or equal (using Or)
/// let q5 = compare!((var!(age)) <= (data!(65)));    // Less or equal (using Or)
/// ```
#[macro_export]
macro_rules! compare {
    // Greater than
    (($left:expr) > ($right:expr)) => {
        greater!($left, $right)
    };
    ($left:ident > ($right:expr)) => {
        greater!($left, $right)
    };
    (($left:expr) > $right:ident) => {
        greater!($left, $right)
    };
    ($left:ident > $right:ident) => {
        greater!($left, $right)
    };

    // Less than
    (($left:expr) < ($right:expr)) => {
        less!($left, $right)
    };
    ($left:ident < ($right:expr)) => {
        less!($left, $right)
    };
    (($left:expr) < $right:ident) => {
        less!($left, $right)
    };
    ($left:ident < $right:ident) => {
        less!($left, $right)
    };

    // Equals
    (($left:expr) == ($right:expr)) => {
        eq!($left, $right)
    };
    ($left:ident == ($right:expr)) => {
        eq!($left, $right)
    };
    (($left:expr) == $right:ident) => {
        eq!($left, $right)
    };
    ($left:ident == $right:ident) => {
        eq!($left, $right)
    };

    // Greater than or equal
    (($left:expr) >= ($right:expr)) => {{
        let left_val = $left;
        let right_val = $right;
        or!(
            greater!(left_val.clone(), right_val.clone()),
            eq!(left_val, right_val)
        )
    }};
    ($left:ident >= ($right:expr)) => {{
        let right_val = $right;
        or!(
            greater!($left.clone(), right_val.clone()),
            eq!($left, right_val)
        )
    }};
    (($left:expr) >= $right:ident) => {{
        let left_val = $left;
        or!(
            greater!(left_val.clone(), $right.clone()),
            eq!(left_val, $right)
        )
    }};
    ($left:ident >= $right:ident) => {
        or!(greater!($left.clone(), $right.clone()), eq!($left, $right))
    };

    // Less than or equal
    (($left:expr) <= ($right:expr)) => {{
        let left_val = $left;
        let right_val = $right;
        or!(
            less!(left_val.clone(), right_val.clone()),
            eq!(left_val, right_val)
        )
    }};
    ($left:ident <= ($right:expr)) => {{
        let right_val = $right;
        or!(
            less!($left.clone(), right_val.clone()),
            eq!($left, right_val)
        )
    }};
    (($left:expr) <= $right:ident) => {{
        let left_val = $left;
        or!(
            less!(left_val.clone(), $right.clone()),
            eq!(left_val, $right)
        )
    }};
    ($left:ident <= $right:ident) => {
        or!(less!($left.clone(), $right.clone()), eq!($left, $right))
    };

    // Not equals
    (($left:expr) != ($right:expr)) => {
        not!(eq!($left, $right))
    };
    ($left:ident != ($right:expr)) => {
        not!(eq!($left, $right))
    };
    (($left:expr) != $right:ident) => {
        not!(eq!($left, $right))
    };
    ($left:ident != $right:ident) => {
        not!(eq!($left, $right))
    };
}

// ===== ADDITIONAL MACROS FOR MISSING WOQL2 STRUCTS =====

/// Create a LexicalKey query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = lexical_key!(data!("doc:"), list![data!("Person"), data!("123")], var!(uri));
/// ```
#[macro_export]
macro_rules! lexical_key {
    ($base:expr, $key_list:expr, $uri:expr) => {
        $crate::query::Query::LexicalKey($crate::misc::LexicalKey {
            base: $crate::macros::into_data_value($base),
            key_list: $key_list.into_iter().map(|v| $crate::macros::into_data_value(v)).collect(),
            uri: $crate::macros::into_node_value($uri),
        })
    };
}

/// Create a HashKey query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = hash_key!(data!("doc:"), list![data!("User"), data!("john@example.com")], var!(uri));
/// ```
#[macro_export]
macro_rules! hash_key {
    ($base:expr, $key_list:expr, $uri:expr) => {
        $crate::query::Query::HashKey($crate::misc::HashKey {
            base: $crate::macros::into_data_value($base),
            key_list: $key_list.into_iter().map(|v| $crate::macros::into_data_value(v)).collect(),
            uri: $crate::macros::into_node_value($uri),
        })
    };
}

/// Create a RandomKey query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = random_key!(data!("doc:"), var!(uri));
/// ```
#[macro_export]
macro_rules! random_key {
    ($base:expr, $uri:expr) => {
        $crate::query::Query::RandomKey($crate::misc::RandomKey {
            base: $crate::macros::into_data_value($base),
            uri: $crate::macros::into_node_value($uri),
        })
    };
}

/// Create a Size query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = size!("db:mydb", var!(size));
/// ```
#[macro_export]
macro_rules! size {
    ($resource:expr, $size:expr) => {
        $crate::query::Query::Size($crate::misc::Size {
            resource: $resource.to_string(),
            size: $crate::macros::into_data_value($size),
        })
    };
}

/// Create a TripleCount query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = triple_count!("db:mydb", var!(count));
/// ```
#[macro_export]
macro_rules! triple_count {
    ($resource:expr, $count:expr) => {
        $crate::query::Query::TripleCount($crate::misc::TripleCount {
            resource: $resource.to_string(),
            count: $crate::macros::into_data_value($count),
        })
    };
}

/// Create a Lower query for lowercase conversion
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = lower!(var!(name), var!(lower_name));
/// ```
#[macro_export]
macro_rules! lower {
    ($mixed:expr, $lower:expr) => {
        $crate::query::Query::Lower($crate::string::Lower {
            mixed: $crate::macros::into_data_value($mixed),
            lower: $crate::macros::into_data_value($lower),
        })
    };
}

/// Create an Upper query for uppercase conversion
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = upper!(var!(name), var!(upper_name));
/// ```
#[macro_export]
macro_rules! upper {
    ($mixed:expr, $upper:expr) => {
        $crate::query::Query::Upper($crate::string::Upper {
            mixed: $crate::macros::into_data_value($mixed),
            upper: $crate::macros::into_data_value($upper),
        })
    };
}

/// Create a Pad query for string padding
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = pad!(var!(text), data!("*"), data!(10), var!(padded));
/// ```
#[macro_export]
macro_rules! pad {
    ($string:expr, $char:expr, $times:expr, $result:expr) => {
        $crate::query::Query::Pad($crate::string::Pad {
            string: $crate::macros::into_data_value($string),
            char: $crate::macros::into_data_value($char),
            times: $crate::macros::into_data_value($times),
            result_string: $crate::macros::into_data_value($result),
        })
    };
}

/// Create a Split query for string splitting
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = split!(var!(text), data!(","), var!(parts));
/// ```
#[macro_export]
macro_rules! split {
    ($string:expr, $pattern:expr, $list:expr) => {
        $crate::query::Query::Split($crate::string::Split {
            string: $crate::macros::into_data_value($string),
            pattern: $crate::macros::into_data_value($pattern),
            list: $crate::macros::into_data_value($list),
        })
    };
}

/// Create a Join query for joining strings
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = join!(var!(parts), data!(", "), var!(result));
/// ```
#[macro_export]
macro_rules! join {
    ($list:expr, $separator:expr, $result:expr) => {
        $crate::query::Query::Join($crate::string::Join {
            list: $crate::macros::into_list_or_variable($list),
            separator: $crate::macros::into_data_value($separator),
            result_string: $crate::macros::into_data_value($result),
        })
    };
}

/// Create a Substring query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = substring!(var!(text), data!(0), data!(5), data!(0), var!(sub));
/// ```
#[macro_export]
macro_rules! substring {
    ($string:expr, $before:expr, $length:expr, $after:expr, $substring:expr) => {
        $crate::query::Query::Substring($crate::string::Substring {
            string: $crate::macros::into_data_value($string),
            before: $crate::macros::into_data_value($before),
            length: $crate::macros::into_data_value($length),
            after: $crate::macros::into_data_value($after),
            substring: $crate::macros::into_data_value($substring),
        })
    };
}

/// Create a Like query for pattern matching
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = like!(var!(text), data!("%pattern%"), var!(similarity));
/// ```
#[macro_export]
macro_rules! like {
    ($string:expr, $pattern:expr, $similarity:expr) => {
        $crate::query::Query::Like($crate::string::Like {
            left: $crate::macros::into_data_value($string),
            right: $crate::macros::into_data_value($pattern),
            similarity: $crate::macros::into_data_value($similarity),
        })
    };
}

/// Create a Plus arithmetic expression
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = plus!(var!(x), var!(y), var!(sum));
/// ```
#[macro_export]
macro_rules! plus {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Plus($crate::expression::Plus {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create a Minus arithmetic expression
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = minus!(var!(x), var!(y), var!(difference));
/// ```
#[macro_export]
macro_rules! minus {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Minus($crate::expression::Minus {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create a Times arithmetic expression
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = times!(var!(x), var!(y), var!(product));
/// ```
#[macro_export]
macro_rules! times {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Times($crate::expression::Times {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create a Divide arithmetic expression
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = divide!(var!(x), var!(y), var!(quotient));
/// ```
#[macro_export]
macro_rules! divide {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Divide($crate::expression::Divide {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create a Div arithmetic expression (integer division)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = div!(var!(x), var!(y), var!(quotient));
/// ```
#[macro_export]
macro_rules! div {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Div($crate::expression::Div {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create an Exp arithmetic expression (exponentiation)
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = exp!(var!(base), var!(exponent), var!(result));
/// ```
#[macro_export]
macro_rules! exp {
    ($left:expr, $right:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Exp($crate::expression::Exp {
                left: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($left))),
                right: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($right))),
            }),
            $result
        )
    };
}

/// Create a Floor arithmetic expression
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = floor!(var!(x), var!(floored));
/// ```
#[macro_export]
macro_rules! floor {
    ($argument:expr, $result:expr) => {
        eval!(
            $crate::expression::ArithmeticExpression::Floor($crate::expression::Floor {
                argument: Box::new($crate::expression::ArithmeticExpression::Value($crate::macros::into_arithmetic_value($argument))),
            }),
            $result
        )
    };
}

/// Create a Length query for getting list length
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = length!(var!(list), var!(len));
/// ```
#[macro_export]
macro_rules! length {
    ($list:expr, $length:expr) => {
        $crate::query::Query::Length($crate::collection::Length {
            list: $crate::macros::into_data_value($list),
            length: $crate::macros::into_data_value($length),
        })
    };
}

/// Create a Dot query for accessing document fields
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = dot!(var!(doc), data!("field"), var!(value));
/// ```
#[macro_export]
macro_rules! dot {
    ($document:expr, $field:expr, $value:expr) => {
        $crate::query::Query::Dot($crate::collection::Dot {
            document: $crate::macros::into_data_value($document),
            field: $crate::macros::into_data_value($field),
            value: $crate::macros::into_data_value($value),
        })
    };
}

/// Create a Subsumption query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = subsumption!(var!(child), var!(parent));
/// ```
#[macro_export]
macro_rules! subsumption {
    ($child:expr, $parent:expr) => {
        $crate::query::Query::Subsumption($crate::compare::Subsumption {
            child: $crate::macros::into_node_value($child),
            parent: $crate::macros::into_node_value($parent),
        })
    };
}

/// Create a TypeOf query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = typeof!(var!(element), var!(type));
/// ```
#[macro_export]
macro_rules! typeof_ {
    ($element:expr, $type:expr) => {
        $crate::query::Query::TypeOf($crate::compare::TypeOf {
            value: $crate::macros::into_value($element),
            type_uri: $crate::macros::into_node_value($type),
        })
    };
}

/// Create a Using query for resource specification
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = using!("db:mydb", triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! using {
    ($collection:expr, $query:expr) => {
        $crate::query::Query::Using($crate::control::Using {
            collection: $collection.to_string(),
            query: Box::new($query),
        })
    };
}

/// Create a From query for graph source specification
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = from!("db:mydb/graph", triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! from {
    ($graph:expr, $query:expr) => {
        $crate::query::Query::From($crate::control::From {
            graph: $graph.to_string(),
            query: Box::new($query),
        })
    };
}

/// Create an Into query for graph target specification
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = into!("db:mydb/graph", insert_doc!(var!(doc)));
/// ```
#[macro_export]
macro_rules! into {
    ($graph:expr, $query:expr) => {
        $crate::query::Query::Into($crate::control::Into {
            graph: $graph.to_string(),
            query: Box::new($query),
        })
    };
}

/// Create a Pin query for variable pinning
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = pin!(triple!(var!(x), "name", var!(name)));
/// ```
#[macro_export]
macro_rules! pin {
    ($query:expr) => {
        $crate::query::Query::Pin($crate::control::Pin {
            query: Box::new($query),
        })
    };
}

/// Create a Once query for once-only execution
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = once!(triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! once {
    ($query:expr) => {
        $crate::query::Query::Once($crate::control::Once {
            query: Box::new($query),
        })
    };
}

/// Create an AddTriple query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = add_triple!(node_var!(x), node_value!("name"), var!(name));
/// ```
#[macro_export]
macro_rules! add_triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddTriple($crate::triple::AddTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddTriple($crate::triple::AddTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: $graph,
        })
    };
}

/// Create an AddedTriple query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = added_triple!(node_var!(x), node_value!("name"), var!(name));
/// ```
#[macro_export]
macro_rules! added_triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddedTriple($crate::triple::AddedTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddedTriple($crate::triple::AddedTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: $graph,
        })
    };
}

/// Create a DeleteTriple query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = delete_triple!(node_var!(x), node_value!("name"), var!(name));
/// ```
#[macro_export]
macro_rules! delete_triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::DeleteTriple($crate::triple::DeleteTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::DeleteTriple($crate::triple::DeleteTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: $graph,
        })
    };
}

/// Create a DeletedTriple query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = deleted_triple!(node_var!(x), node_value!("name"), var!(name));
/// ```
#[macro_export]
macro_rules! deleted_triple {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::DeletedTriple($crate::triple::DeletedTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::DeletedTriple($crate::triple::DeletedTriple {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_value($object),
            graph: $graph,
        })
    };
}

/// Create an AddLink query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = add_link!(node_var!(x), node_value!("friend"), node_var!(y));
/// ```
#[macro_export]
macro_rules! add_link {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddLink($crate::triple::AddLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddLink($crate::triple::AddLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: $graph,
        })
    };
}

/// Create an AddedLink query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = added_link!(node_var!(x), node_value!("friend"), node_var!(y));
/// ```
#[macro_export]
macro_rules! added_link {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddedLink($crate::triple::AddedLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddedLink($crate::triple::AddedLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: $graph,
        })
    };
}

/// Create an AddData query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = add_data!(node_var!(x), node_value!("age"), data!(25));
/// ```
#[macro_export]
macro_rules! add_data {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddData($crate::triple::AddData {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddData($crate::triple::AddData {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: $graph,
        })
    };
}

/// Create an AddedData query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = added_data!(node_var!(x), node_value!("age"), data!(25));
/// ```
#[macro_export]
macro_rules! added_data {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::AddedData($crate::triple::AddedData {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::AddedData($crate::triple::AddedData {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_data_value($object),
            graph: $graph,
        })
    };
}

/// Create a DeleteLink query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = delete_link!(node_var!(x), node_value!("friend"), node_var!(y));
/// ```
#[macro_export]
macro_rules! delete_link {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::DeleteLink($crate::triple::DeleteLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::DeleteLink($crate::triple::DeleteLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: $graph,
        })
    };
}

/// Create a DeletedLink query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = deleted_link!(node_var!(x), node_value!("friend"), node_var!(y));
/// ```
#[macro_export]
macro_rules! deleted_link {
    ($subject:expr, $predicate:expr, $object:expr) => {
        $crate::query::Query::DeletedLink($crate::triple::DeletedLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: terminusdb_schema::GraphType::Instance,
        })
    };
    ($subject:expr, $predicate:expr, $object:expr, $graph:expr) => {
        $crate::query::Query::DeletedLink($crate::triple::DeletedLink {
            subject: $crate::macros::into_node_value($subject),
            predicate: $crate::macros::into_node_value($predicate),
            object: $crate::macros::into_node_value($object),
            graph: $graph,
        })
    };
}

/// Create a NamedQuery
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = named_query!("find_persons", triple!(var!(x), "rdf:type", "Person"));
/// ```
#[macro_export]
macro_rules! named_query {
    ($name:expr, $query:expr) => {
        $crate::query::NamedQuery {
            name: $name.to_string(),
            query: Box::new($query),
        }
    };
}

/// Create a NamedParametricQuery
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = named_parametric_query!("find_by_type", ["type"], triple!(var!(x), "rdf:type", var!(type)));
/// ```
#[macro_export]
macro_rules! named_parametric_query {
    ($name:expr, [$($param:expr),* $(,)?], $query:expr) => {
        $crate::query::NamedParametricQuery {
            name: $name.to_string(),
            parameters: vec![$($param.to_string()),*],
            query: Box::new($query),
        }
    };
}

/// Create a Call query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = call!("find_persons", triple!(var!(x), "rdf:type", "Person"));
/// let q2 = call!("find_by_type", [var!(type)], triple!(var!(x), "rdf:type", var!(type)));
/// ```
#[macro_export]
macro_rules! call {
    ($name:expr, $query:expr) => {
        $crate::query::Call {
            name: $name.to_string(),
            arguments: vec![],
            query: $query,
        }
    };
    ($name:expr, [$($arg:expr),* $(,)?], $query:expr) => {
        $crate::query::Call {
            name: $name.to_string(),
            arguments: vec![$($crate::macros::into_value($arg)),*],
            query: $query,
        }
    };
}

/// Create an OrderBy query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// use terminusdb_woql2::order::{Order, OrderTemplate};
/// let q = order_by!(
///     [OrderTemplate { variable: "name".to_string(), order: Order::Asc }],
///     triple!(var!(x), "name", var!(name))
/// );
/// ```
#[macro_export]
macro_rules! order_by {
    ([$($template:expr),* $(,)?], $query:expr) => {
        $crate::query::Query::OrderBy($crate::order::OrderBy {
            ordering: vec![$($template),*],
            query: Box::new($query),
        })
    };
}

/// Create a GroupBy query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let q = group_by!(var!(group), ["type"], var!(groups), triple!(var!(x), "rdf:type", var!(type)));
/// ```
#[macro_export]
macro_rules! group_by {
    ($template:expr, [$($var:expr),* $(,)?], $grouped:expr, $query:expr) => {
        $crate::query::Query::GroupBy($crate::order::GroupBy {
            template: $crate::macros::into_value($template),
            group_by: vec![$($var.to_string()),*],
            grouped_value: $crate::macros::into_value($grouped),
            query: Box::new($query),
        })
    };
}

/// Create a Get query
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// use terminusdb_woql2::get::{Column, QueryResource};
/// let resource = QueryResource { ... };
/// let q = get!([Column { ... }], resource, var!(response));
/// ```
#[macro_export]
macro_rules! get {
    ([$($column:expr),* $(,)?], $resource:expr, $response:expr) => {
        $crate::query::Query::Get($crate::get::Get {
            columns: vec![$($column),*],
            resource: $resource,
            response: $crate::macros::into_node_value($response),
        })
    };
}

/// Create a dictionary Value
///
/// # Examples
/// ```
/// # use terminusdb_woql2::*;
/// let dict = dictionary!{
///     "name" => var!(name),
///     "age" => data!(25)
/// };
/// ```
#[macro_export]
macro_rules! dictionary {
    ($($key:expr => $value:expr),* $(,)?) => {
        $crate::value::Value::Dictionary($crate::value::DictionaryTemplate {
            data: {
                let mut set = std::collections::BTreeSet::new();
                $(
                    set.insert($crate::value::FieldValuePair {
                        field: $key.to_string(),
                        value: $crate::macros::into_value($value),
                    });
                )*
                set
            }
        })
    };
}


// Helper functions for conversion
pub use self::conversion::*;

mod conversion {
    use crate::expression::ArithmeticValue;
    use crate::value::{DataValue, ListOrVariable, NodeValue, Value};
    use terminusdb_schema::XSDAnySimpleType;

    /// Convert various types into XSDAnySimpleType
    pub fn into_xsd_type<T: IntoXsdType>(value: T) -> XSDAnySimpleType {
        value.into_xsd_type()
    }

    /// Convert various types into Value
    pub fn into_value<T: IntoValue>(value: T) -> Value {
        value.into_value()
    }

    /// Convert various types into NodeValue
    pub fn into_node_value<T: IntoNodeValue>(value: T) -> NodeValue {
        value.into_node_value()
    }

    /// Convert various types into ArithmeticValue
    pub fn into_arithmetic_value<T: IntoArithmeticValue>(value: T) -> ArithmeticValue {
        value.into_arithmetic_value()
    }

    /// Convert various types into DataValue
    pub fn into_data_value<T: IntoDataValue>(value: T) -> DataValue {
        value.into_data_value()
    }

    /// Convert various types into ListOrVariable
    pub fn into_list_or_variable<T: IntoListOrVariable>(value: T) -> ListOrVariable {
        value.into_list_or_variable()
    }

    // Trait for converting to XSDAnySimpleType
    pub trait IntoXsdType {
        fn into_xsd_type(self) -> XSDAnySimpleType;
    }

    impl IntoXsdType for i32 {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Integer(self as i64)
        }
    }

    impl IntoXsdType for i64 {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Integer(self)
        }
    }

    impl IntoXsdType for f32 {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Float(self as f64)
        }
    }

    impl IntoXsdType for f64 {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Float(self)
        }
    }

    impl IntoXsdType for bool {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Boolean(self)
        }
    }

    impl IntoXsdType for &str {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::String(self.to_string())
        }
    }

    impl IntoXsdType for String {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::String(self)
        }
    }

    // Trait for converting to Value
    pub trait IntoValue {
        fn into_value(self) -> Value;
    }

    impl IntoValue for Value {
        fn into_value(self) -> Value {
            self
        }
    }

    impl IntoValue for &str {
        fn into_value(self) -> Value {
            Value::Node(self.to_string())
        }
    }

    impl IntoValue for String {
        fn into_value(self) -> Value {
            Value::Node(self)
        }
    }

    impl IntoValue for i32 {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Integer(self as i64))
        }
    }

    impl IntoValue for i64 {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Integer(self))
        }
    }

    impl IntoValue for f32 {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Float(self as f64))
        }
    }

    impl IntoValue for f64 {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Float(self))
        }
    }

    impl IntoValue for bool {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Boolean(self))
        }
    }

    // Trait for converting to NodeValue
    pub trait IntoNodeValue {
        fn into_node_value(self) -> NodeValue;
    }

    impl IntoNodeValue for NodeValue {
        fn into_node_value(self) -> NodeValue {
            self
        }
    }

    impl IntoNodeValue for &str {
        fn into_node_value(self) -> NodeValue {
            NodeValue::Node(self.to_string())
        }
    }

    impl IntoNodeValue for String {
        fn into_node_value(self) -> NodeValue {
            NodeValue::Node(self)
        }
    }

    impl IntoNodeValue for Value {
        fn into_node_value(self) -> NodeValue {
            match self {
                Value::Variable(v) => NodeValue::Variable(v),
                Value::Node(n) => NodeValue::Node(n),
                _ => panic!("Cannot convert data/list/dictionary Value to NodeValue"),
            }
        }
    }

    // Trait for converting to ArithmeticValue
    pub trait IntoArithmeticValue {
        fn into_arithmetic_value(self) -> ArithmeticValue;
    }

    impl IntoArithmeticValue for ArithmeticValue {
        fn into_arithmetic_value(self) -> ArithmeticValue {
            self
        }
    }

    impl IntoArithmeticValue for Value {
        fn into_arithmetic_value(self) -> ArithmeticValue {
            match self {
                Value::Variable(v) => ArithmeticValue::Variable(v),
                Value::Data(d) => ArithmeticValue::Data(d),
                _ => panic!("Cannot convert non-variable/data Value to ArithmeticValue"),
            }
        }
    }

    // Trait for converting to DataValue
    pub trait IntoDataValue {
        fn into_data_value(self) -> DataValue;
    }

    impl IntoDataValue for DataValue {
        fn into_data_value(self) -> DataValue {
            self
        }
    }

    impl IntoDataValue for Value {
        fn into_data_value(self) -> DataValue {
            match self {
                Value::Variable(v) => DataValue::Variable(v),
                Value::Data(d) => DataValue::Data(d),
                Value::List(items) => {
                    // Convert Value list to DataValue list
                    let data_items: Vec<DataValue> =
                        items.into_iter().map(|v| v.into_data_value()).collect();
                    DataValue::List(data_items)
                }
                _ => panic!("Cannot convert node/dictionary Value to DataValue"),
            }
        }
    }

    impl IntoDataValue for &str {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::String(self.to_string()))
        }
    }

    impl IntoDataValue for String {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::String(self))
        }
    }

    impl IntoDataValue for i32 {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::Integer(self as i64))
        }
    }

    impl IntoDataValue for u32 {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::UnsignedInt(self as usize))
        }
    }

    impl IntoDataValue for usize {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::UnsignedInt(self as usize))
        }
    }

    impl IntoDataValue for i64 {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::Integer(self))
        }
    }

    impl IntoDataValue for f32 {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::Float(self as f64))
        }
    }

    impl IntoDataValue for f64 {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::Float(self))
        }
    }

    impl IntoDataValue for bool {
        fn into_data_value(self) -> DataValue {
            DataValue::Data(XSDAnySimpleType::Boolean(self))
        }
    }

    // Trait for converting to ListOrVariable
    pub trait IntoListOrVariable {
        fn into_list_or_variable(self) -> ListOrVariable;
    }

    impl IntoListOrVariable for ListOrVariable {
        fn into_list_or_variable(self) -> ListOrVariable {
            self
        }
    }

    impl IntoListOrVariable for DataValue {
        fn into_list_or_variable(self) -> ListOrVariable {
            match self {
                DataValue::List(items) => ListOrVariable::List(items),
                _ => ListOrVariable::Variable(self),
            }
        }
    }

    impl IntoListOrVariable for Value {
        fn into_list_or_variable(self) -> ListOrVariable {
            self.into_data_value().into_list_or_variable()
        }
    }
}
