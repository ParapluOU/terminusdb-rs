use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, multispace0},
    combinator::{map, recognize},
    error::{ParseError as NomParseError, VerboseError},
    multi::{separated_list0, separated_list1},
    number::complete::double,
    sequence::{delimited, preceded, tuple},
    IResult,
};
use terminusdb_woql2::{
    collection::Sum,
    compare::{Equals, Greater, IsA, Less, Subsumption, TypeOf},
    control::{Distinct, Select, WoqlOptional},
    document::{DeleteDocument, InsertDocument, ReadDocument, UpdateDocument},
    expression::{ArithmeticExpression, ArithmeticValue, Div, Minus, Plus, Times},
    misc::{Count, Limit, Start},
    order::{GroupBy, Order, OrderBy, OrderTemplate},
    path::{InversePathPredicate, PathOr, PathPattern, PathPlus, PathPredicate, PathSequence, PathStar},
    query::{And, Eval, Not, Or, Path, Query},
    string::{Concatenate, Lower, Regexp, Substring, Trim, Upper},
    triple::Triple,
    value::{DataValue, NodeValue, Value},
};
use terminusdb_schema::{XSDAnySimpleType, GraphType};

use crate::error::{ParseError, ParseResult};

type ParseInput<'a> = &'a str;
type NomResult<'a, T> = IResult<ParseInput<'a>, T, VerboseError<ParseInput<'a>>>;

pub fn parse_woql_dsl(input: &str) -> ParseResult<Query> {
    let input = input.trim();
    match parse_query(input) {
        Ok((remaining, query)) => {
            if remaining.trim().is_empty() {
                Ok(query)
            } else {
                Err(ParseError::ParseError {
                    position: input.len() - remaining.len(),
                    message: format!("Unexpected input: '{}'", remaining),
                })
            }
        }
        Err(e) => Err(ParseError::NomError(format!("{:?}", e))),
    }
}

fn ws<'a, F, O>(f: F) -> impl FnMut(ParseInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(ParseInput<'a>) -> NomResult<'a, O>,
{
    delimited(multispace0, f, multispace0)
}

fn parse_identifier(input: ParseInput) -> NomResult<&str> {
    recognize(tuple((
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_'),
    )))(input)
}

fn parse_variable(input: ParseInput) -> NomResult<String> {
    map(
        preceded(char('$'), parse_identifier),
        |s: &str| s.to_string(),
    )(input)
}

fn parse_string_literal(input: ParseInput) -> NomResult<String> {
    let (input, _) = char('"')(input)?;
    let (input, content) = take_while(|c| c != '"')(input)?;
    let (input, _) = char('"')(input)?;
    Ok((input, content.to_string()))
}

fn parse_number(input: ParseInput) -> NomResult<f64> {
    double(input)
}

fn parse_boolean(input: ParseInput) -> NomResult<bool> {
    alt((
        map(tag("true"), |_| true),
        map(tag("false"), |_| false),
    ))(input)
}

fn parse_value(input: ParseInput) -> NomResult<Value> {
    alt((
        map(parse_variable, |var| Value::Variable(var)),
        map(parse_string_literal, |s| {
            if s.starts_with('@') || s.contains(':') {
                Value::Node(s)
            } else {
                Value::Data(XSDAnySimpleType::String(s))
            }
        }),
        map(parse_number, |n| Value::Data(XSDAnySimpleType::Float(n))),
        map(parse_boolean, |b| Value::Data(XSDAnySimpleType::Boolean(b))),
        parse_list_value,
    ))(input)
}

fn parse_list_value(input: ParseInput) -> NomResult<Value> {
    map(
        delimited(
            ws(char('[')),
            separated_list0(ws(char(',')), parse_value),
            ws(char(']')),
        ),
        |values| Value::List(values),
    )(input)
}

fn parse_node_value(input: ParseInput) -> NomResult<NodeValue> {
    alt((
        map(parse_variable, |var| NodeValue::Variable(var)),
        map(parse_string_literal, |s| NodeValue::Node(s)),
    ))(input)
}

fn parse_data_value(input: ParseInput) -> NomResult<DataValue> {
    alt((
        map(parse_variable, |var| DataValue::Variable(var)),
        map(parse_string_literal, |s| DataValue::Data(XSDAnySimpleType::String(s))),
        map(parse_number, |n| DataValue::Data(XSDAnySimpleType::Float(n))),
        map(parse_boolean, |b| DataValue::Data(XSDAnySimpleType::Boolean(b))),
    ))(input)
}

