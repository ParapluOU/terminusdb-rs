# TerminusDB WOQL DSL Parser

This crate provides a parser for the WOQL DSL (Domain Specific Language) that converts string representations of WOQL queries into the compositional `terminusdb_woql2::Query` structures.

## DSL Syntax Reference

The DSL uses a compositional, function-based syntax that mirrors the structure of WOQL2. Variables are automatically detected from the `$` prefix without requiring declaration.

### Variables

Variables in WOQL DSL are prefixed with `$` and are automatically recognized:
```text
$Person
$Name
$Age
```

### Values

The DSL supports several value types:
- **Variables**: `$varname`
- **Strings**: `"hello world"`
- **Node references**: `"@schema:Person"` or `"rdf:type"`
- **Numbers**: `42`, `3.14`
- **Booleans**: `true`, `false`
- **Lists**: `[$var1, $var2, "literal"]`

## Query Operations

### Triple Operations

#### triple
Query for triple patterns in the graph.
```text
triple($Subject, $Predicate, $Object)
triple($Person, "@schema:name", $Name)
triple($Person, "rdf:type", "@schema:Person")
```

### Logical Operations

#### and
Conjunction - all queries must succeed.
```text
and(
  query1,
  query2,
  ...
)

and(
  triple($Person, "rdf:type", "@schema:Person"),
  triple($Person, "@schema:age", $Age),
  greater($Age, 18)
)
```

#### or
Disjunction - at least one query must succeed.
```text
or(
  query1,
  query2,
  ...
)

or(
  triple($Person, "@schema:isAdult", true),
  greater($Age, 18)
)
```

#### not
Negation - succeeds if the query fails (provides no bindings).
```text
not(query)

not(triple($Person, "@schema:banned", true))
```

#### opt / optional
Optional query - succeeds even if the inner query fails.
```text
opt(query)
optional(query)

opt(triple($Person, "@schema:nickname", $Nickname))
```

### Control Flow Operations

#### select
Select specific variables from query results.
```text
select(
  [variables],
  query
)

select(
  [$Name, $Age],
  and(
    triple($Person, "@schema:name", $Name),
    triple($Person, "@schema:age", $Age)
  )
)
```

#### distinct
Ensure unique combinations of specified variables.
```text
distinct(
  [variables],
  query
)

distinct(
  [$Name],
  triple($Person, "@schema:name", $Name)
)
```

#### limit
Limit the number of results.
```text
limit(number, query)

limit(10, triple($Person, "@schema:name", $Name))
```

#### start
Skip a number of results.
```text
start(number, query)

start(20, triple($Person, "@schema:name", $Name))
```

#### order_by
Order results by variables.
```text
order_by(
  [order_specs],
  query
)

order_by(
  [asc($Name), desc($Age)],
  and(
    triple($Person, "@schema:name", $Name),
    triple($Person, "@schema:age", $Age)
  )
)
```

#### group_by
Group results by variables and apply aggregate functions.
```text
group_by(
  [group_vars],
  [template_vars],
  query
)

group_by(
  [$Department],
  [$Department, $AvgSalary],
  and(
    triple($Person, "@schema:department", $Department),
    triple($Person, "@schema:salary", $Salary),
    eval(avg($Salary), $AvgSalary)
  )
)
```

### Comparison Operations

#### eq
Test equality between values.
```text
eq($Value1, $Value2)

eq($Name, "John")
eq($Age, 25)
```

#### greater
Test if first value is greater than second.
```text
greater($Value1, $Value2)

greater($Age, 18)
greater($Salary, 50000)
```

#### less
Test if first value is less than second.
```text
less($Value1, $Value2)

less($Age, 65)
less($Price, 100.0)
```

### Type Operations

#### isa
Check if a value is of a specific type.
```text
isa($Value, type)

isa($Person, "@schema:Person")
isa($Age, "xsd:integer")
```

#### type_of
Get the type of a value.
```text
type_of($Value, $Type)

type_of($Person, $Type)
```

#### subsumption
Check type subsumption relationships.
```text
subsumption($SubType, $SuperType)

subsumption("@schema:Employee", "@schema:Person")
```

