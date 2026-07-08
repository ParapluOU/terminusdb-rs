---
tags:
  - woql
  - cookbook
  - intermediate
title: "WOQL Cookbook: Finding and matching datatypes"
nextjs:
  metadata:
    title: "WOQL Cookbook: Finding and matching datatypes"
    description: Examples of WOQL query patterns with datatypes using the WOQL datalog type_of() in three different ways
    keywords: woql, query, datalog, cookbook, declarative logic
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/cookbook-woql-query-patterns/
media: []
---

This page is intended to show how to accomplish goals with WOQL. Before really "getting" WOQL, it feels counter-intuitive as solutions are generated using patterns.

Logicians and people having used prolog, datalog and similar declarative languages will find it easier to understand the core concepts of unification and generator patterns and we hope the examples on this page will help show common solutions.

## How the examples on this page work

Most of the work in TerminusDB happens with data that is stored in the instance graph. WOQL is not limited to processing stored graph information, it is also possible to process supplied CSV files and build patterns using the `member()` predicate.

The examples on the first section of the page are kept as simple as possible, to enable running them without schema or instance data. The second section will require a schema and instance data with some instructions on how to set it up.

You are expected to have instance data to query and a basic understanding of querying triples of the graph and processing documents with knowledge of datatypes, sets, lists, arrays etc.

## Variables

Variable unification is core to the WOQL engine. Read up on [unification](/docs/what-is-datalog/#unification-the-key-mechanism) if you are unsure, and play with the examples on this page to learn.

## Understanding value types

All values that are processed in TerminusDB are of a certain type. Sometimes it's important to either match a type, understand what datatype a value has, or to match elements in a variable to a datatype.

Thanks to pattern matching this can be done with the very same predicate, used in multiple ways.

### Code: Which type a value has

The `type_of` predicate can be used to bind a variable to the type of a value, `v:datatype` in this example. This is useful when you want to match values with specific types, such as specific classes of integers, or match all kinds of decimal numbers, floats and doubles.

```woql
and(
  member("v:list", [literal("MyString", "xsd:string"), literal(1, "xsd:integer")]),
  type_of("v:list", "v:datatype")
)
```

The above yields:

{% table %}

- list
- datatype

---

- MyString
- xsd:string

---

- 1
- xsd:integer

{% /table %}

### Code: Which value is a string?

Here we use `type_of()` in a different way, where we instead bind the type parameter to a specific type, `xsd:string`, to constrain the allowed values in `v:list` for the solutions to return.

```woql
and(
  member("v:list", [
    literal("MyString1", "xsd:string"),
    literal(1, "xsd:integer"),
    literal("MyString2", "xsd:string")
  ]),
  type_of("v:list", "xsd:string")
)
```

The above yields:

{% table %}

- list

---

- MyString1

---

- MyString2

{% /table %}

### Code: Is a specific value a string?

In the last example, which is a bit contrived, we check if the literal string is a string, and bind a variable to a value if the comparison is true.

What is important in this example is that all statements of the `and()` predicate must resolve to true for it to be a solution. Thus, we know that there must be equality for a solution to be returned.

To make this a bit more interesting, we use optionality to offer two variables that are both optional so that only one solution is returned, where one comparison is true, and the other isn't.

In order to satisfy the `and()` predicate, the solution result of the literal check needs to be inverted using the `not()` operator, and then the `eq()` operator sets the variable `v:is_integer` to be `false` as the result of the comparison is not true.

```woql
and(
  opt().and(
    type_of(literal("", "xsd:string"), "xsd:string"),
    eq("v:is_string", true)
  ),
  opt().and(
    not().type_of(literal("", "xsd:string"), "xsd:integer"),
    eq("v:is_integer", false)
  )
)
```

The above yields:

{% table %}

- is_string
- is_integer

---

- true
- false

{% /table %}

## Conclusion

WOQL predicates can be used in many ways. Through clever use of them, problems that are hard to solve in other ways, often get elegant solutions using WOQL, thanks to the powerful pattern matching capabilities of WOQL and the flexible use of variables. 

Traditional database query languages like SQL are limited to the data and the columns that are available, which makes it hard to write solutions with declarative logic.

The flexibility to use the logical predicates in multiple ways makes it practical to write solutions using declarative logic.