fn parse_function_call(input: ParseInput) -> NomResult<(&str, Vec<ParsedArg>)> {
    let (input, name) = parse_identifier(input)?;
    let (input, args) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), parse_argument),
        ws(char(')')),
    )(input)?;
    Ok((input, (name, args)))
}

#[derive(Debug, Clone)]
enum ParsedArg {
    Query(Query),
    Value(Value),
    NodeValue(NodeValue),
    DataValue(DataValue),
    ValueList(Vec<Value>),
    StringList(Vec<String>),
    ArithmeticExpr(ArithmeticExpression),
    PathPattern(PathPattern),
    OrderTemplate(Vec<OrderTemplate>),
}

fn parse_argument(input: ParseInput) -> NomResult<ParsedArg> {
    alt((
        map(parse_order_template_list, ParsedArg::OrderTemplate),
        map(parse_value_list, ParsedArg::ValueList),
        map(parse_arithmetic_expr, ParsedArg::ArithmeticExpr),
        map(parse_path_pattern, ParsedArg::PathPattern),
        map(parse_query, ParsedArg::Query),
        map(parse_value, ParsedArg::Value),
    ))(input)
}

fn parse_value_list(input: ParseInput) -> NomResult<Vec<Value>> {
    delimited(
        ws(char('[')),
        separated_list0(ws(char(',')), parse_value),
        ws(char(']')),
    )(input)
}

fn parse_order_template_list(input: ParseInput) -> NomResult<Vec<OrderTemplate>> {
    delimited(
        ws(char('[')),
        separated_list0(ws(char(',')), parse_order_template),
        ws(char(']')),
    )(input)
}

fn parse_order_template(input: ParseInput) -> NomResult<OrderTemplate> {
    alt((
        map(
            preceded(tag("asc"), delimited(ws(char('(')), parse_variable, ws(char(')')))),
            |var| OrderTemplate {
                order: Order::Asc,
                variable: var,
            },
        ),
        map(
            preceded(tag("desc"), delimited(ws(char('(')), parse_variable, ws(char(')')))),
            |var| OrderTemplate {
                order: Order::Desc,
                variable: var,
            },
        ),
    ))(input)
}

fn parse_arithmetic_expr(input: ParseInput) -> NomResult<ArithmeticExpression> {
    alt((
        map(
            preceded(tag("plus"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_arithmetic_value),
                ws(char(')')),
            )),
            |args| {
                if args.len() != 2 {
                    panic!("plus requires exactly 2 arguments");
                }
                ArithmeticExpression::Plus(Plus {
                    left: Box::new(args[0].clone()),
                    right: Box::new(args[1].clone()),
                })
            },
        ),
        map(
            preceded(tag("minus"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_arithmetic_value),
                ws(char(')')),
            )),
            |args| {
                if args.len() != 2 {
                    panic!("minus requires exactly 2 arguments");
                }
                ArithmeticExpression::Minus(Minus {
                    left: Box::new(args[0].clone()),
                    right: Box::new(args[1].clone()),
                })
            },
        ),
        map(
            preceded(tag("times"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_arithmetic_value),
                ws(char(')')),
            )),
            |args| {
                if args.len() != 2 {
                    panic!("times requires exactly 2 arguments");
                }
                ArithmeticExpression::Times(Times {
                    left: Box::new(args[0].clone()),
                    right: Box::new(args[1].clone()),
                })
            },
        ),
        map(
            preceded(tag("div"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_arithmetic_value),
                ws(char(')')),
            )),
            |args| {
                if args.len() != 2 {
                    panic!("div requires exactly 2 arguments");
                }
                ArithmeticExpression::Div(Div {
                    left: Box::new(args[0].clone()),
                    right: Box::new(args[1].clone()),
                })
            },
        ),
    ))(input)
}

fn parse_arithmetic_value(input: ParseInput) -> NomResult<ArithmeticExpression> {
    alt((
        map(parse_variable, |var| ArithmeticExpression::Value(ArithmeticValue::Variable(var))),
        map(parse_number, |n| ArithmeticExpression::Value(ArithmeticValue::Data(XSDAnySimpleType::Float(n)))),
        parse_arithmetic_expr,
    ))(input)
}

