#![allow(clippy::redundant_clone)] // Allow clones in tests for clarity
                                   // Import items from the crate root
use crate::prelude::*;

// Import expression helpers
use crate::expression::{div, divide, exp, floor, minus, plus, times};

// Import woql2/schema types needed for assertions
use terminusdb_schema::XSDAnySimpleType;
// Only import Query types used in assertions
use terminusdb_woql2::prelude::{
    DataValue,
    NodeValue,
    Query as Woql2Query,
    // Import types for new tests
    Value as Woql2Value,
};
// Import specific woql2 Path types for assertions
use terminusdb_woql2::path::{
    InversePathPredicate as Woql2InversePathPredicate, PathOr as Woql2PathOr,
    PathPattern as Woql2PathPattern, PathPlus as Woql2PathPlus,
    PathPredicate as Woql2PathPredicate, PathSequence, PathStar as Woql2PathStar,
    PathTimes as Woql2PathTimes,
};
use terminusdb_woql2::query::Path as Woql2Path;

// Import the actual struct definitions needed for matching
use terminusdb_woql2::prelude::{
    // Import AddTriple & DeleteTriple
    AddTriple as Woql2AddTriple,
    And,
    ArithmeticExpression as Woql2ArithmeticExpression,
    ArithmeticValue as Woql2ArithmeticValue,
    Concatenate,
    Count as Woql2Count,
    DeleteDocument as Woql2DeleteDocument,
    DeleteTriple as Woql2DeleteTriple,
    Div as Woql2Div,
    Divide as Woql2Divide,
    Equals as Woql2Equals,
    // Import Eval and Arithmetic types
    Eval as Woql2Eval,
    Exp as Woql2Exp,
    Floor as Woql2Floor,
    From as Woql2From,
    Greater as Woql2Greater,
    // Import Aggregation/Grouping Structs
    GroupBy as Woql2GroupBy,
    If as Woql2If,
    Immediately as Woql2Immediately,
    InsertDocument as Woql2InsertDocument,
    Into as Woql2Into,
    IsA as Woql2IsA,
    Join,
    Length as Woql2Length,
    Less as Woql2Less,
    Like,
    Limit as Woql2Limit,
    Lower,
    Minus as Woql2Minus,
    Not as Woql2Not,
    // Import Once & Immediately
    Once as Woql2Once,
    Or as Woql2Or,
    Order as Woql2Order,
    // Import Ordering Structs
    OrderBy as Woql2OrderBy,
    OrderTemplate as Woql2OrderTemplate,
    Pad,
    Plus as Woql2Plus,
    // Import Document Ops Structs
    ReadDocument as Woql2ReadDocument,
    Regexp,
    Select as Woql2Select,
    Split,
    Start as Woql2Start,
    Substring,
    Subsumption as Woql2Subsumption,
    Sum as Woql2Sum,
    Times as Woql2Times,
    Trim,
    Triple as Woql2Triple,
    True as Woql2True,
    TypeOf as Woql2TypeOf,
    Typecast as Woql2Typecast,
    UpdateDocument as Woql2UpdateDocument,
    Upper,
    Using as Woql2Using,
    WoqlOptional,
};

use terminusdb_schema::{GraphType, ToTDBInstance};

// Helper to extract queries from And for testing
fn assert_and_contains(query: Woql2Query, expected_len: usize) -> Vec<Woql2Query> {
    match query {
        Woql2Query::And(and_query) => {
            assert_eq!(and_query.and.len(), expected_len);
            and_query.and
        }
        _ => panic!("Expected And query, found {:?}", query),
    }
}

#[test]
fn test_single_triple() {
    let builder = WoqlBuilder::new();
    let query = builder.triple("doc:Subj", "prop:pred", "v:Obj").finalize();

    // A single triple should not be wrapped in And
    match query {
        Woql2Query::Triple(ref t) => {
            assert!(matches!(&t.subject, NodeValue::Node(iri) if iri == "doc:Subj"));
            assert!(matches!(&t.predicate, NodeValue::Node(iri) if iri == "prop:pred"));
            assert!(matches!(&t.object, Woql2Value::Variable(var_name) if var_name == "Obj"));
        }
        _ => panic!("Expected Triple query"),
    }
}

#[test]
fn test_triple_with_var_and_literal() {
    let builder = WoqlBuilder::new();
    let subj_var = Var::new("Subj");
    let query = builder
        .triple(subj_var.clone(), "prop:pred", string_literal("ObjectValue"))
        .finalize();

    match query {
        Woql2Query::Triple(ref t) => {
            assert!(matches!(&t.subject, NodeValue::Variable(var_name) if var_name == "Subj"));
            assert!(matches!(&t.predicate, NodeValue::Node(iri) if iri == "prop:pred"));
            assert!(
                matches!(&t.object, Woql2Value::Data(XSDAnySimpleType::String(s)) if s == "ObjectValue")
            );
        }
        _ => panic!("Expected Triple query"),
    }
}

#[test]
fn test_triple_with_integer_literal() {
    let builder = WoqlBuilder::new();
    let query = builder.triple("doc:Item", "prop:count", 42u64).finalize();

    match query {
        Woql2Query::Triple(ref t) => {
            assert!(matches!(&t.subject, NodeValue::Node(iri) if iri == "doc:Item"));
            assert!(matches!(&t.predicate, NodeValue::Node(iri) if iri == "prop:count"));
            assert!(
                matches!(&t.object, Woql2Value::Data(XSDAnySimpleType::UnsignedInt(val)) if *val == 42usize)
            );
        }
        _ => panic!("Expected Triple query"),
    }
}

#[test]
fn test_multiple_triples_implicit_and_flattened() {
    let builder = WoqlBuilder::new();
    let query = builder
        .triple("a", "b", "c")
        .triple("d", "e", "f")
        .triple("g", "h", "i")
        .finalize();

    // Expecting And(T1, T2, T3)
    let queries_in_and = assert_and_contains(query, 3);
    assert!(matches!(queries_in_and[0], Woql2Query::Triple(_)));
    assert!(matches!(queries_in_and[1], Woql2Query::Triple(_)));
    assert!(matches!(queries_in_and[2], Woql2Query::Triple(_)));
}

