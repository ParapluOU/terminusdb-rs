---
title: Numeric Types and Precision Reference
nextjs:
  metadata:
    title: Numeric Types and Precision Reference
    description: Understanding xsd:decimal, xsd:double, xsd:float arithmetic operations and precision handling in TerminusDB
    keywords: TerminusDB, xsd:decimal, xsd:double, xsd:float, numeric precision, arithmetic, IEEE 754
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/numeric-precision-reference/
media: []
tags:
  - typescript
  - documents
  - reference
---

TerminusDB provides precise control over numeric types and arithmetic operations, following W3C XSD standards and Prolog's natural semantics. This guide explains how numeric types work, when precision is preserved, and how to choose the right type for your use case.

--> Valid as of the 11.2 release.

## Numeric Type Overview

TerminusDB supports four primary numeric types for storage, and performs arithmetic operations using two families of xsd:decimal and xsd:double:

{% table %}

- **Type**
- **Storage**
- **Precision**
- **Use Case**

---

- `xsd:decimal`
- Rational number
- **Exact** (arbitrary size and precision, capped at 20 decimals)
- Financial calculations, exact arithmetic

---

- `xsd:integer`
- Rational number (subset)
- **Exact** (arbitrary size)
- Counting, indexing, exact integers

---

- `xsd:double`
- IEEE 754 64-bit float
- **Approximate** (15-17 digits)
- Scientific calculations, measurements

---

- `xsd:float`
- IEEE 754 32-bit float
- **Approximate** (6-9 digits)
- Low-precision measurements

{% /table %}

## Type Inference Rules

When performing arithmetic operations, TerminusDB follows **Prolog's natural semantics**, where floating point numbers are "contagious."

### Pure Type Operations during arithmetic

{% table %}

- **Operation**
- **Result Type**
- **Example**

---

- `xsd:decimal + xsd:decimal`
- `xsd:decimal`
- Exact: `0.1 + 0.2 = 0.3`

---

- `xsd:integer + xsd:integer`
- `xsd:decimal`
- Exact: `5 + 3 = 8`

---

- `xsd:double + xsd:double`
- `xsd:double`
- IEEE 754: `0.1 + 0.2 = 0.30000000000000004`

---

- `xsd:float + xsd:float`
- `xsd:double`
- Promoted to double precision

{% /table %}

### Mixed Type Operations (Floats Are Contagious)

**Rule**: If ANY operand is `xsd:double` or `xsd:float`, the result is `xsd:double`.

{% table %}

- **Operation**
- **Result Type**
- **Rationale**

---

- `xsd:double + xsd:decimal`
- `xsd:double`
- Float "infects" the operation

---

- `xsd:double + xsd:integer`
- `xsd:double`
- Float takes precedence

---

- `xsd:float + xsd:decimal`
- `xsd:double`
- Float promoted to double

{% /table %}

{% callout type="note" %}
**Why are floats contagious?**
This follows Prolog's arithmetic semantics, ensuring consistent and predictable behavior. When you mix approximate (float) with exact (rational) types, the result must be approximate since the float has already lost precision during parsing.
{% /callout %}

## Division Operations

Division behavior depends on operand types:

{% table %}

- **Operands**
- **Operator Used**
- **Result**

---

- Both `xsd:decimal` or `xsd:integer`
- `rdiv` (rational division)
- Exact: `1/3` stays as `1 rdiv 3` during evaluation, stored as xsd:decimal

---

- Any `xsd:double` or `xsd:float`
- `/` (IEEE 754 division)
- IEEE 754: `1.0/3.0 = 0.3333333333333333`

{% /table %}

### Examples

Looking at the pure WOQL AST helps show the evaluation structure.

```javascript
// Rational division (exact)
{
  "@type": "Eval",
  "expression": {
    "@type": "Divide",
    "left": { "@type": "ArithmeticValue", "data": { "@type": "xsd:decimal", "@value": "1" } },
    "right": { "@type": "ArithmeticValue", "data": { "@type": "xsd:decimal", "@value": "3" } }
  },
  "result": { "@type": "ArithmeticValue", "variable": "Result" }
}
// Result: { "@type": "xsd:decimal", "@value": "0.3333333..." } (processed as 1/3 rational, stored as decimal if persisted)

// IEEE 754 division (approximate)
{
  "@type": "Eval",
  "expression": {
    "@type": "Divide",
    "left": { "@type": "ArithmeticValue", "data": { "@type": "xsd:double", "@value": "1.0" } },
    "right": { "@type": "ArithmeticValue", "data": { "@type": "xsd:double", "@value": "3.0" } }
  },
  "result": { "@type": "ArithmeticValue", "variable": "Result" }
}
// Result: { "@type": "xsd:double", "@value": 0.3333333333333333 }
```