fn parse_path_pattern(input: ParseInput) -> NomResult<PathPattern> {
    alt((
        map(
            preceded(tag("pred"), delimited(
                ws(char('(')),
                parse_string_literal,
                ws(char(')')),
            )),
            |pred| PathPattern::Predicate(PathPredicate { predicate: Some(pred) }),
        ),
        map(
            preceded(tag("inv"), delimited(
                ws(char('(')),
                parse_string_literal,
                ws(char(')')),
            )),
            |pred| PathPattern::InversePredicate(InversePathPredicate { predicate: Some(pred) }),
        ),
        map(
            preceded(tag("star"), delimited(
                ws(char('(')),
                parse_path_pattern,
                ws(char(')')),
            )),
            |pattern| PathPattern::Star(PathStar { 
                star: Box::new(pattern),
            }),
        ),
        map(
            preceded(tag("plus"), delimited(
                ws(char('(')),
                parse_path_pattern,
                ws(char(')')),
            )),
            |pattern| PathPattern::Plus(PathPlus { 
                plus: Box::new(pattern),
            }),
        ),
        map(
            preceded(tag("seq"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_path_pattern),
                ws(char(')')),
            )),
            |patterns| PathPattern::Sequence(PathSequence { 
                sequence: patterns,
            }),
        ),
        map(
            preceded(tag("or"), delimited(
                ws(char('(')),
                separated_list1(ws(char(',')), parse_path_pattern),
                ws(char(')')),
            )),
            |patterns| PathPattern::Or(PathOr { 
                or: patterns,
            }),
        ),
    ))(input)
}

fn parse_query(input: ParseInput) -> NomResult<Query> {
    alt((
        parse_vars_statement,
        parse_query_function,
    ))(input)
}

fn parse_vars_statement(input: ParseInput) -> NomResult<Query> {
    let (input, _) = tag("vars")(input)?;
    let (input, _vars) = delimited(
        ws(char('(')),
        separated_list1(ws(char(',')), parse_variable),
        ws(char(')')),
    )(input)?;
    
    // Continue parsing the rest of the query after vars declaration
    let (input, _) = multispace0(input)?;
    parse_query(input)
}

fn parse_query_function(input: ParseInput) -> NomResult<Query> {
    let (input, (name, args)) = parse_function_call(input)?;
    
    let query = match name {
        "triple" => parse_triple_args(args),
        "and" => parse_and_args(args),
        "or" => parse_or_args(args),
        "not" => parse_not_args(args),
        "select" => parse_select_args(args),
        "distinct" => parse_distinct_args(args),
        "limit" => parse_limit_args(args),
        "start" => parse_start_args(args),
        "order_by" => parse_order_by_args(args),
        "group_by" => parse_group_by_args(args),
        "greater" => parse_greater_args(args),
        "less" => parse_less_args(args),
        "eq" => parse_equals_args(args),
        "eval" => parse_eval_args(args),
        "path" => parse_path_args(args),
        "read_document" => parse_read_document_args(args),
        "insert_document" => parse_insert_document_args(args),
        "update_document" => parse_update_document_args(args),
        "delete_document" => parse_delete_document_args(args),
        "count" => parse_count_args(args),
        "sum" => parse_sum_args(args),
        "concat" => parse_concat_args(args),
        "substring" => parse_substring_args(args),
        "trim" => parse_trim_args(args),
        "upper" => parse_upper_args(args),
        "lower" => parse_lower_args(args),
        "regexp" => parse_regexp_args(args),
        "isa" => parse_isa_args(args),
        "type_of" => parse_type_of_args(args),
        "subsumption" => parse_subsumption_args(args),
        "opt" => parse_opt_args(args),
        "optional" => parse_opt_args(args),
        _ => Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }?;
    
    Ok((input, query))
}

// Helper functions for parsing specific query types
fn parse_triple_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() < 3 || args.len() > 4 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let subject = match &args[0] {
        ParsedArg::Value(Value::Variable(v)) => NodeValue::Variable(v.clone()),
        ParsedArg::Value(Value::Node(n)) => NodeValue::Node(n.clone()),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let predicate = match &args[1] {
        ParsedArg::Value(Value::Variable(v)) => NodeValue::Variable(v.clone()),
        ParsedArg::Value(Value::Node(n)) => NodeValue::Node(n.clone()),
        ParsedArg::Value(Value::Data(XSDAnySimpleType::String(s))) => NodeValue::Node(s.clone()),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let object = match &args[2] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let graph = if args.len() == 4 {
        match &args[3] {
            ParsedArg::Value(Value::Data(XSDAnySimpleType::String(s))) => {
                match s.as_str() {
                    "instance" => GraphType::Instance,
                    "schema" => GraphType::Schema,
                    _ => GraphType::Instance,
                }
            }
            _ => GraphType::Instance,
        }
    } else {
        GraphType::Instance
    };
    
    Ok(Query::Triple(Triple {
        subject,
        predicate,
        object,
        graph,
    }))
}

fn parse_and_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    let mut queries = Vec::new();
    for arg in args {
        match arg {
            ParsedArg::Query(q) => queries.push(q),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        }
    }
    Ok(Query::And(And { and: queries }))
}