### String Operations

#### concat
Concatenate a list of strings.
```text
concat([strings], $Result)

concat(["Hello", " ", "World"], $Greeting)
concat([$FirstName, " ", $LastName], $FullName)
```

#### substring
Extract substring by position and length.
```text
substring($String, $Before, $Length, $After, $Substring)

substring("Hello World", 0, 5, 6, $Sub)
```

#### trim
Remove whitespace from string ends.
```text
trim($Untrimmed, $Trimmed)

trim("  hello  ", $Clean)
```

#### upper
Convert string to uppercase.
```text
upper($Mixed, $Upper)

upper("hello", $Uppercase)
```

#### lower
Convert string to lowercase.
```text
lower($Mixed, $Lower)

lower("HELLO", $Lowercase)
```

#### regexp
Match string against regular expression.
```text
regexp(pattern, $String, $Result)

regexp("[0-9]+", $Phone, $Matches)
```

### Arithmetic Operations

#### eval
Evaluate arithmetic expressions.
```text
eval(expression, $Result)

eval(plus($X, $Y), $Sum)
eval(minus($A, $B), $Difference)
eval(times($P, $Q), $Product)
eval(div($Num, $Den), $Quotient)
```

Arithmetic expressions can be nested:
```text
eval(plus(times(2, $X), 1), $Result)
```

### Collection Operations

#### sum
Sum a list of numeric values.
```text
sum([values], $Result)

sum([1, 2, 3, 4], $Total)
sum([$Value1, $Value2, $Value3], $Sum)
```

#### count
Count solutions from a query.
```text
count(query, $Count)

count(triple($Person, "rdf:type", "@schema:Person"), $PersonCount)
```

### Document Operations

#### read_document
Read a document by ID.
```text
read_document($ID, $Document)

read_document("Person/john-doe", $PersonData)
```

#### insert_document
Insert a new document.
```text
insert_document($Document, $ID)

insert_document($NewPerson, $PersonID)
```

#### update_document
Update an existing document.
```text
update_document($Document)

update_document($UpdatedPerson)
```

#### delete_document
Delete a document by ID.
```text
delete_document($ID)

delete_document("Person/john-doe")
```

### Path Operations

#### path
Find paths through the graph matching a pattern.
```text
path($Start, pattern, $End, $Path)
```

Path patterns include:
- **pred**: Follow a specific predicate
  ```text
  path($Person, pred("@schema:knows"), $Friend)
  ```

- **inv**: Follow inverse of predicate
  ```text
  path($Child, inv("@schema:parent"), $Parent)
  ```

- **star**: Zero or more repetitions
  ```text
  path($Person, star(pred("@schema:knows")), $Connection)
  ```

- **plus**: One or more repetitions
  ```text
  path($Person, plus(pred("@schema:manages")), $Report)
  ```

- **seq**: Sequence of patterns
  ```text
  path($A, seq(pred("@schema:knows"), pred("@schema:likes")), $C)
  ```

- **or**: Alternative patterns
  ```text
  path($Person, or(pred("@schema:knows"), pred("@schema:likes")), $Other)
  ```

## Complex Example

```text
select(
  [$Name, $Department, $AvgAge],
  and(
    distinct(
      [$Department],
      triple($Person, "@schema:department", $Department)
    ),
    group_by(
      [$Department],
      [$AvgAge],
      and(
        triple($Person, "@schema:department", $Department),
        triple($Person, "@schema:age", $Age),
        eval(avg($Age), $AvgAge)
      )
    ),
    triple($Department, "@schema:name", $Name)
  )
)
```

## Usage

```rust
use terminusdb_woql_dsl::parse_woql_dsl;

let dsl = r#"
select(
  [$Name],
  triple($Person, "@schema:name", $Name)
)
"#;

let query = parse_woql_dsl(dsl).unwrap();
// Variables $Person and $Name are automatically detected
```

## Notes

- Variables (identifiers starting with `$`) are automatically recognized
- The parser converts DSL strings into `terminusdb_woql2::Query` structures
- All operations mirror the WOQL2 query types
- Error messages indicate parsing failures with position information