#[test]
fn test_explicit_and() {
    let b1 = WoqlBuilder::new().triple("a", "b", "c");
    let b2 = WoqlBuilder::new().triple("d", "e", "f");
    let b3 = WoqlBuilder::new().triple("g", "h", "i");

    let final_query = b1.and(vec![b2, b3]).finalize();
    let queries_in_and = assert_and_contains(final_query, 3);
    assert!(matches!(queries_in_and[0], Woql2Query::Triple(_))); // From b1
    assert!(matches!(queries_in_and[1], Woql2Query::Triple(_))); // From b2
    assert!(matches!(queries_in_and[2], Woql2Query::Triple(_))); // From b3
}

#[test]
fn test_explicit_or() {
    let b1 = WoqlBuilder::new().triple("a", "b", "c");
    let b2 = WoqlBuilder::new().triple("d", "e", "f");

    let final_query = b1.or(vec![b2]).finalize();
    match final_query {
        Woql2Query::Or(or_query) => {
            assert_eq!(or_query.or.len(), 2);
            assert!(matches!(or_query.or[0], Woql2Query::Triple(_))); // From b1
            assert!(matches!(or_query.or[1], Woql2Query::Triple(_))); // From b2
        }
        _ => panic!("Expected Or query, found {:?}", final_query),
    }
}

