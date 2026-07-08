---
tags:
  - woql
  - explanation
  - intermediate
title: What is unification?
nextjs:
  metadata:
    title: What is unification?
    description: Understand unification of variables, a unique and powerful datalog concept included in TerminusDB
    keywords: unification, prolog, datalog, variable, concept
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/what-is-unification/
media: []
---

**Unification** is the mechanism that makes WOQL queries work. When the same variable appears in multiple positions within a query, unification finds all values that satisfy every constraint simultaneously — creating implicit joins without explicit JOIN syntax. If you have used Prolog, this will be familiar; if you haven’t, think of it as pattern-matching across multiple conditions at once.

This page explains how unification works in WOQL, how variables are bound and scoped, and how to use this to write powerful queries that would require complex JOINs or subqueries in SQL.

## Core Principles of Variable Binding

Variables are defined in client language WOQL by prefixing them with the letter `v:`, or by using the built in variables generation functions in the clients. They are initially not bound, floating, and will take any value.

In WOQL, they can be bound to constants, or other variables using the `eq()` predicate, or they can get values by letting the WOQL engine match them with knowledge graph data, constrained by the logic of the query.

### Single Value Constraint

The fundamental rule governing unification is simple yet powerful: a logical variable for a query can only take on one value in a given solution. This means that once a variable gets bound to a specific value during query evaluation, it maintains that same value throughout the entire solution.

This means that variables may need to be scoped to particular sections of a query using the `select()` keyword to "firewall" them from other parts of the query. This can be accomplished using the `eq()` predicate for setting a locally unique variable, and the `select()` keyword that does not include that variable to make sure it does not "bubble up" to other scopes.

The single value per solution means that all viable solutions will be projected in the final materialization of the WOQL query based on the logic. The variables that have been `select()`ed will be returned as part of the bindings from the query API call.

### Consistency Requirement

When you use the same variable in multiple places within your query, unification ensures consistency, but also requires that the variable is expected to be bound to the same value in all places (for the same solution). If the variable is used in two places, these two values will be evaluated to be the same value. This consistency check is what makes pattern matching so reliable in graph queries.

You can think of the variable of a list with all possible values consistent with the solutions of query logic in the local scope of the query. Initially, the an unbound variable of a `triple("v:subject", "rdf:type", "v:type")` query will have `v:subject` match every subject in the graph, and `v:type` match every type in the graph, and thus return every document and subdocument stored in the graph.

If, instead, the below query is passed, solutions will only match and include where `v:subject`s having the `Entity` type.

```javascript
and(
  eq("v:type", "@schema:Entity"),
  triple("v:subject", "rdf:type", "v:type")
)
```

All solutions that are possible are brought by the engine, and this enables some unexpected and impressive abilities of the engine that we will come back to, as it will "fill in the blanks" for all solutions that are possible.

### How Variables Get Their Values

Variables obtain concrete values through three primary mechanisms:

  * From equations - Direct assignments or comparisons
  * From logic constraints applied to values of variables
  * From predicate definitions - Through pattern matching against existing data in the graph

## Unification in WOQL Context

### The Triple Predicate

The most important predicate in WOQL is `triple`, which gives results about edges in the current graph. This predicate forms the foundation for most graph queries and demonstrates unification in action. It has a close cousin in the `quad` predicate, which can return results from adjacent graphs such as the schema graph.

### Variable Representation

In WOQL, logical variables are represented as strings with the prefix “v:”. For example:
	*	`"v:Subject"`
	*	`"v:Predicate"`
	*	`"v:Object"`

### Pattern Matching vs Generation

Unification in functions enables most WOQL functions to serve as both pattern matchers and pattern generators, depending on whether a variable or constant is provided as an argument. This dual nature makes WOQL incredibly flexible:

	*	Pattern Matching: When you provide constants, WOQL matches existing data with the constant
	*	Pattern Generation: When you provide variables, WOQL generates all possible valid solutions, which can be constrained by rules and match against data

## Practical Examples

### Basic Variable Unification

```javascript
triple("v:Subject", "v:Predicate", "v:Object")
```

This query gets back the solutions for every possible assignment of subjects, predicates, and objects that the graph currently has - essentially all edges in the instance graph, across all documents and subdocuments stored in the TerminusDB graph of the data product.

### Multi-hop Path Queries