## Precision Loss: When and Why

{% callout type="warning" title="Critical Understanding" %}
Precision is lost at *parse time*, not during arithmetic!
{% /callout %}

### Example: The 0.1 + 0.2 Problem

```javascript
// Using xsd:double (precision lost at parse time)
{
  "@type": "Eval",
  "expression": {
    "@type": "Plus",
    "left": { "@type": "ArithmeticValue", "data": { "@type": "xsd:double", "@value": "0.1" } },
    "right": { "@type": "ArithmeticValue", "data": { "@type": "xsd:double", "@value": "0.2" } }
  },
  "result": { "@type": "ArithmeticValue", "variable": "Result" }
}
// Result: { "@type": "xsd:double", "@value": 0.30000000000000004 }
// Why? "0.1" cannot be represented exactly in binary floating point!

// Using xsd:decimal (exact)
{
  "@type": "Eval",
  "expression": {
    "@type": "Plus",
    "left": { "@type": "ArithmeticValue", "data": { "@type": "xsd:decimal", "@value": "0.1" } },
    "right": { "@type": "ArithmeticValue", "data": { "@type": "xsd:decimal", "@value": "0.2" } }
  },
  "result": { "@type": "ArithmeticValue", "variable": "Result" }
}
// Result: { "@type": "xsd:decimal", "@value": "0.3" } (exact!)
```

### Why Mixing Doesn't Help

```javascript
// Mixed types: precision ALREADY lost
{
  "@type": "Eval",
  "expression": {
    "@type": "Plus",
    "left": { "@type": "ArithmeticValue", "data": { "@type": "xsd:double", "@value": "0.1" } },
    "right": { "@type": "ArithmeticValue", "data": { "@type": "xsd:decimal", "@value": "0.2" } }
  },
  "result": { "@type": "ArithmeticValue", "variable": "Result" }
}
// Result: { "@type": "xsd:double", "@value": 0.30000000000000004 }
// Why? The xsd:double(0.1) was already imprecise before arithmetic!
```

## Best Practices

### Use `xsd:decimal` When

- Financial calculations (money, prices, invoices)
- Exact arithmetic required (inventory, quantities)
- Precision matters more than performance
- Regulatory compliance required

```javascript
// Good: Financial calculation, make sure to handle JSON correctly
// Be especially careful with javascript, you can always send as strings
// Ensure to use the "reviver" pattern and leverage Decimal.js or equivalent!
{
  price: { "@type": "xsd:decimal", "@value": "99.99" },
  tax: { "@type": "xsd:decimal", "@value": "0.08" },
  total: { "@type": "xsd:decimal", "@value": "107.99" }
}
```

### Use `xsd:double` When

- Scientific measurements
- Statistical calculations
- Approximate values (sensor readings)
- Performance-critical operations
- Interoperability with IEEE 754 systems

```javascript
// Good: Scientific measurement
{
  temperature: { "@type": "xsd:double", "@value": 98.6 },
  coordinates: {
    latitude: { "@type": "xsd:double", "@value": 51.5074 },
    longitude: { "@type": "xsd:double", "@value": -0.1278 }
  }
}
```

### Use `xsd:integer` When

- Counting (items, users, events)
- IDs and identifiers
- Array indices
- Exact whole numbers

```javascript
// Good: Counting
{
  quantity: { "@type": "xsd:integer", "@value": "42" },
  userId: { "@type": "xsd:integer", "@value": "12345" }
}
```

## Variable Arithmetic

Variables preserve their types during arithmetic:

```javascript
// Variables maintain their types
{
  "@type": "And",
  "and": [
    {
      "@type": "Equals",
      "left": { "variable": "x" },
      "right": { "data": { "@type": "xsd:double", "@value": "0.1" } }
    },
    {
      "@type": "Equals",
      "left": { "variable": "y" },
      "right": { "data": { "@type": "xsd:double", "@value": "0.2" } }
    },
    {
      "@type": "Eval",
      "expression": {
        "@type": "Plus",
        "left": { "variable": "x" },
        "right": { "variable": "y" }
      },
      "result": { "variable": "result" }
    }
  ]
}
// Result: { "@type": "xsd:double", "@value": 0.30000000000000004 }
// The variables x and y are xsd:double, so result is xsd:double
```

## Common Pitfalls