#[test]
fn test_not() {
    let builder = WoqlBuilder::new().triple("a", "b", "c");
    let final_query = builder.not().finalize();

    match final_query {
        Woql2Query::Not(not_query) => {
            // Check the inner query is the triple
            assert!(matches!(*not_query.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Not query, found {:?}", final_query),
    }
}

#[test]
fn test_complex_logical() {
    let (a, b, c, d, e, f, g, h, i) = vars!("a", "b", "c", "d", "e", "f", "g", "h", "i");

    // (Triple(a,b,c) AND Triple(d,e,f)) OR NOT Triple(g,h,i)

    let and_part = WoqlBuilder::new()
        .triple(a.clone(), b.clone(), c.clone())
        .triple(d.clone(), e.clone(), f.clone()); // Implicit And

    let not_part = WoqlBuilder::new()
        .triple(g.clone(), h.clone(), i.clone())
        .not();

    let final_query = and_part.or(vec![not_part]).finalize();

    match final_query {
        Woql2Query::Or(or_query) => {
            assert_eq!(or_query.or.len(), 2);
            // Check first part is And(T1, T2)
            let _ = assert_and_contains(or_query.or[0].clone(), 2);
            // Check second part is Not(T3)
            match &or_query.or[1] {
                Woql2Query::Not(not_query) => {
                    assert!(matches!(*not_query.query, Woql2Query::Triple(_)));
                }
                _ => panic!("Expected Not query in Or[1]"),
            }
        }
        _ => panic!("Expected Or query, found {:?}", final_query),
    }
}

#[test]
#[should_panic] // Expect panic because a literal cannot be a subject
fn test_triple_literal_subject_panics() {
    let _query = WoqlBuilder::new()
        .triple(string_literal("InvalidSubject"), "prop:pred", "v:Obj")
        .finalize();
}

#[test]
#[should_panic] // Expect panic because a literal cannot be a predicate
fn test_triple_literal_predicate_panics() {
    let _query = WoqlBuilder::new()
        .triple("doc:Subj", string_literal("InvalidPredicate"), "v:Obj")
        .finalize();
}

// Test explicit node usage
#[test]
fn test_explicit_node_function() {
    let builder = WoqlBuilder::new();
    let query = builder
        .triple(
            node("doc:ExplicitNode"),
            "prop:explicit",
            node("v:ExplicitVar"),
        )
        .finalize();
    match query {
        Woql2Query::Triple(ref t) => {
            assert!(matches!(&t.subject, NodeValue::Node(iri) if iri == "doc:ExplicitNode"));
            assert!(
                matches!(&t.object, Woql2Value::Variable(var_name) if var_name == "ExplicitVar")
            );
        }
        _ => panic!("Expected Triple query"),
    }
}

#[test]
fn test_limit() {
    let builder = WoqlBuilder::new().triple("a", "b", "c").limit(10);
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Limit(limit_query) => {
            assert_eq!(limit_query.limit, 10);
            // Check the inner query is the triple
            assert!(matches!(*limit_query.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Limit query, found {:?}", final_query),
    }
}

#[test]
fn test_start() {
    let builder = WoqlBuilder::new().triple("a", "b", "c").start(5);
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Start(start_query) => {
            assert_eq!(start_query.start, 5);
            assert!(matches!(*start_query.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Start query, found {:?}", final_query),
    }
}

#[test]
fn test_select() {
    let (a, b, c) = vars!("A", "B", "C");
    let builder = WoqlBuilder::new()
        .triple(a.clone(), "pred", b.clone())
        .select(vec![a.clone(), c.clone()]); // Select A and C
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Select(select_query) => {
            assert_eq!(
                select_query.variables,
                vec!["A".to_string(), "C".to_string()]
            );
            // Check the inner query is the triple
            match *select_query.query {
                Woql2Query::Triple(ref t) => {
                    assert!(matches!(&t.subject, NodeValue::Variable(v) if v == "A"));
                }
                _ => panic!("Expected inner query to be Triple"),
            }
        }
        _ => panic!("Expected Select query, found {:?}", final_query),
    }
}

#[test]
fn test_limit_start_select_chain() {
    let (a, b) = vars!("A", "B");
    let builder = WoqlBuilder::new()
        .triple(a.clone(), "pred", b.clone())
        .limit(10)
        .start(2)
        .select(vec![a.clone()]);
    let final_query = builder.finalize();

    // Expect Select(Start(Limit(Triple))) order
    match final_query {
        Woql2Query::Select(select_q) => {
            assert_eq!(select_q.variables, vec!["A".to_string()]);
            match *select_q.query {
                Woql2Query::Start(start_q) => {
                    assert_eq!(start_q.start, 2);
                    match *start_q.query {
                        Woql2Query::Limit(limit_q) => {
                            assert_eq!(limit_q.limit, 10);
                            assert!(matches!(*limit_q.query, Woql2Query::Triple(_)));
                        }
                        _ => panic!("Expected Limit query"),
                    }
                }
                _ => panic!("Expected Start query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_eq_literals() {
    let builder = WoqlBuilder::new().eq(string_literal("hello"), string_literal("hello"));
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Equals(ref eq_q) => {
            assert!(
                matches!(&eq_q.left, Woql2Value::Data(XSDAnySimpleType::String(s)) if s == "hello")
            );
            assert!(
                matches!(&eq_q.right, Woql2Value::Data(XSDAnySimpleType::String(s)) if s == "hello")
            );
        }
        _ => panic!("Expected Equals query"),
    }
}

#[test]
fn test_less_variables() {
    let (a, b) = vars!("A", "B");
    let builder = WoqlBuilder::new().less(a.clone(), b.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Less(ref less_q) => {
            assert!(matches!(&less_q.left, DataValue::Variable(v) if v == "A"));
            assert!(matches!(&less_q.right, DataValue::Variable(v) if v == "B"));
        }
        _ => panic!("Expected Less query"),
    }
}

#[test]
fn test_greater_mixed() {
    let a = vars!("A");
    let builder = WoqlBuilder::new().greater(a.clone(), 100u64);
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Greater(ref greater_q) => {
            assert!(matches!(&greater_q.left, DataValue::Variable(v) if v == "A"));
            assert!(
                matches!(&greater_q.right, DataValue::Data(XSDAnySimpleType::UnsignedInt(val)) if *val == 100)
            );
        }
        _ => panic!("Expected Greater query"),
    }
}

#[test]
fn test_triple_and_eq() {
    let v = vars!("Val");
    let builder = WoqlBuilder::new()
        .triple("doc:subj", "prop:val", v.clone())
        .eq(v.clone(), string_literal("target"));
    let final_query = builder.finalize();

    let queries = assert_and_contains(final_query, 2);
    assert!(matches!(queries[0], Woql2Query::Triple(_)));
    match &queries[1] {
        Woql2Query::Equals(ref eq_q) => {
            assert!(matches!(&eq_q.left, Woql2Value::Variable(v_name) if v_name == "Val"));
            assert!(
                matches!(&eq_q.right, Woql2Value::Data(XSDAnySimpleType::String(s)) if s == "target")
            );
        }
        _ => panic!("Expected Equals query in And[1]"),
    }
}

#[test]
#[should_panic] // Expect panic because eq expects DataValue, but node() produces NodeValue
fn test_eq_node_panics() {
    let _ = WoqlBuilder::new()
        .eq(node("doc:a"), node("doc:b"))
        .finalize();
}

#[test]
fn test_opt() {
    let builder = WoqlBuilder::new().triple("a", "b", "c").opt();
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::WoqlOptional(opt_q) => {
            assert!(matches!(*opt_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Optional query"),
    }
}

#[test]
fn test_when() {
    let test_q = WoqlBuilder::new().eq(vars!("A"), 10u64);
    let then_q = WoqlBuilder::new().triple("doc", "isa", "TypeA");

    let final_query = WoqlBuilder::when(test_q, then_q).finalize();

    match final_query {
        Woql2Query::If(if_q) => {
            // Check test part
            assert!(matches!(*if_q.test, Woql2Query::Equals(_)));
            // Check then part
            assert!(matches!(*if_q.then_query, Woql2Query::Triple(_)));
            // Check else part is True
            assert!(matches!(*if_q.else_query, Woql2Query::True(_)));
        }
        _ => panic!("Expected If query"),
    }
}

#[test]
fn test_if_then_else() {
    let test_q = WoqlBuilder::new().eq(vars!("A"), 10u64);
    let then_q = WoqlBuilder::new().triple("doc", "isa", "TypeA");
    let else_q = WoqlBuilder::new().triple("doc", "isa", "TypeB");

    let final_query = WoqlBuilder::if_then_else(test_q, then_q, else_q).finalize();

    match final_query {
        Woql2Query::If(if_q) => {
            assert!(matches!(*if_q.test, Woql2Query::Equals(_)));
            assert!(matches!(*if_q.then_query, Woql2Query::Triple(_)));
            assert!(matches!(*if_q.else_query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected If query"),
    }
}

#[test]
fn test_isa() {
    // Prefix unused type_var with underscore
    let (element_var, _type_var) = vars!("Element", "Type");
    let builder = WoqlBuilder::new().isa(element_var.clone(), "scm:MyClass");
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::IsA(isa_q) => {
            assert!(matches!(isa_q.element, NodeValue::Variable(v) if v == "Element"));
            assert!(matches!(isa_q.type_of, NodeValue::Node(n) if n == "scm:MyClass"));
        }
        _ => panic!("Expected IsA query"),
    }
}

#[test]
fn test_subsumption() {
    let builder = WoqlBuilder::new().subsumption("scm:Child", "scm:Parent");
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Subsumption(sub_q) => {
            assert!(matches!(sub_q.child, NodeValue::Node(n) if n == "scm:Child"));
            assert!(matches!(sub_q.parent, NodeValue::Node(n) if n == "scm:Parent"));
        }
        _ => panic!("Expected Subsumption query"),
    }
}

#[test]
fn test_type_of() {
    let (val_var, type_var) = vars!("Value", "ValueType");
    let builder = WoqlBuilder::new().type_of(val_var.clone(), type_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::TypeOf(typeof_q) => {
            assert!(matches!(typeof_q.value, Woql2Value::Variable(v) if v == "Value"));
            assert!(matches!(typeof_q.type_uri, NodeValue::Variable(v) if v == "ValueType"));
        }
        _ => panic!("Expected TypeOf query"),
    }
}

#[test]
fn test_typecast() {
    let (input_var, result_var) = vars!("Input", "Result");
    let builder = WoqlBuilder::new().typecast(input_var.clone(), "xsd:integer", result_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Typecast(cast_q) => {
            assert!(matches!(cast_q.value, Woql2Value::Variable(v) if v == "Input"));
            assert!(matches!(cast_q.type_uri, NodeValue::Node(n) if n == "xsd:integer"));
            assert!(matches!(cast_q.result_value, Woql2Value::Variable(v) if v == "Result"));
        }
        _ => panic!("Expected Typecast query"),
    }
}

#[test]
fn test_using() {
    let builder = WoqlBuilder::new()
        .triple("a", "b", "c")
        .using("my_collection");
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Using(using_q) => {
            assert_eq!(using_q.collection, "my_collection");
            assert!(matches!(*using_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Using query"),
    }
}

#[test]
fn test_from() {
    let builder = WoqlBuilder::new()
        .triple("a", "b", "c")
        .from("graph/schema");
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::From(from_q) => {
            assert_eq!(from_q.graph, "graph/schema");
            assert!(matches!(*from_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected From query"),
    }
}

#[test]
fn test_into() {
    let builder = WoqlBuilder::new()
        .triple("a", "b", "c") // Assume this is AddTriple later
        .into("graph/instance");
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Into(into_q) => {
            assert_eq!(into_q.graph, "graph/instance");
            // Later, inner query would be AddTriple if we added that method
            assert!(matches!(*into_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Into query"),
    }
}

#[test]
fn test_using_from_into_chain() {
    let builder = WoqlBuilder::new()
        .triple("a", "b", "c")
        .using("my_db/my_repo")
        .from("schema")
        .into("instance");
    let final_query = builder.finalize();

    // Expect Into(From(Using(Triple)))
    match final_query {
        Woql2Query::Into(into_q) => {
            assert_eq!(into_q.graph, "instance");
            match *into_q.query {
                Woql2Query::From(from_q) => {
                    assert_eq!(from_q.graph, "schema");
                    match *from_q.query {
                        Woql2Query::Using(using_q) => {
                            assert_eq!(using_q.collection, "my_db/my_repo");
                            assert!(matches!(*using_q.query, Woql2Query::Triple(_)));
                        }
                        _ => panic!("Expected Using query"),
                    }
                }
                _ => panic!("Expected From query"),
            }
        }
        _ => panic!("Expected Into query"),
    }
}

// --- String Operation Tests ---

#[test]
fn test_trim() {
    let (untrimmed_var, trimmed_var) = vars!("Input", "Output");
    let builder = WoqlBuilder::new().trim(untrimmed_var.clone(), trimmed_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Trim(trim_q) => {
            assert!(matches!(trim_q.untrimmed, DataValue::Variable(v) if v == "Input"));
            assert!(matches!(trim_q.trimmed, DataValue::Variable(v) if v == "Output"));
        }
        _ => panic!("Expected Trim query, found {:?}", final_query),
    }
}

#[test]
fn test_lower() {
    let (mixed_var, lower_var) = vars!("Mixed", "Lower");
    let builder = WoqlBuilder::new().lower(mixed_var.clone(), lower_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Lower(lower_q) => {
            assert!(matches!(lower_q.mixed, DataValue::Variable(v) if v == "Mixed"));
            assert!(matches!(lower_q.lower, DataValue::Variable(v) if v == "Lower"));
        }
        _ => panic!("Expected Lower query, found {:?}", final_query),
    }
}

#[test]
fn test_upper() {
    let (mixed_var, upper_var) = vars!("Mixed", "Upper");
    let builder = WoqlBuilder::new().upper(mixed_var.clone(), upper_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Upper(upper_q) => {
            assert!(matches!(upper_q.mixed, DataValue::Variable(v) if v == "Mixed"));
            assert!(matches!(upper_q.upper, DataValue::Variable(v) if v == "Upper"));
        }
        _ => panic!("Expected Upper query, found {:?}", final_query),
    }
}

#[test]
fn test_pad() {
    let (str_var, char_var, times_var, result_var) = vars!("S", "C", "T", "R");
    let builder = WoqlBuilder::new().pad(
        str_var.clone(),
        char_var.clone(),
        times_var.clone(),
        result_var.clone(),
    );
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Pad(pad_q) => {
            assert!(matches!(pad_q.string, DataValue::Variable(v) if v == "S"));
            assert!(matches!(pad_q.char, DataValue::Variable(v) if v == "C"));
            assert!(matches!(pad_q.times, DataValue::Variable(v) if v == "T"));
            assert!(matches!(pad_q.result_string, DataValue::Variable(v) if v == "R"));
        }
        _ => panic!("Expected Pad query, found {:?}", final_query),
    }
}

#[test]
fn test_split() {
    let (str_var, pattern_var, list_var) = vars!("Input", "Pattern", "OutputList");
    let builder = WoqlBuilder::new().split(str_var.clone(), pattern_var.clone(), list_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Split(split_q) => {
            assert!(matches!(split_q.string, DataValue::Variable(v) if v == "Input"));
            assert!(matches!(split_q.pattern, DataValue::Variable(v) if v == "Pattern"));
            assert!(matches!(split_q.list, DataValue::Variable(v) if v == "OutputList"));
        }
        _ => panic!("Expected Split query, found {:?}", final_query),
    }
}

#[test]
fn test_join() {
    let (list_var, sep_var, result_var) = vars!("InputList", "Separator", "Result");
    let builder = WoqlBuilder::new().join(list_var.clone(), sep_var.clone(), result_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Join(join_q) => {
            assert!(matches!(join_q.list, DataValue::Variable(v) if v == "InputList"));
            assert!(matches!(join_q.separator, DataValue::Variable(v) if v == "Separator"));
            assert!(matches!(join_q.result_string, DataValue::Variable(v) if v == "Result"));
        }
        _ => panic!("Expected Join query, found {:?}", final_query),
    }
}

#[test]
fn test_concat() {
    let (list_var, result_var) = vars!("InputList", "Result");
    let builder = WoqlBuilder::new().concat(list_var.clone(), result_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Concatenate(concat_q) => {
            assert!(matches!(concat_q.list, DataValue::Variable(v) if v == "InputList"));
            assert!(matches!(concat_q.result_string, DataValue::Variable(v) if v == "Result"));
        }
        _ => panic!("Expected Concatenate query, found {:?}", final_query),
    }
}

#[test]
fn test_concatenate_alias() {
    let (list_var, result_var) = vars!("InputList", "Result");
    // Use the `concatenate` alias directly
    let builder = WoqlBuilder::new().concatenate(list_var.clone(), result_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Concatenate(concat_q) => {
            assert!(matches!(concat_q.list, DataValue::Variable(v) if v == "InputList"));
            assert!(matches!(concat_q.result_string, DataValue::Variable(v) if v == "Result"));
        }
        _ => panic!(
            "Expected Concatenate query from alias, found {:?}",
            final_query
        ),
    }
}

#[test]
fn test_substring() {
    let (str_var, before_var, len_var, after_var, sub_var) = vars!("S", "B", "L", "A", "Sub");
    let builder = WoqlBuilder::new().substring(
        str_var.clone(),
        before_var.clone(),
        len_var.clone(),
        after_var.clone(),
        sub_var.clone(),
    );
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Substring(sub_q) => {
            assert!(matches!(sub_q.string, DataValue::Variable(v) if v == "S"));
            assert!(matches!(sub_q.before, DataValue::Variable(v) if v == "B"));
            assert!(matches!(sub_q.length, DataValue::Variable(v) if v == "L"));
            assert!(matches!(sub_q.after, DataValue::Variable(v) if v == "A"));
            assert!(matches!(sub_q.substring, DataValue::Variable(v) if v == "Sub"));
        }
        _ => panic!("Expected Substring query, found {:?}", final_query),
    }
}

#[test]
fn test_regexp() {
    let (pattern_var, str_var, result_var) = vars!("Pattern", "Input", "Matches");
    let builder = WoqlBuilder::new().regexp(
        pattern_var.clone(),
        str_var.clone(),
        Some(result_var.clone()), // Test with result binding
    );
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Regexp(re_q) => {
            assert!(matches!(re_q.pattern, DataValue::Variable(v) if v == "Pattern"));
            assert!(matches!(re_q.string, DataValue::Variable(v) if v == "Input"));
            assert!(matches!(re_q.result, Some(DataValue::Variable(v)) if v == "Matches"));
        }
        _ => panic!("Expected Regexp query, found {:?}", final_query),
    }
}

#[test]
fn test_regexp_no_result() {
    let (pattern_var, str_var) = vars!("Pattern", "Input");
    let builder = WoqlBuilder::new().regexp(
        pattern_var.clone(),
        str_var.clone(),
        None::<Var>, // Test without result binding
    );
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Regexp(re_q) => {
            assert!(matches!(re_q.pattern, DataValue::Variable(v) if v == "Pattern"));
            assert!(matches!(re_q.string, DataValue::Variable(v) if v == "Input"));
            assert!(re_q.result.is_none());
        }
        _ => panic!(
            "Expected Regexp query without result, found {:?}",
            final_query
        ),
    }
}

#[test]
fn test_like() {
    let (left_var, right_var, sim_var) = vars!("L", "R", "Sim");
    let builder = WoqlBuilder::new().like(left_var.clone(), right_var.clone(), sim_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Like(like_q) => {
            assert!(matches!(like_q.left, DataValue::Variable(v) if v == "L"));
            assert!(matches!(like_q.right, DataValue::Variable(v) if v == "R"));
            assert!(matches!(like_q.similarity, DataValue::Variable(v) if v == "Sim"));
        }
        _ => panic!("Expected Like query, found {:?}", final_query),
    }
}

// --- Document Operation Tests ---

#[test]
fn test_read_document() {
    let (id_var, doc_var) = vars!("DocIRI", "TheDocument");
    let builder = WoqlBuilder::new().read_document(id_var.clone(), doc_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::ReadDocument(read_q) => {
            assert!(matches!(read_q.identifier, NodeValue::Variable(v) if v == "DocIRI"));
            assert!(matches!(read_q.document, Woql2Value::Variable(v) if v == "TheDocument"));
        }
        _ => panic!("Expected ReadDocument query, found {:?}", final_query),
    }
}

#[test]
fn test_insert_document() {
    let (doc_val_var, new_id_var) = vars!("InputDoc", "NewID");
    let builder = WoqlBuilder::new().insert_document(doc_val_var.clone(), Some(new_id_var.clone()));
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::InsertDocument(insert_q) => {
            assert!(matches!(insert_q.document, Woql2Value::Variable(v) if v == "InputDoc"));
            assert!(matches!(insert_q.identifier, Some(NodeValue::Variable(v)) if v == "NewID"));
        }
        _ => panic!("Expected InsertDocument query, found {:?}", final_query),
    }
}

#[test]
fn test_insert_document_no_id() {
    let doc_val_var = vars!("InputDoc");
    // Test without binding the new ID
    let builder = WoqlBuilder::new().insert_document(doc_val_var.clone(), None::<Var>);
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::InsertDocument(insert_q) => {
            assert!(matches!(insert_q.document, Woql2Value::Variable(v) if v == "InputDoc"));
            assert!(insert_q.identifier.is_none());
        }
        _ => panic!(
            "Expected InsertDocument query without ID, found {:?}",
            final_query
        ),
    }
}

#[test]
fn test_update_document() {
    let (doc_val_var, updated_id_var) = vars!("UpdatedDoc", "UpdatedID");
    let builder =
        WoqlBuilder::new().update_document(doc_val_var.clone(), Some(updated_id_var.clone()));
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::UpdateDocument(update_q) => {
            assert!(matches!(update_q.document, Woql2Value::Variable(v) if v == "UpdatedDoc"));
            assert!(
                matches!(update_q.identifier, Some(NodeValue::Variable(v)) if v == "UpdatedID")
            );
        }
        _ => panic!("Expected UpdateDocument query, found {:?}", final_query),
    }
}

#[test]
fn test_delete_document() {
    let id_var = vars!("DocToDeleteIRI");
    let builder = WoqlBuilder::new().delete_document(id_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::DeleteDocument(delete_q) => {
            assert!(matches!(delete_q.identifier, NodeValue::Variable(v) if v == "DocToDeleteIRI"));
        }
        _ => panic!("Expected DeleteDocument query, found {:?}", final_query),
    }
}

// --- Control Flow Tests ---

#[test]
fn test_once() {
    let builder = WoqlBuilder::new().triple("a", "b", "c").once();
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Once(once_q) => {
            assert!(matches!(*once_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Once query, found {:?}", final_query),
    }
}

#[test]
fn test_immediately() {
    let builder = WoqlBuilder::new().triple("a", "b", "c").immediately();
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Immediately(imm_q) => {
            assert!(matches!(*imm_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Immediately query, found {:?}", final_query),
    }
}

// --- Triple & Data Manipulation Tests ---

#[test]
fn test_add_triple() {
    let builder = WoqlBuilder::new().add_triple("doc:subj", "prop:pred", 123);
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::AddTriple(add_q) => {
            assert!(matches!(add_q.subject, NodeValue::Node(n) if n == "doc:subj"));
            assert!(matches!(add_q.predicate, NodeValue::Node(n) if n == "prop:pred"));
            assert!(
                matches!(add_q.object, Woql2Value::Data(XSDAnySimpleType::UnsignedInt(i)) if i == 123)
            );
            assert_eq!(add_q.graph, GraphType::Instance);
        }
        _ => panic!("Expected AddTriple query, found {:?}", final_query),
    }
}

#[test]
fn test_add_triple_chaining() {
    let builder = WoqlBuilder::new()
        .add_triple("a", "b", "c")
        .add_triple("d", "e", "f");
    let final_query = builder.finalize();
    let queries = assert_and_contains(final_query, 2);
    assert!(matches!(queries[0], Woql2Query::AddTriple(_)));
    assert!(matches!(queries[1], Woql2Query::AddTriple(_)));
}

#[test]
fn test_delete_triple() {
    let builder = WoqlBuilder::new().delete_triple(vars!("S"), vars!("P"), vars!("O"));
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::DeleteTriple(del_q) => {
            assert!(matches!(del_q.subject, NodeValue::Variable(v) if v == "S"));
            assert!(matches!(del_q.predicate, NodeValue::Variable(v) if v == "P"));
            assert!(matches!(del_q.object, Woql2Value::Variable(v) if v == "O"));
            assert_eq!(del_q.graph, GraphType::Instance);
        }
        _ => panic!("Expected DeleteTriple query, found {:?}", final_query),
    }
}

#[test]
fn test_delete_triple_chaining() {
    let builder = WoqlBuilder::new()
        .delete_triple("a", "b", "c")
        .delete_triple("d", "e", "f");
    let final_query = builder.finalize();
    let queries = assert_and_contains(final_query, 2);
    assert!(matches!(queries[0], Woql2Query::DeleteTriple(_)));
    assert!(matches!(queries[1], Woql2Query::DeleteTriple(_)));
}

// --- Mathematical Operation Tests ---

#[test]
fn test_eval_simple() {
    let result_var = vars!("Result");
    let input_var = vars!("Input");
    // Eval(Plus(Value(Data(10)), Value(Variable("Input"))), Variable("Result"))
    let expr = plus(10u64, input_var.clone()); // Use From<u64> and From<Var>
    let builder = WoqlBuilder::new().eval(expr, result_var.clone()); // Use From<Var>
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Eval(eval_q) => {
            // Check result variable (should be Woql2ArithmeticValue::Variable)
            assert!(
                matches!(eval_q.result_value, Woql2ArithmeticValue::Variable(v) if v == "Result")
            );

            // Check expression structure (should be Woql2ArithmeticExpression::Plus)
            match eval_q.expression {
                Woql2ArithmeticExpression::Plus(plus_expr) => {
                    // Check left operand (should be Value(Data(UnsignedInt)))
                    match *plus_expr.left {
                        Woql2ArithmeticExpression::Value(Woql2ArithmeticValue::Data(data)) => {
                            assert!(matches!(data, XSDAnySimpleType::UnsignedInt(i) if i == 10));
                        }
                        _ => panic!("Expected left operand to be Value(Data(10))"),
                    }
                    // Check right operand (should be Value(Variable))
                    match *plus_expr.right {
                        Woql2ArithmeticExpression::Value(Woql2ArithmeticValue::Variable(v)) => {
                            assert_eq!(v, "Input");
                        }
                        _ => panic!("Expected right operand to be Value(Variable(\"Input\"))"),
                    }
                }
                _ => panic!("Expected expression to be Plus"),
            }
        }
        _ => panic!("Expected Eval query, found {:?}", final_query),
    }
}

#[test]
fn test_eval_nested() {
    let result_var = vars!("Result");
    let (a, b) = vars!("A", "B");
    // Eval(Times(Minus(Value(Var A), Value(Data 5)), Plus(Value(Data 2), Value(Var B))), Var Result)
    let expr = times(minus(a.clone(), 5u64), plus(2u64, b.clone()));
    let builder = WoqlBuilder::new().eval(expr, result_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Eval(eval_q) => {
            assert!(
                matches!(eval_q.result_value, Woql2ArithmeticValue::Variable(v) if v == "Result")
            );
            // Basic check for Times structure - deeper checks would be complex
            assert!(matches!(
                eval_q.expression,
                Woql2ArithmeticExpression::Times(_)
            ));
        }
        _ => panic!("Expected Eval query, found {:?}", final_query),
    }
}

#[test]
fn test_eval_floor() {
    let result_var = vars!("Result");
    let input_var = vars!("X");
    let expr = floor(divide(input_var.clone(), 2.5f64)); // Use From<Var> and From<f64>
    let builder = WoqlBuilder::new().eval(expr, result_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Eval(eval_q) => {
            assert!(
                matches!(eval_q.result_value, Woql2ArithmeticValue::Variable(v) if v == "Result")
            );
            // Check expression is Floor
            match eval_q.expression {
                Woql2ArithmeticExpression::Floor(floor_expr) => {
                    // Check inner expression is Divide
                    assert!(matches!(
                        *floor_expr.argument,
                        Woql2ArithmeticExpression::Divide(_)
                    ));
                }
                _ => panic!("Expected expression to be Floor"),
            }
        }
        _ => panic!("Expected Eval query, found {:?}", final_query),
    }
}

#[test]
fn test_eval_div_exp() {
    let result_var = vars!("Result");
    let (x, y, z) = vars!("X", "Y", "Z");
    // Eval(Div(Exp(Var X, Var Y), Var Z), Var Result)
    let expr = div(exp(x.clone(), y.clone()), z.clone());
    let builder = WoqlBuilder::new().eval(expr, result_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Eval(eval_q) => {
            assert!(
                matches!(eval_q.result_value, Woql2ArithmeticValue::Variable(v) if v == "Result")
            );
            assert!(matches!(
                eval_q.expression,
                Woql2ArithmeticExpression::Div(_)
            ));
        }
        _ => panic!("Expected Eval query, found {:?}", final_query),
    }
}

// --- Aggregation & Grouping Tests ---

#[test]
fn test_group_by() {
    let (name_var, age_var, grouped_list_var) = vars!("Name", "Age", "GroupedList");
    let template_var = vars!("Template"); // Assume template is a variable here

    // GroupBy(template=Var(Template), group_by=["Name"], grouped=Var(GroupedList), query=Triple(Var(Name), type, Person))
    let subquery = WoqlBuilder::new().isa(name_var.clone(), "Person");
    let builder = subquery.group_by(
        template_var.clone(),     // Template for output rows
        vec![name_var.clone()],   // Group by Name
        grouped_list_var.clone(), // Output variable
    );
    let final_query = builder.finalize(); // GroupBy returns a builder, finalize it

    match final_query {
        Woql2Query::GroupBy(gb_q) => {
            assert!(matches!(gb_q.template, Woql2Value::Variable(v) if v == "Template"));
            assert_eq!(gb_q.group_by, vec!["Name".to_string()]);
            assert!(matches!(gb_q.grouped_value, Woql2Value::Variable(v) if v == "GroupedList"));
            assert!(matches!(*gb_q.query, Woql2Query::IsA(_))); // Check inner query
        }
        _ => panic!("Expected GroupBy query, found {:?}", final_query),
    }
}

#[test]
fn test_count() {
    let count_var = vars!("TotalCount");
    let subquery = WoqlBuilder::new().triple("a", "b", "c");
    let builder = subquery.count(count_var.clone());
    let final_query = builder.finalize(); // Count returns a builder, finalize it

    match final_query {
        Woql2Query::Count(count_q) => {
            assert!(matches!(count_q.count, DataValue::Variable(v) if v == "TotalCount"));
            assert!(matches!(*count_q.query, Woql2Query::Triple(_))); // Check inner query
        }
        _ => panic!("Expected Count query, found {:?}", final_query),
    }
}

#[test]
fn test_sum() {
    let list_var = vars!("NumberList");
    let sum_var = vars!("SumResult");
    // Standalone Sum operation
    let builder = WoqlBuilder::sum(list_var.clone(), sum_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Sum(sum_q) => {
            assert!(matches!(sum_q.list, DataValue::Variable(v) if v == "NumberList"));
            assert!(matches!(sum_q.result, DataValue::Variable(v) if v == "SumResult"));
        }
        _ => panic!("Expected Sum query, found {:?}", final_query),
    }
}

#[test]
fn test_length() {
    let list_var = vars!("AnyList");
    let length_var = vars!("ListLength");
    // Standalone Length operation
    let builder = WoqlBuilder::length(list_var.clone(), length_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Length(len_q) => {
            assert!(matches!(len_q.list, DataValue::Variable(v) if v == "AnyList"));
            assert!(matches!(len_q.length, DataValue::Variable(v) if v == "ListLength"));
        }
        _ => panic!("Expected Length query, found {:?}", final_query),
    }
}

// --- Ordering Results Tests ---

#[test]
fn test_order_by() {
    let (name_var, age_var) = vars!("Name", "Age");
    let subquery = WoqlBuilder::new().triple(name_var.clone(), "age", age_var.clone());

    let builder = subquery.order_by(vec![
        (age_var.clone(), Woql2Order::Desc), // Order by Age descending
        (name_var.clone(), Woql2Order::Asc), // Then by Name ascending
    ]);
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::OrderBy(ob_q) => {
            assert_eq!(ob_q.ordering.len(), 2);
            // Check first ordering template
            assert_eq!(ob_q.ordering[0].variable, "Age");
            assert_eq!(ob_q.ordering[0].order, Woql2Order::Desc);
            // Check second ordering template
            assert_eq!(ob_q.ordering[1].variable, "Name");
            assert_eq!(ob_q.ordering[1].order, Woql2Order::Asc);
            // Check inner query
            assert!(matches!(*ob_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected OrderBy query, found {:?}", final_query),
    }
}

fn print_query_json(query: &Woql2Query) {
    println!("{}", serde_json::to_string_pretty(query).unwrap());
}

// Tests for Path Queries

#[test]
fn test_path_simple() {
    let (x, y, z) = vars!("x", "y", "z");
    // Use path_plus alias from prelude, wrap seq args in vec![]
    let path_pattern = seq(vec![pred("parent"), path_plus(pred("child"))]);
    // Call as associated function WoqlBuilder::path
    let query = WoqlBuilder::path(x, path_pattern, y, Some(z)).finalize();

    print_query_json(&query);

    match query {
        Woql2Query::Path(Woql2Path {
            subject,
            pattern,
            object,
            path, // Use correct field name 'path'
        }) => {
            // Match against Woql2Value variants
            assert!(matches!(subject, Woql2Value::Variable(v) if v == "x"));
            assert!(matches!(object, Woql2Value::Variable(v) if v == "y"));
            assert!(matches!(path, Some(Woql2Value::Variable(v)) if v == "z"));

            // Check the pattern structure using correct Woql2PathPattern variant names
            if let Woql2PathPattern::Sequence(PathSequence { sequence }) = pattern {
                assert_eq!(sequence.len(), 2);
                // First element: pred("parent")
                assert!(matches!(sequence[0], Woql2PathPattern::Predicate(_)));
                // Second element: plus(pred("child"))
                assert!(matches!(sequence[1], Woql2PathPattern::Plus(_)));
            } else {
                panic!("Expected Sequence, found {:?}", pattern);
            }
        }
        _ => panic!("Expected Path query, found {:?}", query),
    }
}

#[test]
fn test_path_complex_with_binding() {
    let (start_node, end_node, _edge_var, path_var) = vars!("start", "end", "edge", "p"); // edge_var unused for now

    // Use path_times alias, wrap seq/or args in vec![], remove .bind()
    let path_pattern = seq(vec![
        or(vec![
            // TODO: Revisit binding intermediate path segments.
            // The original test attempted `pred("friend").bind(edge_var.clone())` here.
            // Removed `.bind()` call as PathPattern doesn't support it directly.
            // Need to investigate how/if woql2 supports binding intermediate path nodes/edges.
            seq(vec![pred("friend"), pred("knows")]),
            inv("enemy"), // Pass the string directly to inv
        ]),
        path_times(pred("follows"), 2, 5),
    ]);

    // Call as associated function WoqlBuilder::path
    let query = WoqlBuilder::path(start_node, path_pattern, end_node, Some(path_var)).finalize();

    print_query_json(&query);

    match query {
        Woql2Query::Path(Woql2Path {
            subject,
            pattern,
            object,
            path, // Use correct field name 'path'
        }) => {
            // Match against Woql2Value variants
            assert!(matches!(subject, Woql2Value::Variable(v) if v == "start"));
            assert!(matches!(object, Woql2Value::Variable(v) if v == "end"));
            assert!(matches!(path, Some(Woql2Value::Variable(v)) if v == "p"));

            // Very basic check on the complex pattern structure using correct variant names
            if let Woql2PathPattern::Sequence(PathSequence { sequence }) = pattern {
                assert_eq!(sequence.len(), 2);
                assert!(matches!(sequence[0], Woql2PathPattern::Or(_)));
                assert!(matches!(sequence[1], Woql2PathPattern::Times(_)));
            } else {
                panic!("Expected top-level Sequence, found {:?}", pattern);
            }
        }
        _ => panic!("Expected Path query, found {:?}", query),
    }
}

#[test]
fn test_path_repetitions() {
    let (x, y, p) = vars!("x", "y", "p");

    // Path: (parent)+ followed by (child){2,4} followed by sibling*
    // Use path_plus, path_times, path_star aliases, wrap seq args in vec![]
    let path_pattern = seq(vec![
        path_plus(pred("parent")),
        path_times(pred("child"), 2, 4),
        path_star(pred("sibling")),
    ]);

    // Call as associated function WoqlBuilder::path
    let query = WoqlBuilder::path(x, path_pattern, y, Some(p)).finalize();

    print_query_json(&query);

    match query {
        Woql2Query::Path(Woql2Path {
            subject: _, // Ignore subject/object/path in this check
            pattern,
            object: _,
            path: _, // Use correct field name 'path' and ignore
        }) => {
            // Use correct Woql2PathPattern variant names
            if let Woql2PathPattern::Sequence(PathSequence { sequence }) = pattern {
                assert_eq!(sequence.len(), 3);
                assert!(matches!(sequence[0], Woql2PathPattern::Plus(_)));
                assert!(matches!(sequence[1], Woql2PathPattern::Times(_)));
                assert!(matches!(sequence[2], Woql2PathPattern::Star(_)));
            } else {
                panic!("Expected top-level Sequence, found {:?}", pattern);
            }
        }
        _ => panic!("Expected Path query, found {:?}", query),
    }
}

// --- Collection Operation Tests ---

#[test]
fn test_member() {
    let (element_var, list_var) = vars!("Element", "List");
    let builder = WoqlBuilder::new().member(element_var.clone(), list_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Member(member_q) => {
            assert!(matches!(member_q.member, DataValue::Variable(v) if v == "Element"));
            assert!(matches!(member_q.list, DataValue::Variable(v) if v == "List"));
        }
        _ => panic!("Expected Member query, found {:?}", final_query),
    }
}

#[test]
fn test_dot() {
    let (doc_var, field_literal, value_var) =
        (vars!("Doc"), string_literal("field_name"), vars!("Value"));
    let builder = WoqlBuilder::new().dot(doc_var.clone(), field_literal.clone(), value_var.clone());
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::Dot(dot_q) => {
            assert!(matches!(dot_q.document, DataValue::Variable(v) if v == "Doc"));
            assert!(
                matches!(dot_q.field, DataValue::Data(XSDAnySimpleType::String(s)) if s == "field_name")
            );
            assert!(matches!(dot_q.value, DataValue::Variable(v) if v == "Value"));
        }
        _ => panic!("Expected Dot query, found {:?}", final_query),
    }
}

// --- Miscellaneous Operation Tests ---

#[test]
fn test_distinct() {
    let (a, b) = vars!("A", "B");
    let builder = WoqlBuilder::new()
        .triple(a.clone(), "pred", b.clone())
        .distinct(vec![a.clone(), b.clone()]);
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::Distinct(distinct_q) => {
            assert_eq!(distinct_q.variables, vec!["A".to_string(), "B".to_string()]);
            // Check inner query
            assert!(matches!(*distinct_q.query, Woql2Query::Triple(_)));
        }
        _ => panic!("Expected Distinct query, found {:?}", final_query),
    }
}

#[test]
fn test_triple_count() {
    let count_var = vars!("TripleCount");
    let builder = WoqlBuilder::triple_count("schema/main", count_var.clone());
    let final_query = builder.finalize();

    match final_query {
        Woql2Query::TripleCount(tc_q) => {
            assert_eq!(tc_q.resource, "schema/main");
            assert!(matches!(tc_q.count, DataValue::Variable(v) if v == "TripleCount"));
        }
        _ => panic!("Expected TripleCount query, found {:?}", final_query),
    }
}

#[test]
fn test_added_triple() {
    let (s_var, o_var) = vars!("S", "O");
    let builder = WoqlBuilder::new().added_triple(
        s_var.clone(),
        "prop:pred",
        o_var.clone(),
        Some(GraphType::Instance),
    );
    let final_query = builder.finalize();
    match final_query {
        Woql2Query::AddedTriple(added_q) => {
            assert!(matches!(added_q.subject, NodeValue::Variable(v) if v == "S"));
            assert!(matches!(added_q.predicate, NodeValue::Node(n) if n == "prop:pred"));
            assert!(matches!(added_q.object, Woql2Value::Variable(v) if v == "O"));
            assert_eq!(added_q.graph, GraphType::Instance);
        }
        _ => panic!("Expected AddedTriple query, found {:?}", final_query),
    }
}

#[test]
fn test_nested_limit_and_clauses() {
    let builder = WoqlBuilder::new();

    // Construct the inner query: And(Triple(...), ReadDocument(...))
    let inner_query_builder = builder
        .triple(Var::new("Subject"), "rdf:type", node("@schema:Axiom"))
        .read_document(Var::new("Subject"), Var::new("Doc"));

    // Apply limits
    let final_query_builder = inner_query_builder.limit(100).limit(10);

    let final_query = final_query_builder.finalize();

    // Expected JSON structure
    let expected_json_str = r#"{
        "@type": "Limit",
        "limit": 10,
        "query": {
            "@type": "Limit",
            "limit": 100,
            "query": {
                "@type": "And",
                "and": [
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Subject"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "node": "rdf:type"
                        },
                        "object": {
                            "@type": "Value",
                            "node": "@schema:Axiom"
                        },
                        "graph": "instance"
                    },
                    {
                        "@type": "ReadDocument",
                        "identifier": {
                            "@type": "NodeValue",
                            "variable": "Subject"
                        },
                        "document": {
                            "@type": "Value",
                            "variable": "Doc"
                        }
                    }
                ]
            }
        }
    }"#;

    // Serialize the built query using the to_json() method from woql2 traits
    let generated_json_val = final_query.to_json();

    // Parse the expected JSON string
    let expected_json_val: serde_json::Value =
        serde_json::from_str(expected_json_str).expect("Failed to parse expected JSON string");

    // Compare the JSON values
    assert_eq!(
        generated_json_val, expected_json_val,
        "Generated JSON does not match expected JSON"
    );
}