fn parse_or_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    let mut queries = Vec::new();
    for arg in args {
        match arg {
            ParsedArg::Query(q) => queries.push(q),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        }
    }
    Ok(Query::Or(Or { or: queries }))
}

fn parse_not_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 1 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    match &args[0] {
        ParsedArg::Query(q) => Ok(Query::Not(Not { query: Box::new(q.clone()) })),
        _ => Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    }
}

fn parse_select_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let variables = match &args[0] {
        ParsedArg::ValueList(values) => {
            let mut vars = Vec::new();
            for v in values {
                match v {
                    Value::Variable(var) => vars.push(var.clone()),
                    _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                        "",
                        nom::error::ErrorKind::Tag,
                    ))),
                }
            }
            vars
        }
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[1] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Select(Select {
        variables,
        query: Box::new(query),
    }))
}

fn parse_distinct_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let variables = match &args[0] {
        ParsedArg::ValueList(values) => {
            let mut vars = Vec::new();
            for v in values {
                match v {
                    Value::Variable(var) => vars.push(var.clone()),
                    _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                        "",
                        nom::error::ErrorKind::Tag,
                    ))),
                }
            }
            vars
        }
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[1] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Distinct(Distinct {
        variables,
        query: Box::new(query),
    }))
}

fn parse_limit_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let limit = match &args[0] {
        ParsedArg::Value(Value::Data(XSDAnySimpleType::Float(n))) => *n as i64,
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[1] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Limit(Limit {
        limit: limit as u64,
        query: Box::new(query),
    }))
}

fn parse_start_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let start = match &args[0] {
        ParsedArg::Value(Value::Data(XSDAnySimpleType::Float(n))) => *n as i64,
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[1] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Start(Start {
        start: start as u64,
        query: Box::new(query),
    }))
}

fn parse_order_by_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let ordering = match &args[0] {
        ParsedArg::OrderTemplate(templates) => templates.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[1] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::OrderBy(OrderBy {
        ordering,
        query: Box::new(query),
    }))
}

fn parse_group_by_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 4 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let template = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let group_by = match &args[1] {
        ParsedArg::ValueList(values) => {
            let mut vars = Vec::new();
            for v in values {
                match v {
                    Value::Variable(var) => vars.push(var.clone()),
                    _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                        "",
                        nom::error::ErrorKind::Tag,
                    ))),
                }
            }
            vars
        }
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let grouped_value = match &args[2] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let query = match &args[3] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::GroupBy(GroupBy {
        template,
        group_by,
        grouped_value,
        query: Box::new(query),
    }))
}

fn parse_greater_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let left = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let right = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Greater(Greater { left, right }))
}

fn parse_less_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let left = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let right = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Less(Less { left, right }))
}

fn parse_equals_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let left = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let right = match &args[1] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Equals(Equals { left, right }))
}

fn parse_eval_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let expression = match &args[0] {
        ParsedArg::ArithmeticExpr(expr) => expr.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let result_value = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => ArithmeticValue::Variable(var.clone()),
            Value::Data(d) => ArithmeticValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Eval(Eval {
        expression,
        result_value,
    }))
}

fn parse_path_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() < 3 || args.len() > 4 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let subject = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let pattern = match &args[1] {
        ParsedArg::PathPattern(p) => p.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let object = match &args[2] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let path = if args.len() == 4 {
        match &args[3] {
            ParsedArg::Value(v) => Some(v.clone()),
            _ => None,
        }
    } else {
        None
    };
    
    Ok(Query::Path(Path {
        subject,
        pattern,
        object,
        path,
    }))
}

fn parse_read_document_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let id = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let document = match &args[1] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::ReadDocument(ReadDocument {
        identifier: id,
        document,
    }))
}

fn parse_insert_document_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() < 1 || args.len() > 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let document = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let identifier = if args.len() == 2 {
        match &args[1] {
            ParsedArg::Value(v) => match v {
                Value::Variable(var) => Some(NodeValue::Variable(var.clone())),
                Value::Node(n) => Some(NodeValue::Node(n.clone())),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };
    
    Ok(Query::InsertDocument(InsertDocument {
        document,
        identifier,
    }))
}

fn parse_update_document_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() < 1 || args.len() > 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let document = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let identifier = if args.len() == 2 {
        match &args[1] {
            ParsedArg::Value(v) => match v {
                Value::Variable(var) => Some(NodeValue::Variable(var.clone())),
                Value::Node(n) => Some(NodeValue::Node(n.clone())),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };
    
    Ok(Query::UpdateDocument(UpdateDocument { document, identifier }))
}

fn parse_delete_document_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 1 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let identifier = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::DeleteDocument(DeleteDocument { identifier }))
}

