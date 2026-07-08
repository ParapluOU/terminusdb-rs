---
tags:
  - woql
  - reference
  - advanced
title: WOQL Variable Generation Guide (Javascript)
nextjs:
  metadata:
    title: WOQL Variable Generation - vars, vars_unique, Vars, and VarsUnique
    description: Complete guide to the four variable generation methods in the TerminusDB JavaScript client - vars, vars_unique, Vars, and VarsUnique - with practical examples and use cases
    keywords: terminusdb, datalog, javascript, query language, terminusdb query, woql, woql variable generation guide (javascript)
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-variable-generation/
media: []
---

When writing WOQL queries with the JavaScript client, you need to create variables to bind values, pass data between query operations, and return results. TerminusDB provides **four distinct methods** for generating variables, each optimized for different coding styles and use cases.

This guide explains when and how to use `vars()`, `vars_unique()`, `Vars()`, and `VarsUnique()` in the javascript client.

## Quick Overview

| Method | Returns | Naming | Use Case |
|--------|---------|--------|----------|
| `vars()` | Array | Exact names | Simple queries, array destructuring |
| `vars_unique()` | Array | Auto-suffixed | Prevent naming conflicts, local scopes |
| `Vars()` | Object | Exact names | Complex queries, object property access |
| `VarsUnique()` | Object | Auto-suffixed | Complex queries with local scopes |

## Understanding Variable Generation

### Why Do We Need Variables?

