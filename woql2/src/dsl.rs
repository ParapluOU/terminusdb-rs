/// Module for rendering WOQL queries to DSL syntax

use std::fmt;

/// Trait for rendering WOQL queries to DSL syntax
pub trait ToDSL {
    /// Render this value/query to DSL syntax
    fn to_dsl(&self) -> String;
}

/// Helper function to escape string literals for DSL
fn escape_string(s: &str) -> String {
    // For now, just wrap in quotes and escape internal quotes
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// Helper function to render a list of items with a separator
fn render_list<T: ToDSL>(items: &[T], separator: &str) -> String {
    items
        .iter()
        .map(|item| item.to_dsl())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Helper function to render a function call
fn render_function(name: &str, args: &[String]) -> String {
    format!("{}({})", name, args.join(", "))
}

/// Helper function to render a list value
fn render_list_value<T: ToDSL>(items: &[T]) -> String {
    format!("[{}]", render_list(items, ", "))
}

// Implementations for value types
use crate::value::{Value, NodeValue, DataValue};
use terminusdb_schema::XSDAnySimpleType;

impl ToDSL for Value {
    fn to_dsl(&self) -> String {
        match self {
            Value::Variable(var) => format!("${}", var),
            Value::Node(uri) => escape_string(uri),
            Value::Data(data) => data.to_dsl(),
            Value::List(items) => render_list_value(items),
            Value::Dictionary(_dict) => {
                // Dictionary templates are complex, for now just render as empty dict
                // This would need more work to properly render field-value pairs
                "{}".to_string()
            }
        }
    }
}

impl ToDSL for NodeValue {
    fn to_dsl(&self) -> String {
        match self {
            NodeValue::Variable(var) => format!("${}", var),
            NodeValue::Node(uri) => escape_string(uri),
        }
    }
}

impl ToDSL for DataValue {
    fn to_dsl(&self) -> String {
        match self {
            DataValue::Variable(var) => format!("${}", var),
            DataValue::Data(data) => data.to_dsl(),
            DataValue::List(items) => render_list_value(items),
        }
    }
}

impl ToDSL for XSDAnySimpleType {
    fn to_dsl(&self) -> String {
        match self {
            XSDAnySimpleType::String(s) => escape_string(s),
            XSDAnySimpleType::Boolean(b) => b.to_string(),
            XSDAnySimpleType::Float(f) => f.to_string(),
            XSDAnySimpleType::Decimal(d) => d.to_string(),
            XSDAnySimpleType::DateTime(dt) => escape_string(&dt.to_string()),
            XSDAnySimpleType::Date(d) => escape_string(&d.to_string()),
            XSDAnySimpleType::Time(t) => escape_string(&t.to_string()),
            _ => escape_string(&format!("{:?}", self)),
        }
    }
}

// Implementations for Query types
use crate::query::{Query, And, Or, Not, True, Eval, Path};
use crate::triple::Triple;
use terminusdb_schema::GraphType;

impl ToDSL for Query {
    fn to_dsl(&self) -> String {
        match self {
            Query::And(and) => and.to_dsl(),
            Query::Or(or) => or.to_dsl(),
            Query::Not(not) => not.to_dsl(),
            Query::True(t) => t.to_dsl(),
            Query::Triple(triple) => triple.to_dsl(),
            Query::Eval(eval) => eval.to_dsl(),
            Query::Path(path) => path.to_dsl(),
            Query::Select(select) => select.to_dsl(),
            Query::Distinct(distinct) => distinct.to_dsl(),
            Query::Limit(limit) => limit.to_dsl(),
            Query::Start(start) => start.to_dsl(),
            Query::OrderBy(order_by) => order_by.to_dsl(),
            Query::GroupBy(group_by) => group_by.to_dsl(),
            Query::Greater(greater) => greater.to_dsl(),
            Query::Less(less) => less.to_dsl(),
            Query::Equals(equals) => equals.to_dsl(),
            Query::Count(count) => count.to_dsl(),
            Query::Sum(sum) => sum.to_dsl(),
            Query::Concatenate(concat) => concat.to_dsl(),
            Query::Substring(substring) => substring.to_dsl(),
            Query::Trim(trim) => trim.to_dsl(),
            Query::Upper(upper) => upper.to_dsl(),
            Query::Lower(lower) => lower.to_dsl(),
            Query::Regexp(regexp) => regexp.to_dsl(),
            Query::IsA(isa) => isa.to_dsl(),
            Query::TypeOf(type_of) => type_of.to_dsl(),
            Query::Subsumption(subsumption) => subsumption.to_dsl(),
            Query::WoqlOptional(opt) => opt.to_dsl(),
            Query::ReadDocument(read_doc) => read_doc.to_dsl(),
            Query::InsertDocument(insert_doc) => insert_doc.to_dsl(),
            Query::UpdateDocument(update_doc) => update_doc.to_dsl(),
            Query::DeleteDocument(delete_doc) => delete_doc.to_dsl(),
            // Add more as needed
            _ => format!("// TODO: {}", std::any::type_name_of_val(self)),
        }
    }
}

impl ToDSL for Triple {
    fn to_dsl(&self) -> String {
        let mut args = vec![
            self.subject.to_dsl(),
            self.predicate.to_dsl(),
            self.object.to_dsl(),
        ];
        
        // Only add graph parameter if it's not the default (Instance)
        if self.graph != GraphType::Instance {
            args.push(escape_string(&format!("{:?}", self.graph).to_lowercase()));
        }
        
        render_function("triple", &args)
    }
}

impl ToDSL for And {
    fn to_dsl(&self) -> String {
        let args: Vec<String> = self.and.iter().map(|q| q.to_dsl()).collect();
        render_function("and", &args)
    }
}

impl ToDSL for Or {
    fn to_dsl(&self) -> String {
        let args: Vec<String> = self.or.iter().map(|q| q.to_dsl()).collect();
        render_function("or", &args)
    }
}

impl ToDSL for Not {
    fn to_dsl(&self) -> String {
        render_function("not", &[self.query.to_dsl()])
    }
}

impl ToDSL for True {
    fn to_dsl(&self) -> String {
        "true()".to_string()
    }
}

// Import more types as needed
use crate::control::{Select, Distinct, WoqlOptional};
use crate::misc::{Limit, Start, Count};
use crate::order::{OrderBy, GroupBy, OrderTemplate, Order};
use crate::compare::{Greater, Less, Equals, IsA, TypeOf, Subsumption};
use crate::collection::Sum;
use crate::string::{Concatenate, Substring, Trim, Upper, Lower, Regexp};
use crate::document::{ReadDocument, InsertDocument, UpdateDocument, DeleteDocument};

impl ToDSL for Select {
    fn to_dsl(&self) -> String {
        let vars = format!("[{}]", 
            self.variables.iter()
                .map(|v| format!("${}", v))
                .collect::<Vec<_>>()
                .join(", ")
        );
        render_function("select", &[vars, self.query.to_dsl()])
    }
}

impl ToDSL for Distinct {
    fn to_dsl(&self) -> String {
        let vars = format!("[{}]", 
            self.variables.iter()
                .map(|v| format!("${}", v))
                .collect::<Vec<_>>()
                .join(", ")
        );
        render_function("distinct", &[vars, self.query.to_dsl()])
    }
}

impl ToDSL for Limit {
    fn to_dsl(&self) -> String {
        render_function("limit", &[self.limit.to_string(), self.query.to_dsl()])
    }
}

impl ToDSL for Start {
    fn to_dsl(&self) -> String {
        render_function("start", &[self.start.to_string(), self.query.to_dsl()])
    }
}

impl ToDSL for OrderBy {
    fn to_dsl(&self) -> String {
        let order_specs = render_list_value(&self.ordering);
        render_function("order_by", &[order_specs, self.query.to_dsl()])
    }
}

impl ToDSL for OrderTemplate {
    fn to_dsl(&self) -> String {
        let order_fn = match self.order {
            Order::Asc => "asc",
            Order::Desc => "desc",
        };
        format!("{}(${})", order_fn, self.variable)
    }
}

impl ToDSL for GroupBy {
    fn to_dsl(&self) -> String {
        let group_vars = format!("[{}]", 
            self.group_by.iter()
                .map(|v| format!("${}", v))
                .collect::<Vec<_>>()
                .join(", ")
        );
        render_function("group_by", &[
            group_vars,
            self.template.to_dsl(),
            self.grouped_value.to_dsl(),
            self.query.to_dsl()
        ])
    }
}

impl ToDSL for Greater {
    fn to_dsl(&self) -> String {
        render_function("greater", &[self.left.to_dsl(), self.right.to_dsl()])
    }
}

impl ToDSL for Less {
    fn to_dsl(&self) -> String {
        render_function("less", &[self.left.to_dsl(), self.right.to_dsl()])
    }
}

impl ToDSL for Equals {
    fn to_dsl(&self) -> String {
        render_function("eq", &[self.left.to_dsl(), self.right.to_dsl()])
    }
}

impl ToDSL for Count {
    fn to_dsl(&self) -> String {
        render_function("count", &[self.query.to_dsl(), self.count.to_dsl()])
    }
}

impl ToDSL for Sum {
    fn to_dsl(&self) -> String {
        render_function("sum", &[self.list.to_dsl(), self.result.to_dsl()])
    }
}

impl ToDSL for Concatenate {
    fn to_dsl(&self) -> String {
        render_function("concat", &[self.list.to_dsl(), self.result_string.to_dsl()])
    }
}

impl ToDSL for Substring {
    fn to_dsl(&self) -> String {
        render_function("substring", &[
            self.string.to_dsl(),
            self.before.to_dsl(),
            self.length.to_dsl(),
            self.after.to_dsl(),
            self.substring.to_dsl()
        ])
    }
}

impl ToDSL for Trim {
    fn to_dsl(&self) -> String {
        render_function("trim", &[self.untrimmed.to_dsl(), self.trimmed.to_dsl()])
    }
}

impl ToDSL for Upper {
    fn to_dsl(&self) -> String {
        render_function("upper", &[self.mixed.to_dsl(), self.upper.to_dsl()])
    }
}

impl ToDSL for Lower {
    fn to_dsl(&self) -> String {
        render_function("lower", &[self.mixed.to_dsl(), self.lower.to_dsl()])
    }
}

impl ToDSL for Regexp {
    fn to_dsl(&self) -> String {
        let mut args = vec![self.pattern.to_dsl(), self.string.to_dsl()];
        if let Some(result) = &self.result {
            args.push(result.to_dsl());
        }
        render_function("regexp", &args)
    }
}

impl ToDSL for IsA {
    fn to_dsl(&self) -> String {
        render_function("isa", &[self.element.to_dsl(), self.type_of.to_dsl()])
    }
}

impl ToDSL for TypeOf {
    fn to_dsl(&self) -> String {
        render_function("type_of", &[self.value.to_dsl(), self.type_uri.to_dsl()])
    }
}

impl ToDSL for Subsumption {
    fn to_dsl(&self) -> String {
        render_function("subsumption", &[self.parent.to_dsl(), self.child.to_dsl()])
    }
}

impl ToDSL for WoqlOptional {
    fn to_dsl(&self) -> String {
        render_function("opt", &[self.query.to_dsl()])
    }
}

impl ToDSL for ReadDocument {
    fn to_dsl(&self) -> String {
        render_function("read_document", &[self.identifier.to_dsl(), self.document.to_dsl()])
    }
}

impl ToDSL for InsertDocument {
    fn to_dsl(&self) -> String {
        let mut args = vec![self.document.to_dsl()];
        if let Some(id) = &self.identifier {
            args.push(id.to_dsl());
        }
        render_function("insert_document", &args)
    }
}

impl ToDSL for UpdateDocument {
    fn to_dsl(&self) -> String {
        let mut args = vec![self.document.to_dsl()];
        if let Some(id) = &self.identifier {
            args.push(id.to_dsl());
        }
        render_function("update_document", &args)
    }
}

impl ToDSL for DeleteDocument {
    fn to_dsl(&self) -> String {
        render_function("delete_document", &[self.identifier.to_dsl()])
    }
}

// Arithmetic expressions and path patterns need special handling
use crate::expression::{ArithmeticExpression, ArithmeticValue, Plus, Minus, Times, Div};

impl ToDSL for Eval {
    fn to_dsl(&self) -> String {
        render_function("eval", &[self.expression.to_dsl(), self.result_value.to_dsl()])
    }
}

impl ToDSL for ArithmeticExpression {
    fn to_dsl(&self) -> String {
        match self {
            ArithmeticExpression::Value(val) => val.to_dsl(),
            ArithmeticExpression::Plus(plus) => render_function("plus", &[plus.left.to_dsl(), plus.right.to_dsl()]),
            ArithmeticExpression::Minus(minus) => render_function("minus", &[minus.left.to_dsl(), minus.right.to_dsl()]),
            ArithmeticExpression::Times(times) => render_function("times", &[times.left.to_dsl(), times.right.to_dsl()]),
            ArithmeticExpression::Div(div) => render_function("div", &[div.left.to_dsl(), div.right.to_dsl()]),
            _ => format!("// TODO: arithmetic {:?}", self),
        }
    }
}

impl ToDSL for ArithmeticValue {
    fn to_dsl(&self) -> String {
        match self {
            ArithmeticValue::Variable(var) => format!("${}", var),
            ArithmeticValue::Data(data) => data.to_dsl(),
        }
    }
}

// Path patterns
use crate::path::{PathPattern, PathPredicate, InversePathPredicate, PathStar, PathPlus, PathSequence, PathOr};

impl ToDSL for Path {
    fn to_dsl(&self) -> String {
        let mut args = vec![
            self.subject.to_dsl(),
            self.pattern.to_dsl(),
            self.object.to_dsl(),
        ];
        if let Some(path) = &self.path {
            args.push(path.to_dsl());
        }
        render_function("path", &args)
    }
}

impl ToDSL for PathPattern {
    fn to_dsl(&self) -> String {
        match self {
            PathPattern::Predicate(pred) => pred.to_dsl(),
            PathPattern::InversePredicate(inv) => inv.to_dsl(),
            PathPattern::Star(star) => star.to_dsl(),
            PathPattern::Plus(plus) => plus.to_dsl(),
            PathPattern::Sequence(seq) => seq.to_dsl(),
            PathPattern::Or(or) => or.to_dsl(),
            _ => format!("// TODO: path pattern {:?}", self),
        }
    }
}

impl ToDSL for PathPredicate {
    fn to_dsl(&self) -> String {
        if let Some(pred) = &self.predicate {
            render_function("pred", &[escape_string(pred)])
        } else {
            render_function("pred", &[escape_string("")])
        }
    }
}

impl ToDSL for InversePathPredicate {
    fn to_dsl(&self) -> String {
        if let Some(pred) = &self.predicate {
            render_function("inv", &[escape_string(pred)])
        } else {
            render_function("inv", &[escape_string("")])
        }
    }
}

impl ToDSL for PathStar {
    fn to_dsl(&self) -> String {
        render_function("star", &[self.star.to_dsl()])
    }
}

impl ToDSL for PathPlus {
    fn to_dsl(&self) -> String {
        render_function("plus", &[self.plus.to_dsl()])
    }
}

impl ToDSL for PathSequence {
    fn to_dsl(&self) -> String {
        let patterns: Vec<String> = self.sequence.iter().map(|p| p.to_dsl()).collect();
        render_function("seq", &patterns)
    }
}

impl ToDSL for PathOr {
    fn to_dsl(&self) -> String {
        let patterns: Vec<String> = self.or.iter().map(|p| p.to_dsl()).collect();
        render_function("or", &patterns)
    }
}

// Display implementations using ToDSL

// Value types
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for NodeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for DataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Query types
impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for And {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Or {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Not {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for True {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Eval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Control flow
impl fmt::Display for Select {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Distinct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for WoqlOptional {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Misc
impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Start {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Order
impl fmt::Display for OrderBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for GroupBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for OrderTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Comparison
impl fmt::Display for Greater {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Less {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Equals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for IsA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for TypeOf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Subsumption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Collection
impl fmt::Display for Sum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// String operations
impl fmt::Display for Concatenate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Substring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Trim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Upper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Lower {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for Regexp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Document operations
impl fmt::Display for ReadDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for InsertDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for UpdateDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for DeleteDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Arithmetic
impl fmt::Display for ArithmeticExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for ArithmeticValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

// Path patterns
impl fmt::Display for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for PathPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for InversePathPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for PathStar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for PathPlus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for PathSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}

impl fmt::Display for PathOr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dsl())
    }
}