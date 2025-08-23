//! Macros for simplified WOQL query construction
//! 
//! These macros provide a more ergonomic way to construct WOQL queries
//! as an alternative to the woql-builder approach.

/// Create a Variable value
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
/// let person = node_value!("Person"); // Creates NodeValue::Node("Person".to_string())
/// ```
#[macro_export]
macro_rules! node_value {
    ($uri:expr) => {
        $crate::value::NodeValue::Node($uri.to_string())
    };
}

/// Create a Data value with automatic type conversion
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
/// let dt = datetime!("2024-01-01T00:00:00Z"); // Creates Value::Data(XsdAnySimpleType::DateTime)
/// let dt2 = datetime!("2024-12-31T23:59:59Z");
/// ```
#[macro_export]
macro_rules! datetime {
    ($value:expr) => {
        $crate::value::Value::Data(terminusdb_schema::XSDAnySimpleType::DateTime(
            chrono::DateTime::parse_from_rfc3339($value)
                .expect("Invalid datetime format, expected RFC3339 (e.g., '2024-01-01T00:00:00Z')")
                .with_timezone(&chrono::Utc)
        ))
    };
}

/// Create a List value
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
/// let q = not!(triple!(var!(x), "archived", data!(true)));
/// ```
#[macro_export]
macro_rules! not {
    ($query:expr) => {
        $crate::query::Query::Not($crate::query::Not {
            query: Box::new($query)
        })
    };
}

/// Create a Select query
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
/// let q = select![x, name], and!(
///     triple!(var!(x), "rdf:type", "Person"),
///     triple!(var!(x), "name", var!(name))
/// );
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
/// let q = type!(var!(x), "Person"); // equivalent to triple!(var!(x), "rdf:type", "Person")
/// let q2 = type!(var!(x), var!(type)); // with variable type
/// ```
#[macro_export]
macro_rules! type_ {
    ($subject:expr, $type:expr) => {
        triple!($subject, "rdf:type", $type)
    };
}

/// Shortcut for isa type checking
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
/// let q = isa!(var!(x), "Person"); // checks if x is of type Person
/// ```
#[macro_export]
macro_rules! isa {
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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

/// Shortcut for typecast operations
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
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
/// # use woql2::macros::*;
/// let q = compare!((var!(date)) == (today!()));
/// let q2 = after!(var!(created), today!());
/// ```
#[macro_export]
macro_rules! today {
    () => {
        $crate::value::Value::Data(terminusdb_schema::XSDAnySimpleType::DateTime(chrono::Utc::now()))
    };
}

/// Check if a date is after another date
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
/// let q = after!(var!(end_date), data!("2024-01-01T00:00:00Z"));
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
/// # use woql2::macros::*;
/// let q = before!(var!(start_date), data!("2024-12-31T23:59:59Z"));
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
/// # use woql2::macros::*;
/// let q = in_between!(var!(date), data!("2024-01-01T00:00:00Z"), data!("2024-12-31T23:59:59Z"));
/// let q2 = in_between!(var!(event_date), today!(), data!("2025-01-01T00:00:00Z"));
/// ```
#[macro_export]
macro_rules! in_between {
    ($date:expr, $start:expr, $end:expr) => {
        and!(
            compare!(($date) >= ($start)),
            compare!(($date) <= ($end))
        )
    };
}

/// Check if today's date is between two dates (inclusive)
/// 
/// # Examples
/// ```
/// # use woql2::macros::*;
/// let q = today_in_between!(data!("2024-01-01T00:00:00Z"), data!("2024-12-31T23:59:59Z"));
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
/// # use woql2::macros::*;
/// let q = compare!(var!(age) > data!(18));      // Greater
/// let q2 = compare!(var!(age) < data!(65));     // Less
/// let q3 = compare!(var!(x) == var!(y));        // Equals
/// let q4 = compare!(var!(age) >= data!(18));    // Greater or equal (using Or)
/// let q5 = compare!(var!(age) <= data!(65));    // Less or equal (using Or)
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
        or!(
            greater!($left.clone(), $right.clone()),
            eq!($left, $right)
        )
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
        or!(
            less!($left.clone(), $right.clone()),
            eq!($left, $right)
        )
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

// Helper functions for conversion
pub use self::conversion::*;

mod conversion {
    use crate::value::{Value, NodeValue, DataValue, ListOrVariable};
    use crate::expression::ArithmeticValue;
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
            XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self))
        }
    }

    impl IntoXsdType for i64 {
        fn into_xsd_type(self) -> XSDAnySimpleType {
            XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self))
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
            Value::Data(XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self)))
        }
    }

    impl IntoValue for i64 {
        fn into_value(self) -> Value {
            Value::Data(XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self)))
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
                    let data_items: Vec<DataValue> = items.into_iter().map(|v| v.into_data_value()).collect();
                    DataValue::List(data_items)
                },
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
            DataValue::Data(XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self)))
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
            DataValue::Data(XSDAnySimpleType::Decimal(decimal_rs::Decimal::from(self)))
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