In WOQL queries, variables act as placeholders that bind to values during query execution. Variables are used with [unification](/docs/what-is-datalog/#unification-the-key-mechanism), where variables get resolved and bind to values and constrain the query during the unfolding of the query resolution.

```javascript
const [person, name] = WOQL.vars("Person", "Name");

WOQL.select([person, name],
  WOQL.triple(person, "rdf:type", "@schema:Person"),
  WOQL.triple(person, "name", name)
);
```

The variables `person` and `name` will hold the results returned by the query.

### The Four Generation Methods

TerminusDB offers two **access patterns** (array vs. object) and two **naming strategies** (exact vs. unique):

**Access Patterns:**
- **Array-based** (`vars`, `vars_unique`) - Use array destructuring
- **Object-based** (`Vars`, `VarsUnique`) - Use property access with dot notation

**Naming Strategies:**
- **Exact names** (`vars`, `Vars`) - Variables have exactly the names you specify
- **Unique names** (`vars_unique`, `VarsUnique`) - Variables get auto-incremented suffixes


## Object-Based Variables

### Vars() - Variable Object with Exact Names

The `Vars()` function returns an object where each property is a variable. Access variables using dot notation. Names are exacly as specified which is important for getting consistent binding results from queries. These are the best to learn with.

**Syntax:**
```javascript
const v = WOQL.Vars("var1", "var2", "var3");
// Access as: v.var1, v.var2, v.var3
```

**When to Use:**
- Complex queries with many variables
- Better code organization and readability
- Self-documenting code
- Top level query bindings variables (for the resulting tabular structure)
- Avoiding long destructuring assignments

**Example - Clean Query Structure:**
```javascript
const v = WOQL.Vars("person", "name", "email", "age", "city");

const query = WOQL.select([v.person, v.name, v.email],
  WOQL.and(
    WOQL.triple(v.person, "rdf:type", "@schema:Person"),
    WOQL.triple(v.person, "name", v.name),
    WOQL.triple(v.person, "email", v.email),
    WOQL.triple(v.person, "age", v.age),
    WOQL.triple(v.person, "city", v.city),
    WOQL.greater(v.age, 18)
  )
);
```

**Example - Multiple Variable Groups:**
```javascript
// Organize variables by domain
const person = WOQL.Vars("id", "name", "email");
const address = WOQL.Vars("street", "city", "country");
const order = WOQL.Vars("orderId", "total", "date");

const query = WOQL.and(
  WOQL.triple(person.id, "name", person.name),
  WOQL.triple(person.id, "email", person.email),
  WOQL.triple(person.id, "address", address.street),
  WOQL.triple(order.orderId, "customer", person.id),
  WOQL.triple(order.orderId, "total", order.total)
);
```

**Example - Arithmetic with Object Variables:**
```javascript
const v = WOQL.Vars("result1", "result2");

const query = WOQL.and(
  WOQL.eval(WOQL.times(2, 3), v.result1),
  WOQL.eval(WOQL.plus(10, 5), v.result2)
);

await client.query(query);
// Returns: [{ result1: 6, result2: 15 }]
```

**Key Characteristics:**
- ✅ Cleaner code with dot notation
- ✅ Better for complex queries
- ✅ Self-documenting variable access
- ✅ No long destructuring lines
- ✅ Easy to pass variable groups around
- ⚠️ Slightly more verbose initialization

### VarsUnique() - Variable Object with Unique Names

The `VarsUnique()` function combines object-based access with automatic name uniqueness. This is important for deep nested queries where locally scoped variables are important (in combination with the select predicate). For advanced use cases.

**Syntax:**
```javascript
const v = WOQL.VarsUnique("var1", "var2", "var3");
// Access as: v.var1, v.var2, v.var3
// Actual names: var1_N, var2_M, var3_P (with unique suffixes)
```

**When to Use:**
- Complex queries needing local scopes
- Building reusable query components
- Preventing variable conflicts in large codebases
- Nested query operations with `select()`

**Example - Local Scoped Variables:**
```javascript
const v = WOQL.VarsUnique("result1", "result2");

const query = WOQL.limit(100).eval(WOQL.times(2, 3), v.result1);

await client.query(query);
// Returns: [{ result1_21: { "@type": "xsd:decimal", "@value": 6 }}]
// The suffix ensures uniqueness
```

**Example - Nested Query Scopes:**
```javascript
// Outer scope
const outer = WOQL.VarsUnique("person", "name");

// Inner scope - won't conflict even if using same names
const inner = WOQL.VarsUnique("person", "detail");

const query = WOQL.and(
  WOQL.triple(outer.person, "name", outer.name),
  WOQL.select([inner.person, inner.detail],
    WOQL.triple(inner.person, "detail", inner.detail)
  )
);
// outer.person and inner.person have different actual names
```

**Example - Reusable Query Functions:**
```javascript
function createPersonQuery() {
  // Each call gets local unique variables (eq can be used to bind variables together)
  const v = WOQL.VarsUnique("person", "name", "email");
  
  return WOQL.and(
    WOQL.triple(v.person, "rdf:type", "@schema:Person"),
    WOQL.triple(v.person, "name", v.name),
    WOQL.triple(v.person, "email", v.email)
  );
}

// Use multiple times without conflicts
const query1 = createPersonQuery();
const query2 = createPersonQuery();
const combined = WOQL.and(query1, query2); // No variable conflicts!
```

**Key Characteristics:**
- ✅ Clean object-based access
- ✅ Automatic name uniqueness
- ✅ Perfect for reusable components
- ✅ Excellent for nested scopes
- ✅ Prevents conflicts in large queries
- ✅ Combine with `eq()` to connect local and global scoped variables 
- ⚠️ Result keys include suffixes
- ⚠️ Be careful with changing variable names as the object keys will change too
- ⚠️ Less predictable variable names in bindings

## Array-Based Variables

Array-based variables makes for neater WOQL, removing the need for the `v.` prefix with a one to one mapping between WOQL variable names and the javascript variable names.

### vars() - Simple Array Variables

The `vars()` function creates an array of variables with exact names.

**Syntax:**
```javascript
const [var1, var2, var3] = WOQL.vars("varName1", "varName2", "varName3");
```

**When to Use:**
- Simple queries with few variables
- When you want exact control over variable names
- Quick prototypes and examples

**Example - Basic Query:**
```javascript
const [person, name, age] = WOQL.vars("Person", "Name", "Age");

const query = WOQL.select([person, name, age],
  WOQL.triple(person, "rdf:type", "@schema:Person"),
  WOQL.triple(person, "name", name),
  WOQL.triple(person, "age", age)
);

const result = await client.query(query);
// Result bindings use exact names: { Person: ..., Name: ..., Age: ... }
```

**Example - Mathematical Operations:**
```javascript
const [result] = WOQL.vars("result");

// Remember that the mathematical evaluation is performed on the server side!
const query = WOQL.eval(
  WOQL.times(2, 3),
  result
);

await client.query(query);
// Returns: [{ result: { "@type": "xsd:decimal", "@value": 6 }}]
```

**Key Characteristics:**
- ✅ Simple and intuitive
- ✅ Variables have exactly the names you provide
- ✅ Perfect for straightforward queries
- ⚠️ No protection against name collisions
- ⚠️ Manual tracking required in complex queries

### vars_unique() - Uniquely Named Array Variables

The `vars_unique()` function creates variables with automatically incremented suffixes (e.g., `result_1`, `result_2`).

**Syntax:**
```javascript
const [var1, var2] = WOQL.vars_unique("varName1", "varName2");
// Creates: varName1_1, varName2_2 (numbers increment globally)
```

**When to Use:**
- Prevent variable name collisions
- Working with nested queries or multiple scopes
- Building queries programmatically
- Use with `select()` for local variable scoping

**Example - Preventing Name Collisions:**
```javascript
// Without unique variables - potential collision
const [x] = WOQL.vars("x");
const [x2] = WOQL.vars("x"); // Same name! Could cause issues

// With unique variables - guaranteed unique
const [a] = WOQL.vars_unique("x");
const [b] = WOQL.vars_unique("x");
// a might be "x_1", b might be "x_2" - no collision
```

**Example - Local Scope with select():**
```javascript
const [localResult] = WOQL.vars_unique("result");

const query = WOQL.select([localResult],
  WOQL.eval(WOQL.times(2, 3), localResult)
);

await client.query(query);
// Returns: [{ result_23: { "@type": "xsd:decimal", "@value": 6 }}]
// The suffix ensures this doesn't conflict with other "result" variables
```

**Example - Resetting Counter for Testing:**
```javascript
// Reset counter to predictable value (useful for tests)
WOQL.vars_unique_reset_start(0);

const [v1] = WOQL.vars_unique("test"); // test_1
const [v2] = WOQL.vars_unique("test"); // test_2
const [v3] = WOQL.vars_unique("test"); // test_3
```

**Key Characteristics:**
- ✅ Automatic name uniqueness
- ✅ Perfect for nested scopes
- ✅ Prevents accidental variable reuse
- ✅ Can reset counter with `vars_unique_reset_start()`
- ⚠️ Variable names less predictable
- ⚠️ Result keys include suffixes

## Comparison and Decision Guide

### Array vs. Object Access

**Choose Array-based (`vars`, `vars_unique`) when:**
- Writing simple queries with few variables
- You prefer destructuring syntax
- Variable count is small and fixed
- You want minimal syntax overhead

**Choose Object-based (`Vars`, `VarsUnique`) when:**
- Working with many variables
- Building complex, multi-part queries
- You want self-documenting code
- Passing variable groups between functions
- Organizing variables by domain/entity

### Exact vs. Unique Naming

**Choose Exact names (`vars`, `Vars`) when:**
- Working on simple, isolated queries
- You need predictable result key names
- Debugging and you want readable output
- No risk of variable name conflicts
- Building examples or documentation

**Choose Unique names (`vars_unique`, `VarsUnique`) when:**
- Working with nested query scopes
- Using `select()` for variable isolation
- Building reusable query components
- Programmatically generating queries
- Large codebase with multiple developers
- You need guaranteed name uniqueness

## Practical Examples

### Example 1: Simple Data Retrieval
```javascript
// Using vars() - simple and direct
const [person, name] = WOQL.vars("Person", "Name");

const query = WOQL.select([person, name],
  WOQL.triple(person, "rdf:type", "@schema:Person"),
  WOQL.triple(person, "name", name)
);
```

### Example 2: Complex Query with Many Variables
```javascript
// Using Vars() - better organization
const v = WOQL.Vars(
  "person", "name", "email", "phone",
  "address", "city", "country", "postalCode"
);

const query = WOQL.and(
  WOQL.triple(v.person, "rdf:type", "@schema:Person"),
  WOQL.triple(v.person, "name", v.name),
  WOQL.triple(v.person, "email", v.email),
  WOQL.triple(v.person, "phone", v.phone),
  WOQL.triple(v.person, "address", v.address),
  WOQL.triple(v.address, "city", v.city),
  WOQL.triple(v.address, "country", v.country),
  WOQL.triple(v.address, "postalCode", v.postalCode)
);
```

### Example 3: Nested Scopes with select()
```javascript
// Using vars_unique() for isolated scopes
const [outerPerson] = WOQL.vars("person");
const [innerPerson, details] = WOQL.vars_unique("person", "details");

const query = WOQL.and(
  WOQL.triple(outerPerson, "rdf:type", "@schema:Person"),
  WOQL.select([details],
    WOQL.and(
      WOQL.eq(innerPerson, outerPerson),
      WOQL.triple(innerPerson, "details", details)
    )
  )
);
// innerPerson gets unique suffix, preventing confusion
```

### Example 4: Reusable Query Components
```javascript
// Using VarsUnique() for composable queries
function createFilterByAge(minAge) {
  const v = WOQL.VarsUnique("person", "age");
  return WOQL.and(
    WOQL.triple(v.person, "age", v.age),
    WOQL.greater(v.age, minAge)
  );
}

function createFilterByCity(city) {
  const v = WOQL.VarsUnique("person", "personCity");
  return WOQL.and(
    WOQL.triple(v.person, "city", v.personCity),
    WOQL.eq(v.personCity, city)
  );
}

// Compose without conflicts
const query = WOQL.and(
  createFilterByAge(18),
  createFilterByCity("London")
);
// Each function gets unique variable names automatically
```

### Example 5: Mathematical Operations
```javascript
// All four methods work for calculations

// vars - simple array
const [result1] = WOQL.vars("result");
WOQL.eval(WOQL.times(2, 3), result1);

// vars_unique - unique array
const [result2] = WOQL.vars_unique("result");
WOQL.eval(WOQL.plus(10, 5), result2);

// Vars - object access
const v1 = WOQL.Vars("sum", "product");
WOQL.and(
  WOQL.eval(WOQL.plus(5, 3), v1.sum),
  WOQL.eval(WOQL.times(5, 3), v1.product)
);

// VarsUnique - unique object access
const v2 = WOQL.VarsUnique("sum", "product");
WOQL.and(
  WOQL.eval(WOQL.plus(7, 2), v2.sum),
  WOQL.eval(WOQL.times(7, 2), v2.product)
);
```

## Best Practices

### 1. Consistent Naming Conventions

Use descriptive variable names:
```javascript
// ✅ Good - descriptive
const v = WOQL.Vars("employeeName", "employeeEmail", "departmentName");

// ❌ Bad - cryptic
const v = WOQL.Vars("x", "y", "z");
```

### 2. Group Related Variables
```javascript
// ✅ Good - grouped by entity
const employee = WOQL.Vars("id", "name", "email");
const department = WOQL.Vars("id", "name", "budget");

// ❌ Bad - mixed together
const v = WOQL.Vars("empId", "empName", "deptId", "deptName");
```

### 3. Use Unique Variables for Nested Scopes
```javascript
// ✅ Good - unique variables prevent conflicts
function subQuery() {
  const v = WOQL.VarsUnique("person", "name");
  return WOQL.triple(v.person, "name", v.name);
}

// ❌ Risky - exact names might conflict across scopes (in larger code bases)
function subQuery() {
  const v = WOQL.Vars("person", "name");
  return WOQL.triple(v.person, "name", v.name);
}
```

### 4. Reset Counter in Tests
```javascript
// For predictable test results
beforeEach(() => {
  WOQL.vars_unique_reset_start(0);
});

test('query with unique variables', () => {
  const [v1] = WOQL.vars_unique("test"); // Always test_1
  expect(v1.name).toBe("test_1");
});
```

### 5. Choose Appropriate Method for Query Complexity
```javascript
// Simple query and for bindings: use vars()
const [person, name] = WOQL.vars("person", "name");

// Medium complexity and for bindings: use Vars()
const v = WOQL.Vars("person", "name", "email", "age");

// Complex/reusable: prefer to use VarsUnique()
const v = WOQL.VarsUnique("person", "name", "email", "age", "address");
```

## Common Patterns

### Pattern 1: Filter and Return Pattern
```javascript
const v = WOQL.Vars("employee", "name", "salary");

WOQL.select([v.employee, v.name],
  WOQL.and(
    WOQL.triple(v.employee, "name", v.name),
    WOQL.triple(v.employee, "salary", v.salary),
    WOQL.greater(v.salary, 50000)
  )
);
```

### Pattern 2: Join Pattern
```javascript
const v = WOQL.Vars("employee", "empName", "department", "deptName");

WOQL.and(
  WOQL.triple(v.employee, "name", v.empName),
  WOQL.triple(v.employee, "department", v.department),
  WOQL.triple(v.department, "name", v.deptName)
);
```

### Pattern 3: Aggregation Pattern
```javascript
const v = WOQL.Vars("department", "count");

WOQL.group_by([v.department], [v.count],
  WOQL.count(v.employee, v.count),
  WOQL.triple(v.employee, "department", v.department)
);
```

### Pattern 4: Conditional Logic Pattern
```javascript
const v = WOQL.Vars("person", "status", "age");

WOQL.or(
  WOQL.and(
    WOQL.triple(v.person, "age", v.age),
    WOQL.greater(v.age, 65),
    WOQL.eq(v.status, "senior")
  ),
  WOQL.and(
    WOQL.triple(v.person, "age", v.age),
    WOQL.less(v.age, 18),
    WOQL.eq(v.status, "minor")
  )
);
```

## Troubleshooting

### Issue: Variable Name Conflicts

**Problem:** Results are mixed up or unexpected

```javascript
// Both use "result" - might conflict
const [result1] = WOQL.vars("result");
const [result2] = WOQL.vars("result");
```

**Solution:** Use unique variables
```javascript
const [result1] = WOQL.vars_unique("result");
const [result2] = WOQL.vars_unique("result");
// result1 → "result_1", result2 → "result_2"
```

### Issue: Cannot Access Variable Properties

**Problem:** `v.varName is undefined`

```javascript
const [v] = WOQL.vars("person", "name"); // Wrong - returns array
// v is just "person", not an object
```

**Solution:** Use `Vars()` for object access
```javascript
const v = WOQL.Vars("person", "name"); // Correct - returns object
// v.person and v.name are accessible
```

**Problem:** `v.varName is undefined with vio:WOQLSyntaxError`

```javascript
const v = WOQL.Vars("person_renamed"); // Wrong - code is still using v.person
// v.person was used previously, and now getting "Not well formed WOQL JSON-LD"
```

**Solution:** Use care when renaming variables produced by `Vars()`, prefer only using in a local scope
```javascript
const v = WOQL.Vars("person", "name"); // Correct - returns object
// v.person and v.name are accessible
```

### Issue: Unpredictable Variable Names in Tests

**Problem:** Test results vary between runs

```javascript
test('my test', () => {
  const [v] = WOQL.vars_unique("test");
  expect(v.name).toBe("test_1"); // Fails if counter was incremented before
});
```

**Solution:** Reset counter before test
```javascript
test('my test', () => {
  WOQL.vars_unique_reset_start(0);
  const [v] = WOQL.vars_unique("test");
  expect(v.name).toBe("test_1"); // Always passes
});
```

### Issue: Too Many Variables to Destructure

**Problem:** Destructuring line is too long

```javascript
const [v1, v2, v3, v4, v5, v6, v7, v8] = WOQL.vars(...); // Hard to read
```

**Solution:** Use object-based approach
```javascript
const v = WOQL.Vars("v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8");
// Access as v.v1, v.v2, etc.
```

## Migration Guide

### From String Variables to vars()
```javascript
// Old style - strings
WOQL.triple("v:Person", "v:name", "v:Name");

// New style - vars()
const [person, name] = WOQL.vars("Person", "Name");
WOQL.triple(person, name, name);
```

### From vars() to Vars()
```javascript
// Before - array destructuring
const [person, name, email, age, city] = WOQL.vars(...);

// After - object access
const v = WOQL.Vars("person", "name", "email", "age", "city");
// Access with v.person, v.name, etc.
```

### From Vars() to VarsUnique()
```javascript
// Before - potential conflicts
function createQuery() {
  const v = WOQL.Vars("person", "name");
  return WOQL.triple(v.person, "name", v.name);
}

// After - guaranteed unique
function createQuery() {
  const v = WOQL.VarsUnique("person", "name");
  return WOQL.triple(v.person, "name", v.name);
}
```

## Quick Reference

```javascript
// Import
const { vars, vars_unique, Vars, VarsUnique } = WOQL;

// Array-based with exact names
const [a, b] = WOQL.vars("a", "b");

// Array-based with unique names
const [x, y] = WOQL.vars_unique("x", "y"); // x_1, y_2

// Object-based with exact names
const v = WOQL.Vars("person", "name");
// Access: v.person, v.name

// Object-based with unique names
const u = WOQL.VarsUnique("person", "name");
// Access: u.person, u.name (but actual names have suffixes)

// Reset unique counter (for tests)
WOQL.vars_unique_reset_start(0);
```

## Related Documentation

- [JavaScript Client Reference](/docs/javascript/) - Complete JS client API
- [WOQL Basics](/docs/woql-basics/) - Introduction to WOQL queries
- [Unification of Variables in Datalog](/docs/what-is-datalog/#unification-the-key-mechanism) - Understanding how variables are resolved
- [Query Documents with JS](/docs/query-documents/) - Document querying examples
- [Run WOQL Query](/docs/run-woql-query/) - Execute queries with the client
- [Database Path Identifiers](/docs/graph-spec-db-spec-database-path-identifiers/) - Understanding DB_SPEC and GRAPH_SPEC

## Summary

Choose your variable generation method based on your needs:

- **`vars()`** - Simple queries, array destructuring, exact names
- **`vars_unique()`** - Simple queries with name safety, local scopes
- **`Vars()`** - Complex queries, clean object access, exact names  
- **`VarsUnique()`** - Complex reusable queries, object access, guaranteed uniqueness

Start with `vars()` or `Vars()` for straightforward queries. Switch to `vars_unique()` or `VarsUnique()` when building reusable components, working with nested scopes, or preventing variable name conflicts in large applications.

The right choice makes your WOQL queries more readable, maintainable, and bug-free!