### Pitfall 1: Assuming Mixed Types Preserve Precision

❌ **Wrong Assumption:**
```javascript
// "If I add xsd:double to xsd:decimal, I'll get exact result"
xsd:double(0.1) + xsd:decimal(0.2) // Hoping for 0.3
```

✅ **Reality:**
```javascript
// Result is 0.30000000000000004 (xsd:double)
// Precision was ALREADY lost when parsing "0.1" as xsd:double
```

### Pitfall 2: Using xsd:double for Money

❌ **Wrong:**
```javascript
{
  price: { "@type": "xsd:double", "@value": "19.99" },
  tax: { "@type": "xsd:double", "@value": "0.08" }
}
// Can lead to rounding errors in financial calculations!
```

✅ **Right:**
```javascript
{
  price: { "@type": "xsd:decimal", "@value": "19.99" },
  tax: { "@type": "xsd:decimal", "@value": "0.08" }
}
```

### Pitfall 3: Comparing Floats for Equality

❌ **Problematic:**
```javascript
0.1 + 0.2 == 0.3  // false with xsd:double!
```

✅ **Better:**
```javascript
// Use xsd:decimal for exact comparisons
0.1 + 0.2 == 0.3  // true with xsd:decimal!
```

## Technical Details

### Internal Representation

{% table %}

- **Type**
- **Prolog Processing**
- **Storage**
- **Example**

---

- `xsd:decimal`
- Rational (`numerator rdiv denominator`)
- TerminusDB Storage: 0.3
- `3r10` (represents 0.3)

---

- `xsd:decimal`
- Rational (denominator = 1)
- TerminusDB Storage: 42 (not 42.0)
- `42` 

---

- `xsd:integer`
- Rational (denominator = 1)
- TerminusDB Storage: 42
- `42` 

---

- `xsd:double`
- IEEE 754 float (64 bit)
- TerminusDB Storage: 0.30000000000000004 (64 bit)
- `0.30000000000000004`

{% /table %}

### Type Checking at Runtime

TerminusDB uses Prolog's type predicates to determine result types:

```prolog
% Type inference (simplified)
infer_result_type(Value, 'xsd:decimal') :-
    rational(Value),  % Check if it's a rational number
    !.
infer_result_type(_Value, 'xsd:double').  % Otherwise it's a float
```


### Comparing Different Types

When comparing different types, use `typecast` to convert one or both operands to the same type family (xsd:decimal, xsd:integer, xsd:double or xsd:float).

## Performance Considerations

{% table %}

- **Aspect**
- **xsd:decimal**
- **xsd:double**

---

- **Storage**
- Variable memory (rational representation)
- 8 bytes fixed

---

- **Arithmetic Speed**
- Slower (rational arithmetic)
- Faster (hardware floats)

---

- **Precision**
- ✅ Exact (arbitrary precision, capped at 20 decimals)
- ⚠️ Approximate (15-17 digits precision)

---

- **Range**
- ✅ Unlimited
- ±1.7 × 10^308

{% /table %}

### When Performance Matters

For performance-critical operations with millions of calculations:

- Use `xsd:double` if approximate results are acceptable
- Use `xsd:decimal` if precision is non-negotiable
- Consider pre-rounding to reduce decimal precision when possible

## Migration Guide

### Converting Existing Data

```javascript
// From xsd:double to xsd:decimal (be aware of precision loss)
{
  "@type": "Cast",
  "value": { "@type": "xsd:double", "@value": 0.30000000000000004 },
  "type": "xsd:decimal"
}
// Result: Best-fit decimal representation

// From xsd:decimal to xsd:double (safe but loses exactness)
{
  "@type": "Cast",
  "value": { "@type": "xsd:decimal", "@value": "0.3" },
  "type": "xsd:double"
}
// Result: 0.3 (may not be exact due to IEEE 754)
```

## Summary

{% callout type="note" title="Key Takeaways" %}
1. **Use `xsd:decimal` for exact arithmetic** (especially money)
2. **Floats are contagious** - any float makes the result a float
3. **Precision is lost at parse time**, not during arithmetic
4. **Division uses `rdiv` for rationals**, `/` for floats
5. **Follow Prolog's natural semantics** - consistent and predictable
{% /callout %}

## See Also

- [Supported Data Types](/docs/dfrnt-data-types) - Complete list of XSD types
- [WOQL Math Queries](/docs/maths-based-queries-in-woql) - Arithmetic operations in WOQL
- [Schema Reference Guide](/docs/schema-reference-guide) - Defining numeric properties in schemas