```javascript
triple("v:Subject", "v:Predicate_1", "v:Intermediate")
triple("v:Intermediate", "v:Predicate_2", "v:Object")
```

Here, unification joins two predicates together by requiring that the object of the first edge, is the subject of the second. The variable `"v:Intermediate"` must unify to the same value in both triples, creating a two-hop path.

`Predicate_1` and `Predicate_2` must be two variables because if they are the same, it will only match edges that are have exactly the same predicate, which is not what we want.

### Specific Starting Points

```javascript
triple("terminusdb://data/My_Object", "v:Predicate_1", "v:Intermediate")
triple("v:Intermediate", "v:Predicate_2", "v:Object")
```

This query refers to a specific starting node and searches for every two-hop path starting from this object. The constant “My_Object” provides a concrete anchor point while variables handle the discovery.

If the `@base` of the data product is set to `terminusdb://data/`, the query can be written more simply, as:

```javascript
triple("My_Object", "v:Predicate_1", "v:Intermediate")
triple("v:Intermediate", "v:Predicate_2", "v:Object")
```

## Solution Generation

### Comprehensive Results

When we leverage datalog to search using WOQL, we implicitly ask for all solutions unless specifically restricted using functions like `limit(n,Q)`. This approach ensures you get complete result sets that satisfy your query constraints.

### Tabular Output

The unification process gives us back something that looks quite similar to a table, but it is a list of solutions with bindings for all logical variables that took on a value during the course of searching for the solutions to the query. Unless they are not bound (null). Optional binding `opt().eq("v:var", "value")` enables solution to have a default value if no solution is found when placed at the end of a query.  

Note that queries will be evalated sequentially, which enables optimization to be performed for producing the output.

### Variable Types in WOQL

Unification in variables means each valid value for a variable, as constrained by the totality of the query, will produce a new row in the results. For multiple variables, the system returns the cartesian product of all possible combinations.

### Flexible Function Arguments

WOQL allows variables or constants to be substituted for any argument to all its functions, except for the resource identifier functions like `using`, `with`, `into`, and `from`, which specify graph contexts.

## Example of pattern generation

A simple pattern that shows the pattern generation is the `substr()` predicate:

```javascript
substr(string, before, length, after, subString)
```

Notice that there is only one solution for example 1, and two solutions for example 2, as the possible solutions for the open ended variables will be generated automatically.

### Code: Example 1 of substr

```javascript
substr("string", 2, 2, "v:after", "ri")
```

This returns `2`, as it has 2 characters before the substring `ri`, and we use 2 characters of the substring `ri`.

#### Code: Example 2 of substr

```javascript
substr("string", "v:before", 5, "v:after", "v:subString")
```

Now, this query will return two solutions:
* First solution has `before=0` and `after=1`, and `subString="strin"`
* Second solution has `before=1` and `after=0`, and `subString="tring"`

## Best Practices

### Variable Naming

Choose descriptive variable names that reflect their role in your query. While `"v:X"` works, `"v:PersonName"` or `"v:CompanyId"` makes your queries more readable and maintainable.

Ensure to use a variable generator with a counter or similar mechanism to ensure variables that must be unique are actually unique. Using `v:Predicate` in multiple places will bind to a specific value across the query, which is a common source of inadvertent errors.

### Query Structure

Design your queries to take advantage of unification’s pattern matching capabilities. 

Start with more constrained patterns and let unification discover the connections rather than trying to specify everything explicitly.

### Performance Considerations

Remember that unification evaluates all possible solutions. When working with large graphs, consider using constraints and specific starting points to focus the search space and improve query performance. Also reorder the queries to find the most optimal way for the engine to find relevant solutions.

The engine will try to optimize certain aspects of the query through the built-in optimizations of Prolog, but some optimizations may be required to help the execution plan generator of the database.

Understanding unification empowers you to write more effective WOQL queries that leverage TerminusDB’s graph capabilities. By thinking in terms of variable binding and pattern matching, you can construct queries that efficiently discover the relationships and data patterns hidden within your knowledge graphs.

## Further Reading

Read more:
* the [Datalog Engine Explanation](/docs/what-is-datalog)
* the [WOQL Explanation](/docs/woql-explanation/)
* how to use [WOQL Variables](/docs/woql-variable-generation/)
* how to [Query with WOQL](/docs/how-to-query-with-woql/).