fn parse_count_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let query = match &args[0] {
        ParsedArg::Query(q) => q.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let count = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Count(Count {
        query: Box::new(query),
        count,
    }))
}

fn parse_sum_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let list = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::List(_) => {
                // Lists as DataValue not directly supported in this DSL
                return Err(nom::Err::Error(VerboseError::from_error_kind(
                    "",
                    nom::error::ErrorKind::Tag,
                )));
            }
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let result = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Sum(Sum {
        list,
        result,
    }))
}

fn parse_concat_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let list = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::List(_) => {
                // For now, we'll just use a variable - proper list handling would need more work
                return Err(nom::Err::Error(VerboseError::from_error_kind(
                    "",
                    nom::error::ErrorKind::Tag,
                )));
            }
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        ParsedArg::ValueList(_) => {
            // The DSL doesn't directly support passing lists as DataValue
            // This would need a redesign of how we handle list arguments
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            )));
        }
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let result_string = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Concatenate(Concatenate { list, result_string }))
}

fn parse_substring_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 5 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let string = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let before = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let length = match &args[2] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let after = match &args[3] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let substring = match &args[4] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Substring(Substring {
        string,
        before,
        length,
        after,
        substring,
    }))
}

fn parse_trim_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let untrimmed = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let trimmed = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Trim(Trim { untrimmed, trimmed }))
}

fn parse_upper_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let mixed = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let upper = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Upper(Upper { mixed, upper }))
}

fn parse_lower_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let mixed = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let lower = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Lower(Lower { mixed, lower }))
}

fn parse_regexp_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 3 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let string = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let pattern = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => DataValue::Variable(var.clone()),
            Value::Data(d) => DataValue::Data(d.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let result = match &args[2] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => Some(DataValue::Variable(var.clone())),
            _ => None,
        },
        _ => None,
    };
    
    Ok(Query::Regexp(Regexp {
        pattern,
        string,
        result,
    }))
}

fn parse_isa_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let element = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let r#type = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            Value::Data(XSDAnySimpleType::String(s)) => NodeValue::Node(s.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::IsA(IsA { element, type_of: r#type }))
}

fn parse_type_of_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let value = match &args[0] {
        ParsedArg::Value(v) => v.clone(),
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let type_uri = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::TypeOf(TypeOf { value, type_uri }))
}

fn parse_subsumption_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 2 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    let parent = match &args[0] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    let child = match &args[1] {
        ParsedArg::Value(v) => match v {
            Value::Variable(var) => NodeValue::Variable(var.clone()),
            Value::Node(n) => NodeValue::Node(n.clone()),
            _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                "",
                nom::error::ErrorKind::Tag,
            ))),
        },
        _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    };
    
    Ok(Query::Subsumption(Subsumption { parent, child }))
}

fn parse_opt_args(args: Vec<ParsedArg>) -> Result<Query, nom::Err<VerboseError<&'static str>>> {
    if args.len() != 1 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Count,
        )));
    }
    
    match &args[0] {
        ParsedArg::Query(q) => Ok(Query::WoqlOptional(WoqlOptional { 
            query: Box::new(q.clone()) 
        })),
        _ => Err(nom::Err::Error(VerboseError::from_error_kind(
            "",
            nom::error::ErrorKind::Tag,
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable() {
        assert_eq!(parse_variable("$Person").unwrap().1, "Person");
        assert_eq!(parse_variable("$my_var").unwrap().1, "my_var");
    }

    #[test]
    fn test_parse_simple_triple() {
        let dsl = r#"triple($Person, "@schema:name", $Name)"#;
        let query = parse_woql_dsl(dsl).unwrap();
        
        match query {
            Query::Triple(t) => {
                assert_eq!(t.subject, NodeValue::Variable("Person".to_string()));
                assert_eq!(t.predicate, NodeValue::Node("@schema:name".to_string()));
                assert_eq!(t.object, Value::Variable("Name".to_string()));
            }
            _ => panic!("Expected Triple query"),
        }
    }

    #[test]
    fn test_parse_select_with_vars() {
        let dsl = r#"
vars($Person, $Name)
select(
    [$Name],
    triple($Person, "@schema:name", $Name)
)
"#;
        let query = parse_woql_dsl(dsl).unwrap();
        
        match query {
            Query::Select(s) => {
                assert_eq!(s.variables, vec!["Name"]);
                match s.query.as_ref() {
                    Query::Triple(_) => (),
                    _ => panic!("Expected Triple in Select"),
                }
            }
            _ => panic!("Expected Select query"),
        }
    }
}