use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query::{Query, And, Or, Not};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{Value, NodeValue, ListOrVariable};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

#[test]
fn test_simple_triple_rendering() {
    let triple = Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:name".to_string()),
        object: Value::Variable("Name".to_string()),
        graph: Some(GraphType::Instance),
    };
    
    let dsl = Query::Triple(triple).to_dsl();
    assert_eq!(dsl, r#"triple($Person, "@schema:name", $Name)"#);
}

#[test]
fn test_and_query_rendering() {
    let triple1 = Query::Triple(Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("rdf:type".to_string()),
        object: Value::Node("@schema:Person".to_string()),
        graph: Some(GraphType::Instance),
    });
    
    let triple2 = Query::Triple(Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:age".to_string()),
        object: Value::Variable("Age".to_string()),
        graph: Some(GraphType::Instance),
    });
    
    let and_query = Query::And(And {
        and: vec![triple1, triple2],
    });
    
    let dsl = and_query.to_dsl();
    assert_eq!(
        dsl,
        r#"and(triple($Person, "rdf:type", "@schema:Person"), triple($Person, "@schema:age", $Age))"#
    );
}

#[test]
fn test_select_query_rendering() {
    let triple = Query::Triple(Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:name".to_string()),
        object: Value::Variable("Name".to_string()),
        graph: Some(GraphType::Instance),
    });
    
    let select = Query::Select(Select {
        variables: vec!["Name".to_string()],
        query: Box::new(triple),
    });
    
    let dsl = select.to_dsl();
    assert_eq!(dsl, r#"select([$Name], triple($Person, "@schema:name", $Name))"#);
}

#[test]
fn test_comparison_rendering() {
    let greater = Query::Greater(Greater {
        left: DataValue::Variable("Age".to_string()),
        right: DataValue::Data(XSDAnySimpleType::Float(18.0)),
    });
    
    let dsl = greater.to_dsl();
    assert_eq!(dsl, "greater($Age, 18)");
}

#[test]
fn test_nested_query_rendering() {
    let inner_and = Query::And(And {
        and: vec![
            Query::Triple(Triple {
                subject: NodeValue::Variable("Person".to_string()),
                predicate: NodeValue::Node("@schema:age".to_string()),
                object: Value::Variable("Age".to_string()),
                graph: Some(GraphType::Instance),
            }),
            Query::Greater(Greater {
                left: DataValue::Variable("Age".to_string()),
                right: DataValue::Data(XSDAnySimpleType::Float(18.0)),
            }),
        ],
    });
    
    let or_query = Query::Or(Or {
        or: vec![
            inner_and,
            Query::Triple(Triple {
                subject: NodeValue::Variable("Person".to_string()),
                predicate: NodeValue::Node("@schema:isAdult".to_string()),
                object: Value::Data(XSDAnySimpleType::Boolean(true)),
                graph: Some(GraphType::Instance),
            }),
        ],
    });
    
    let dsl = or_query.to_dsl();
    assert_eq!(
        dsl,
        r#"or(and(triple($Person, "@schema:age", $Age), greater($Age, 18)), triple($Person, "@schema:isAdult", true))"#
    );
}

#[test]
fn test_optional_rendering() {
    let triple = Query::Triple(Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:nickname".to_string()),
        object: Value::Variable("Nickname".to_string()),
        graph: Some(GraphType::Instance),
    });
    
    let opt = Query::WoqlOptional(WoqlOptional {
        query: Box::new(triple),
    });
    
    let dsl = opt.to_dsl();
    assert_eq!(dsl, r#"opt(triple($Person, "@schema:nickname", $Nickname))"#);
}

#[test]
fn test_path_rendering() {
    let path = Query::Path(Path {
        subject: Value::Variable("Person".to_string()),
        pattern: PathPattern::Predicate(PathPredicate {
            predicate: Some("@schema:knows".to_string()),
        }),
        object: Value::Variable("Friend".to_string()),
        path: None,
    });
    
    let dsl = path.to_dsl();
    assert_eq!(dsl, r#"path($Person, pred("@schema:knows"), $Friend)"#);
}

#[test]
fn test_eval_arithmetic_rendering() {
    let expr = ArithmeticExpression::Plus(Plus {
        left: Box::new(ArithmeticExpression::Value(ArithmeticValue::Variable("X".to_string()))),
        right: Box::new(ArithmeticExpression::Value(ArithmeticValue::Variable("Y".to_string()))),
    });
    
    let eval = Query::Eval(Eval {
        expression: expr,
        result_value: ArithmeticValue::Variable("Sum".to_string()),
    });
    
    let dsl = eval.to_dsl();
    assert_eq!(dsl, "eval(plus($X, $Y), $Sum)");
}

#[test]
fn test_string_operations_rendering() {
    let concat = Query::Concatenate(Concatenate {
        list: ListOrVariable::Variable(DataValue::Variable("Parts".to_string())),
        result_string: DataValue::Variable("Result".to_string()),
    });
    
    let dsl = concat.to_dsl();
    assert_eq!(dsl, "concat($Parts, $Result)");
}

#[test]
fn test_document_operations_rendering() {
    let read = Query::ReadDocument(ReadDocument {
        identifier: NodeValue::Node("Person/john-doe".to_string()),
        document: Value::Variable("PersonData".to_string()),
    });
    
    let dsl = read.to_dsl();
    assert_eq!(dsl, r#"read_document("Person/john-doe", $PersonData)"#);
}