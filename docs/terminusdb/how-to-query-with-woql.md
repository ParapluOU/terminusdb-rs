---
tags:
  - woql
  - tutorial
  - beginner
title: How to query with datalog (WOQL)
nextjs:
  metadata:
    title: How to Query with WOQL Datalog
    keywords: woql query datalog tutorial javascript
    description: Hands-on learn by doing tutorial to learn the WOQL Datalog language with working JavaScript examples
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/how-to-query-with-woql/
media: []
---

WOQL (Web Object Query Language) is a declarative query language built on datalog foundations. This tutorial teaches you WOQL through hands-on JavaScript examples that you can run against your local TerminusDB instance.

Individual deep dive sections:
* [WOQL Basics](/docs/woql-basics/)
* [WOQL Add Docs](/docs/add-documents-with-woql/)
* [WOQL Edit Docs](/docs/edit-documents-with-woql/)
* [WOQL Delete Docs](/docs/delete-documents-with-woql/)
* [WOQL Read Docs](/docs/read-documents-with-woql/)
* [WOQL Filter](/docs/filter-with-woql/)
* [WOQL Order By](/docs/order-by-with-woql/)
* [WOQL Query Arrays](/docs/query-arrays-and-sets-in-woql/)
* [WOQL Group Results](/docs/group-query-results/)
* [WOQL Path Queries](/docs/path-queries-in-woql/)
* [WOQL Math Queries](/docs/maths-based-queries-in-woql/)
* [WOQL Schema Queries](/docs/schema-queries-with-woql/)

## What You'll Learn

By the end of this tutorial, you'll understand:

- How declarative thinking differs from imperative programming
- Variable binding and unification in WOQL
- Reading, filtering, and transforming data
- Building complex queries step by step

## Prerequisites

- TerminusDB server running locally (default: `http://localhost:6363`) - See [Docker installation guide](/docs/install-terminusdb-as-a-docker-container/)
- [Node.js](https://nodejs.org/en/download/) installed on your system
- Basic JavaScript knowledge

---

Complete this tutorial to learn WOQL through hands-on JavaScript examples. Check off each step as you complete it!

{% task-heading id="woql-setup" number="Setup" %}
Create Your Tutorial File
{% /task-heading %}

First, create a new directory and install the TerminusDB JavaScript client:

```bash
mkdir woql-tutorial
cd woql-tutorial
npm init -y
npm install terminusdb
```

Create a file named `woql-tutorial.js` and copy all the examples below into it. The `main()` function accepts arguments so you can easily run specific steps or all steps.

---

## The Complete Tutorial Code

Below is the complete tutorial code. Copy this into your `woql-tutorial.js` file and follow along section by section. We suggest copying each section step by step for maximum learning. 

On macOS, you can use the `pbpaste` command to paste the code into your file, like this: `pbpaste > woql-tutorial.js`. On other platforms, you can use `cat > woql-tutorial.js` and then paste the code into the file.

Or just use a standard text editor.

```javascript
const TerminusClient = require('terminusdb');
const { WOQL } = TerminusClient;

// Configuration
const SERVER_URL = 'http://localhost:6363';
const ACCOUNT = 'admin';
const DB_NAME = 'woql_tutorial';
const DB_LABEL = 'WOQL Tutorial Database';
const DB_DESCRIPTION = 'A hands-on tutorial database for learning WOQL';

// Initialize client with API key
const client = new TerminusClient.WOQLClient(SERVER_URL, {
  user: ACCOUNT,
  organization: ACCOUNT,
  key: 'root' // Default password - change if you've set a different password
});

/**
 * Main Function
 * 
 * Accepts command-line arguments to control which steps to run.
 * 
 * Usage:
 *   node woql-tutorial.js           # Run all steps
 *   node woql-tutorial.js setup     # Run steps 1-3 (setup only)
 *   node woql-tutorial.js queries   # Run steps 4-11 (queries only)
 *   node woql-tutorial.js step5     # Run specific step
 *   node woql-tutorial.js step5-8   # Run range of steps
 */
async function main() {
  const args = process.argv.slice(2);
  const mode = args[0] || 'all';

  console.log('='.repeat(60));
  console.log('    WOQL Tutorial: Learn by Doing');
  console.log('='.repeat(60));

  // Map of available steps
  const steps = {
    1: step1_initializeDatabase,
    2: step2_defineSchema,
    3: step3_insertDocuments,
    4: step4_readAllDocuments,
    5: step5_filterByAge,
    6: step6_multipleVariables,
    7: step7_complexAnd,
    8: step8_orQuery,
    9: step9_optionalData,
    10: step10_orderAndLimit,
    11: step11_groupAndAggregate,
    12: step12_deleteDocuments
  };

  try {
    let stepsToRun = [];

    if (mode === 'all') {
      // Run all steps except delete
      stepsToRun = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    } else if (mode === 'setup') {
      // Run setup steps only
      stepsToRun = [1, 2, 3];
    } else if (mode === 'queries') {
      // Run query steps only
      stepsToRun = [4, 5, 6, 7, 8, 9, 10, 11];
    } else if (mode.startsWith('step')) {
      // Run specific step(s)
      const stepMatch = mode.match(/step(\d+)(?:-(\d+))?/);
      if (stepMatch) {
        const start = parseInt(stepMatch[1]);
        const end = stepMatch[2] ? parseInt(stepMatch[2]) : start;
        for (let i = start; i <= end; i++) {
          if (steps[i]) stepsToRun.push(i);
        }
      }
    } else {
      console.log('\nUsage:');
      console.log('  node woql-tutorial.js           # Run all steps');
      console.log('  node woql-tutorial.js setup     # Run setup (steps 1-3)');
      console.log('  node woql-tutorial.js queries   # Run queries (steps 4-11)');
      console.log('  node woql-tutorial.js step5     # Run specific step');
      console.log('  node woql-tutorial.js step5-8   # Run range of steps');
      process.exit(0);
    }

    console.log(`\nRunning steps: ${stepsToRun.join(', ')}\n`);

    for (const stepNum of stepsToRun) {
      await steps[stepNum]();
    }

    console.log('\n' + '='.repeat(60));
    console.log('    Tutorial Complete! 🎉');
    console.log('='.repeat(60));
    console.log('\nNext steps:');
    console.log('  - Modify the queries to experiment');
    console.log('  - Try adding your own functions');
    console.log('  - Explore the advanced guides below');
    console.log('');

  } catch (error) {
    console.error('\n❌ Tutorial failed:', error);
    process.exit(1);
  }
}

/**
 * STEP 1: Initialize Database
 * 
 * This function creates the database if it doesn't exist.
 * This is your entry point - run this first!
 */
async function step1_initializeDatabase() {
  console.log('\n=== STEP 1: Initialize Database ===');
  
  try {
    // Check if database exists
    const databases = await client.getDatabases();
    const dbExists = databases.some(db => db.name === DB_NAME || db.id === DB_NAME);
    
    if (!dbExists) {
      console.log(`Creating database: ${DB_NAME}`);
      await client.createDatabase(DB_NAME, {
        label: DB_LABEL,
        comment: DB_DESCRIPTION,
        schema: true
      });
      console.log('✓ Database created successfully');
    } else {
      console.log('✓ Database already exists');
    }
    
    // Connect to the database
    client.db(DB_NAME);
    console.log('✓ Connected to database');
    
  } catch (error) {
    console.error('Error initializing database:', error.message);
    throw error;
  }
}

/**
 * STEP 2: Define Schema
 * 
 * We'll create a simple schema with Person documents.
 * Each Person has a name, age, and optional city.
 */
async function step2_defineSchema() {
  console.log('\n=== STEP 2: Define Schema ===');

  try {
    client.db(DB_NAME);

    // Check if Person class already exists
    try {
      const existingSchema = await client.getDocument({ id: 'Person', graph_type: 'schema' });
      if (existingSchema) {
        console.log('✓ Schema already exists');
        console.log('  - Person class with name, age, city, email');
        return;
      }
    } catch (e) {
      // Schema doesn't exist, continue to create it
    }

    const schema = {
      "@type": "Class",
      "@id": "Person",
      "@key": {
        "@type": "Random"
      },
      "name": "xsd:string",
      "age": "xsd:integer",
      "city": {
        "@type": "Optional",
        "@class": "xsd:string"
      },
      "email": {
        "@type": "Optional",
        "@class": "xsd:string"
      }
    };

    await client.addDocument(schema, { graph_type: "schema" });
    console.log('✓ Schema defined successfully');
    console.log('  - Person class with name, age, city, email');

  } catch (error) {
    console.error('Error defining schema:', error.message);
    throw error;
  }
}

/**
 * STEP 3: Insert Sample Documents
 * 
 * Let's add some people to our database.
 * We'll use different ages and cities for interesting queries later.
 */
async function step3_insertDocuments() {
  console.log('\n=== STEP 3: Insert Sample Documents ===');

  try {
    client.db(DB_NAME);

    // Check if documents already exist
    const existing = await client.getDocument({ type: "Person" });
    const existingDocs = Array.isArray(existing) ? existing : (existing ? [existing] : []);

    if (existingDocs.length >= 5) {
      console.log(`✓ Documents already exist (${existingDocs.length} found)`);
      console.log('  Skipping insertion to avoid duplicates');
      return;
    }

    const people = [
      {
        "@id": "Person/1",
        "@type": "Person",
        "name": "Alice Johnson",
        "age": 28,
        "city": "New York",
        "email": "alice@example.com"
      },
      {
        "@id": "Person/2",
        "@type": "Person",
        "name": "Bob Smith",
        "age": 35,
        "city": "San Francisco",
        "email": "bob@example.com"
      },
      {
        "@id": "Person/3",
        "@type": "Person",
        "name": "Carol Williams",
        "age": 28,
        "city": "New York"
      },
      {
        "@id": "Person/4",
        "@type": "Person",
        "name": "David Brown",
        "age": 42,
        "city": "Austin",
        "email": "david@example.com"
      },
      {
        "@id": "Person/5",
        "@type": "Person",
        "name": "Eve Davis",
        "age": 31,
        "city": "San Francisco"
      }
    ];

    for (const person of people) {
      await client.addDocument(person);
      console.log(`✓ Inserted: ${person.name}`);
    }

    console.log(`\n✓ Total documents inserted: ${people.length}`);

  } catch (error) {
    console.error('Error inserting documents:', error.message);
    throw error;
  }
}

/**
 * STEP 4: Read All Documents
 * 
 * Let's learn our first WOQL query!
 * 
 * CONCEPT: Variable Binding and Type Checking
 * - Variables in WOQL start with "v:"
 * - isa() checks if a document is of a specific type
 * - read_document() reads the full document into a variable
 * - select() chooses which variables to return
 */
async function step4_readAllDocuments() {
  console.log('\n=== STEP 4: Read All Documents ===');

  try {
    client.db(DB_NAME);

    // Get all Person documents using WOQL
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.read_document("v:docid", "v:doc")
    );

    const results = await client.query(query);

    console.log(`\n✓ Found ${results.bindings.length} documents:`);
    results.bindings.forEach((binding, index) => {
      const doc = binding.doc;  // Note: binding keys don't include "v:" prefix
      console.log(`  ${index + 1}. ${doc.name} (${doc.age} years old) - ${doc.city || 'no city'}`);
    });

  } catch (error) {
    console.error('Error reading documents:', error.message);
    throw error;
  }
}

/**
 * STEP 5: Filter Documents with triple()
 * 
 * CONCEPT: Property Filtering with Triples
 * - triple() matches property values
 * - Use literal() to create typed values for matching
 * - literal(value, type) ensures type compatibility with stored data
 * - Combines isa() for type checking with property filters
 * - read_document() retrieves the full document after filtering
 */
async function step5_filterByAge() {
  console.log('\n=== STEP 5: Filter by Age ===');

  try {
    client.db(DB_NAME);

    // Find all people aged 28 using WOQL
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "age", WOQL.literal(28, "xsd:integer")),
      WOQL.read_document("v:docid", "v:doc")
    );

    const results = await client.query(query);

    console.log(`✓ Found ${results.bindings.length} people aged 28:`);
    results.bindings.forEach(binding => {
      const doc = binding.doc;
      console.log(`  - ${doc.name} from ${doc.city || 'unknown city'}`);
    });

  } catch (error) {
    console.error('Error filtering documents:', error.message);
    throw error;
  }
}

/**
 * STEP 6: Extract Multiple Properties with Triples
 * 
 * CONCEPT: Multiple Property Extraction
 * - Use multiple triple() calls to extract different properties
 * - Variables bind to actual values from the document
 * - Can filter to only documents with all required properties
 */
async function step6_multipleVariables() {
  console.log('\n=== STEP 6: Extract Specific Fields ===');

  try {
    client.db(DB_NAME);

    // Get name, age, and city for all people who have a city
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.triple("v:docid", "age", "v:age"),
      WOQL.triple("v:docid", "city", "v:city")
    );

    const results = await client.query(query);

    console.log(`✓ Found ${results.bindings.length} people with cities:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}, age ${binding.age['@value']}, lives in ${binding.city['@value']}`);
    });

  } catch (error) {
    console.error('Error with multiple variables:', error.message);
    throw error;
  }
}

/**
 * STEP 7: Complex Filtering with Comparisons
 * 
 * CONCEPT: Combining Filters with Comparisons
 * - greater() checks if a value is greater than another
 * - literal() creates a typed value for matching concrete data
 * - Why literal()? Properties are stored as typed RDF literals (xsd:string, xsd:integer)
 * - Plain strings/numbers won't match - need proper type wrapping
 * - Combine property matches with comparison operators
 * - All conditions in and() must be satisfied
 */
async function step7_complexAnd() {
  console.log('\n=== STEP 7: Complex AND Query ===');

  try {
    client.db(DB_NAME);

    // Find people over 30 in San Francisco
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "age", "v:age"),
      WOQL.triple("v:docid", "city", WOQL.literal("San Francisco", "xsd:string")),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.greater("v:age", 30)
    );

    const results = await client.query(query);

    console.log(`✓ Found ${results.bindings.length} people over 30 in San Francisco:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}, age ${binding.age['@value']}`);
    });

  } catch (error) {
    console.error('Error with AND query:', error.message);
    throw error;
  }
}

/**
 * STEP 8: Alternative Queries with or()
 * 
 * CONCEPT: Disjunction (OR logic)
 * - or() means AT LEAST ONE condition must be true
 * - Each alternative branch can succeed independently
 * - Useful for matching multiple possible values
 */
async function step8_orQuery() {
  console.log('\n=== STEP 8: OR Query ===');

  try {
    client.db(DB_NAME);

    // Find people in New York OR San Francisco
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.or(
        WOQL.triple("v:docid", "city", WOQL.literal("New York", "xsd:string")),
        WOQL.triple("v:docid", "city", WOQL.literal("San Francisco", "xsd:string"))
      )
    );

    const results = await client.query(query);

    console.log(`✓ Found ${results.bindings.length} people in NY or SF:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}`);
    });

  } catch (error) {
    console.error('Error with OR query:', error.message);
    throw error;
  }
}

/**
 * STEP 9: Optional Data with opt()
 * 
 * CONCEPT: Optional Patterns
 * - opt() tries to match, but doesn't fail if it can't
 * - Essential for optional properties (like email)
 * - Query succeeds even when optional part fails
 */
async function step9_optionalData() {
  console.log('\n=== STEP 9: Optional Data ===');

  try {
    client.db(DB_NAME);

    // Get all people, with email if available
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.opt(WOQL.triple("v:docid", "email", "v:email"))
    );

    const results = await client.query(query);

    console.log(`✓ Found ${results.bindings.length} people:`);
    results.bindings.forEach(binding => {
      const email = binding.email ? binding.email['@value'] : 'no email';
      console.log(`  - ${binding.name['@value']}: ${email}`);
    });

  } catch (error) {
    console.error('Error with optional query:', error.message);
    throw error;
  }
}

/**
 * STEP 10: Order and Limit Results
 * 
 * CONCEPT: Result Ordering and Limiting
 * - Extract properties as variables for sorting
 * - order_by() sorts results by a variable (ascending/descending)
 * - limit() restricts the number of results returned
 * - These operations compose with triple patterns
 */
async function step10_orderAndLimit() {
  console.log('\n=== STEP 10: Order and Limit ===');

  try {
    client.db(DB_NAME);

    // Get the 3 youngest people
    const query = WOQL.limit(3,
      WOQL.order_by("v:age", "asc",
        WOQL.and(
          WOQL.isa("v:docid", "Person"),
          WOQL.triple("v:docid", "name", "v:name"),
          WOQL.triple("v:docid", "age", "v:age")
        )
      )
    );

    const results = await client.query(query);

    console.log('✓ The 3 youngest people:');
    results.bindings.forEach((binding, index) => {
      console.log(`  ${index + 1}. ${binding.name['@value']}, age ${binding.age['@value']}`);
    });

  } catch (error) {
    console.error('Error with order/limit:', error.message);
    throw error;
  }
}

/**
 * STEP 11: Group and Aggregate
 * 
 * CONCEPT: Aggregation and Grouping in WOQL
 * - group_by(GroupVars, TemplateVars, OutputVar, Query) groups results
 * - GroupVars: variables to group by (e.g., ["v:city"])
 * - TemplateVars: what to collect in each group (e.g., ["v:docid"])
 * - OutputVar: list variable holding all items in each group
 * - Query: the query pattern to execute and group
 * - length() counts elements in the grouped list
 * - member() can iterate over each group member if needed
 * - This is pure WOQL aggregation, no JavaScript needed
 */
async function step11_groupAndAggregate() {
  console.log('\n=== STEP 11: Group and Aggregate ===');
  
  try {
    client.db(DB_NAME);
    
    // Group people by city and count each group
    const query = WOQL.and(
      WOQL.group_by(
        ["city"],           // Group by city
        ["docid"],          // Collect docids in each group
        "v:city_group",       // Output variable for grouped list
        WOQL.and(
          WOQL.isa("v:docid", "Person"),
          WOQL.triple("v:docid", "city", "v:city")
        )
      ),
      WOQL.length("v:city_group", "v:count")
    );
    
    const results = await client.query(query);
    
    console.log('✓ People per city:');
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.city['@value']}: ${binding.count['@value']} people`);
    });
    
  } catch (error) {
    console.error('Error with grouping:', error.message);
    throw error;
  }
}

/**
 * STEP 12: Negation with not() and Document Deletion
 * 
 * CONCEPT: Declarative Deletion
 * - not() operator succeeds when inner pattern fails
 * - Use to find documents MISSING properties
 * - delete_document() as part of WOQL query (not JavaScript loop)
 * - Demonstrates complete CRUD operations declaratively
 */
async function step12_deleteDocuments() {
  console.log('\n=== STEP 12: Negation and Deletion ===');

  try {
    client.db(DB_NAME);

    // Find and delete people WITHOUT an email address using not()
    const deleteQuery = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.not(WOQL.triple("v:docid", "email", "v:email")),  // Negation: no email
      WOQL.delete_document("v:docid")  // Delete matched documents
    );

    const results = await client.query(deleteQuery);

    console.log(`✓ Deleted ${results.bindings.length} people without email`);

  } catch (error) {
    console.error('Error deleting documents:', error.message);
    throw error;
  }
}

// Run the tutorial
main();
```

---

## Running the Tutorial

Save the code above to `woql-tutorial.js` and run:

```bash
node woql-tutorial.js
```

You should see output for each step showing the queries being executed and their results.

---

## Understanding Each Step

Now let's build the tutorial step by step! We'll start with the boilerplate code that sets up your file, then add each step function one at a time.

{% task-heading id="woql-boilerplate" number="Setup" %}
Boilerplate Code
{% /task-heading %}

**Learning Objective**: Set up the foundation file structure that all tutorial steps will build upon.

**What This Does**: This boilerplate code provides the imports, configuration, and main function that orchestrates running your tutorial steps. You'll paste this first, then add each step function below it.

**Start your `woql-tutorial.js` file with this:**

```javascript
const TerminusClient = require('terminusdb');
const { WOQL } = TerminusClient;

// Configuration - adjust these if your setup differs
const SERVER_URL = 'http://localhost:6363';
const ACCOUNT = 'admin';
const DB_NAME = 'woql_tutorial';
const DB_LABEL = 'WOQL Tutorial Database';
const DB_DESCRIPTION = 'A hands-on tutorial database for learning WOQL';

// Initialize client with API key
const client = new TerminusClient.WOQLClient(SERVER_URL, {
  user: ACCOUNT,
  organization: ACCOUNT,
  key: 'root'  // Default password - change if you've set a different password
});

/**
 * Main Function - Orchestrates tutorial steps
 * 
 * Usage examples:
 *   node woql-tutorial.js           # Run all steps
 *   node woql-tutorial.js setup     # Run steps 1-3 only
 *   node woql-tutorial.js step5     # Run specific step
 *   node woql-tutorial.js step5-8   # Run range of steps
 */
async function main() {
  const args = process.argv.slice(2);
  const mode = args[0] || 'all';

  console.log('='.repeat(60));
  console.log('    WOQL Tutorial: Learn by Doing');
  console.log('='.repeat(60));

  // Map of available steps (you'll add these functions below)
  const steps = {
    1: step1_initializeDatabase,
    2: step2_defineSchema,
    3: step3_insertDocuments,
    4: step4_readAllDocuments,
    5: step5_filterByAge,
    6: step6_multipleVariables,
    7: step7_complexAnd,
    8: step8_orQuery,
    9: step9_optionalData,
    10: step10_orderAndLimit,
    11: step11_groupAndAggregate
  };

  try {
    let stepsToRun = [];

    if (mode === 'all') {
      stepsToRun = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    } else if (mode === 'setup') {
      stepsToRun = [1, 2, 3];
    } else if (mode === 'queries') {
      stepsToRun = [4, 5, 6, 7, 8, 9, 10, 11];
    } else if (mode.startsWith('step')) {
      const stepMatch = mode.match(/step(\d+)(?:-(\d+))?/);
      if (stepMatch) {
        const start = parseInt(stepMatch[1]);
        const end = stepMatch[2] ? parseInt(stepMatch[2]) : start;
        for (let i = start; i <= end; i++) {
          if (steps[i]) stepsToRun.push(i);
        }
      }
    }

    console.log(`\nRunning steps: ${stepsToRun.join(', ')}\n`);

    for (const stepNum of stepsToRun) {
      await steps[stepNum]();
    }

    console.log('\n' + '='.repeat(60));
    console.log('    Tutorial Complete! 🎉');
    console.log('='.repeat(60));
    console.log('\nNext: Try modifying queries or add your own!');

  } catch (error) {
    console.error('\n❌ Tutorial failed:', error.message);
    process.exit(1);
  }
}

// Add your step functions below this line
// Each section below will show you what to add

// Run the tutorial
main();
```

**What's in the Boilerplate**:
1. **Imports** - TerminusClient and WOQL from the client library
2. **Configuration** - Server URL, credentials, database name
3. **Client initialization** - Connected and ready to use
4. **Main function** - Handles command-line arguments and runs your steps
5. **Steps map** - References to functions you'll add below

**Next Steps**: After pasting this boilerplate, you'll add each step function below it. Start with Step 1!

---

{% task-heading id="woql-step1" number="1" %}
Initialize Database
{% /task-heading %}

**Learning Objective**: Understand how to create and connect to a TerminusDB database programmatically.

**Concept**: While WOQL can be even be used to reason without a database, most TerminusDB workflows starts with database initialization, or connecting to a database. This step creates a new database (if it doesn't exist) and establishes a connection for subsequent operations.

**Key Operations**:
- Check if database already exists
- Create database with metadata (label, description)
- Connect client to the database

**Code**:
```javascript
async function step1_initializeDatabase() {
  console.log('\n=== STEP 1: Initialize Database ===');
  
  try {
    // Check if database already exists
    const databases = await client.getDatabaseList(ACCOUNT);
    const exists = databases.some(db => db.name === DB_NAME);
    
    if (exists) {
      console.log(`✓ Database '${DB_NAME}' already exists`);
    } else {
      // Create new database
      console.log(`Creating database: ${DB_NAME}`);
      await client.createDatabase(DB_NAME, {
        label: DB_LABEL,
        comment: DB_DESCRIPTION
      });
      console.log('✓ Database created successfully');
    }
    
    // Connect to the database
    client.db(DB_NAME);
    console.log('✓ Connected to database');
    
  } catch (error) {
    console.error('Error initializing database:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `getDatabaseList()` retrieves all databases for your account
2. Check if our tutorial database already exists to avoid errors
3. `createDatabase()` creates a new database with metadata
4. `client.db()` connects the client to work with this specific database

{% task-heading id="woql-step2" number="2" %}
Define Schema
{% /task-heading %}

**Learning Objective**: Learn how to define document classes with typed properties and optional fields.

**Concept**: Most of the time a schema is used to define the structure of your documents. Schemas provide type safety, validation, and enable powerful querying capabilities. TerminusDB can be used schemaless too with pure JSON objects, or in hybrid mode.

**Key Schema Features**:
- `@type: "Class"` - Defines a document class
- `@key` - Specifies how document IDs are generated (Random, Lexical, Hash)
- Typed properties (`xsd:string`, `xsd:integer`)
- Optional fields using `@type: "Optional"`

**Code**:
```javascript
async function step2_defineSchema() {
  console.log('\n=== STEP 2: Define Schema ===');
  
  try {
    client.db(DB_NAME);
    
    // Check if schema already exists
    try {
      const existingSchema = await client.getDocument({ 
        id: 'Person', 
        graph_type: 'schema' 
      });
      if (existingSchema) {
        console.log('✓ Schema already exists');
        return;
      }
    } catch (e) {
      // Schema doesn't exist, continue to create it
    }
    
    const schema = {
      "@type": "Class",
      "@id": "Person",
      "@key": {
        "@type": "Random"
      },
      "name": "xsd:string",
      "age": "xsd:integer",
      "city": {
        "@type": "Optional",
        "@class": "xsd:string"
      },
      "email": {
        "@type": "Optional",
        "@class": "xsd:string"
      }
    };
    
    await client.addDocument(schema, { graph_type: 'schema' });
    console.log('✓ Schema defined successfully');
    console.log('  - Person class with name, age, city, email');
    
  } catch (error) {
    console.error('Error defining schema:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. Check if schema already exists to avoid error from adding it twice
2. Define Person class with required properties (name, age)
3. Add optional properties (city, email) that may be omitted
4. Use `graph_type: 'schema'` to store in schema graph (not instance data)

{% task-heading id="woql-step3" number="3" %}
Insert Sample Documents
{% /task-heading %}

**Learning Objective**: Understand how to insert documents with explicit IDs and handle duplicate prevention.

**Concept**: With your schema defined, you can now insert documents. Documents must conform to the schema. We use explicit `@id` values here for predictable, stable document identifiers.

**Key Operations**:
- Explicit `@id` assignment for stable identifiers
- Document validation against schema
- Duplicate prevention checks
- Batch insertion patterns

**Code**:
```javascript
async function step3_insertDocuments() {
  console.log('\n=== STEP 3: Insert Sample Documents ===');
  
  try {
    client.db(DB_NAME);
    
    // Check if documents already exist
    const existing = await client.getDocument({ id: 'Person/1' });
    if (existing) {
      console.log('✓ Documents already exist (5 found)');
      console.log('  Skipping insertion to avoid duplicates');
      return;
    }
  } catch (e) {
    // Documents don't exist, proceed with insertion
  }
  
  const people = [
    {
      "@id": "Person/1",
      "@type": "Person",
      "name": "Alice Johnson",
      "age": 28,
      "city": "New York",
      "email": "alice@example.com"
    },
    {
      "@id": "Person/2",
      "@type": "Person",
      "name": "Bob Smith",
      "age": 35,
      "city": "San Francisco",
      "email": "bob@example.com"
    },
    {
      "@id": "Person/3",
      "@type": "Person",
      "name": "Carol Williams",
      "age": 28,
      "city": "New York"
    },
    {
      "@id": "Person/4",
      "@type": "Person",
      "name": "David Brown",
      "age": 42,
      "city": "Austin",
      "email": "david@example.com"
    },
    {
      "@id": "Person/5",
      "@type": "Person",
      "name": "Eve Davis",
      "age": 31,
      "city": "San Francisco"
    }
  ];
  
  await client.addDocument(people);
  console.log(`✓ Inserted: ${people}`);
  
  console.log(`\n✓ Total documents inserted: ${people.length}`);
}
```

**What's Happening**:
1. Check if documents exist to prevent duplicates on repeated runs
2. Define array of Person documents with varied data
3. Use explicit IDs (`Person/1`, `Person/2`, etc.) for predictable references
4. Notice Carol and Eve omit `email` field (allowed by Optional schema)
5. Insert all documents at once with `addDocument(people)` - the client accepts arrays for batch insertion

{% task-heading id="woql-step4" number="4" %}
Read All Documents
{% /task-heading %}

**Learning Objective**: Master the fundamental WOQL pattern for reading documents using type checking and document retrieval.

**Concept: Variable Binding and Type Checking**

WOQL is a declarative query language based on datalog. Instead of telling the system *how* to find data, you describe *what* you want:
- Variables start with `v:` prefix (e.g., `v:docid`, `v:doc`)
- `isa(Variable, Type)` checks if a document is of a specific type
- `read_document(IDVar, DocVar)` retrieves the full document
- Variables bind to values that satisfy the query conditions

**Key WOQL Operations**:
- `and()` - Combines multiple conditions (all must be true)
- `isa()` - Type checking predicate
- `read_document()` - Document retrieval predicate

**Code**:
```javascript
async function step4_readAllDocuments() {
  console.log('\n=== STEP 4: Read All Documents ===');
  
  try {
    client.db(DB_NAME);
    
    // Get all Person documents using WOQL
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.read_document("v:docid", "v:doc")
    );
    
    const results = await client.query(query);
    
    console.log(`\n✓ Found ${results.bindings.length} documents:`);
    results.bindings.forEach((binding, index) => {
      const doc = binding.doc;  // Note: binding keys don't include "v:" prefix
      console.log(`  ${index + 1}. ${doc.name} (${doc.age} years old) - ${doc.city || 'no city'}`);
    });
    
  } catch (error) {
    console.error('Error reading documents:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `isa("v:docid", "Person")` - Finds all document IDs where type is a Person, or a subclass of Person
2. `read_document("v:docid", "v:doc")` - For each ID, retrieves the full document
3. `and()` - Both conditions must be satisfied
4. Results in `bindings` array where each binding has `docid` and `doc` properties
5. Access bound variables **without** the `v:` prefix in results

**Declarative Thinking**: We didn't write a loop or filtering logic. We declared "find documents of type Person and read them" - the query engine figures out how.

{% task-heading id="woql-step5" number="5" %}
Filter Documents with triple()
{% /task-heading %}

**Learning Objective**: Learn to filter documents by property values using triple patterns and typed literals.

**Concept: Triple Patterns and Typed Literals**

Under the hood, TerminusDB stores all data as RDF triples: `(subject, predicate, object)`. The `triple()` predicate lets you match specific property patterns:
- `triple(Subject, Property, Value)` - Matches subject-property-value patterns
- Properties are stored as **typed RDF literals** (e.g., `28^^xsd:integer`)
- Use `literal(value, type)` to specify typed values for matching
- Combine with `isa()` for type-safe filtering

**Key Operations**:
- `triple()` - Property pattern matching
- `literal()` - Specifies typed literal values
- Combining type checking (`isa`) with property filters (`triple`)

**Code**:
```javascript
async function step5_filterByAge() {
  console.log('\n=== STEP 5: Filter by Age ===');
  
  try {
    client.db(DB_NAME);
    
    // Find all people aged 28
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "age", WOQL.literal("28", "xsd:integer")),
      WOQL.read_document("v:docid", "v:doc")
    );
    
    const results = await client.query(query);
    
    console.log(`✓ Found ${results.bindings.length} people aged 28:`);
    results.bindings.forEach(binding => {
      const doc = binding.doc;
      console.log(`  - ${doc.name} from ${doc.city}`);
    });
    
  } catch (error) {
    console.error('Error filtering documents:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `isa("v:docid", "Person")` - Start with all Person documents
2. `triple("v:docid", "age", literal(28, "xsd:integer"))` - Filter to only those with age=28
3. `literal(28, "xsd:integer")` - Creates a typed literal that matches stored data
4. `read_document()` - Retrieves full document for matched IDs
5. **Only** documents satisfying ALL three conditions are returned

{% callout type="note" %}
**Why `literal()`?**

Plain `28` would be interpreted as a URI. Properties are stored as typed literals (`28^^xsd:integer`), so we need `literal()` to specify matching typed values. 

When high precision is needed, submitting a string is preferred over submitting a number due to many clients having narrow precision limits in computing numbers (floats and doubles, vs use of decimals and rationals in TerminusDB).
{% /callout %}

{% task-heading id="woql-step6" number="6" %}
Work with Multiple Variables
{% /task-heading %}

**Learning Objective**: Extract multiple properties from documents and understand variable unification in WOQL.

**Concept: Multiple Property Extraction and Unification**

When you use multiple `triple()` patterns with the same subject variable, you're extracting multiple properties from each document. This demonstrates a key datalog principle: **unification**.
- Same variable appearing multiple times must bind to the same value
- `v:docid` unifies across all triple patterns - same document for all properties
- Only documents with **all** specified properties will match
- Each property binds to a separate variable

**Key Pattern**:
```woql
triple("v:docid", "property1", "v:var1"),
triple("v:docid", "property2", "v:var2"),
triple("v:docid", "property3", "v:var3")
```

**Code**:
```javascript
async function step6_multipleVariables() {
  console.log('\n=== STEP 6: Extract Specific Fields ===');
  
  try {
    client.db(DB_NAME);
    
    // Get name, age, and city for all people who have a city
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.triple("v:docid", "age", "v:age"),
      WOQL.triple("v:docid", "city", "v:city")
    );
    
    const results = await client.query(query);
    
    console.log(`✓ Found ${results.bindings.length} people with cities:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}, age ${binding.age['@value']}, lives in ${binding.city['@value']}`);
    });
    
  } catch (error) {
    console.error('Error with multiple variables:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. Four patterns all use `v:docid` - must be the same document for all
2. Extract three separate properties into three variables
3. `triple("v:docid", "city", "v:city")` - Only matches documents that **have** a city property
4. Result bindings contain `name`, `age`, and `city` as separate values
5. Access typed literals with `['@value']` to get the actual value

**Unification in Action**: Since `v:docid` appears in all patterns, WOQL ensures all properties come from the **same** document, for each document where all constraints are fulfilled. This is automatic constraint enforcement! Think of each possible solution (each docid) as it's own "world", where variables get bound to possible value(s).

{% task-heading id="woql-step7" number="7" %}
Complex Queries with and()
{% /task-heading %}

**Learning Objective**: Combine multiple filter conditions using comparisons and typed literals.

**Concept: Complex Filtering with Comparisons**

Building on what we've learned, we can combine property matching with comparison operators for powerful filtering:
- `greater(Var, Value)` - Checks if variable value is greater than another
- `less(Var, Value)` - Checks if variable value is less than another  
- Mix property filters with comparisons in `and()` blocks
- All conditions must be satisfied simultaneously

**Key Operations**:
- Comparison operators (`greater`, `less`, `equals`)
- Combining `literal()` matching with comparisons
- Multiple property constraints

**Code**:
```javascript
async function step7_complexAnd() {
  console.log('\n=== STEP 7: Complex AND Query ===');
  
  try {
    client.db(DB_NAME);
    
    // Find people over 30 in San Francisco
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "age", "v:age"),
      WOQL.triple("v:docid", "city", WOQL.literal("San Francisco", "xsd:string")),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.greater("v:age", 30)
    );
    
    const results = await client.query(query);
    
    console.log(`✓ Found ${results.bindings.length} people over 30 in San Francisco:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}, age ${binding.age['@value']}`);
    });
    
  } catch (error) {
    console.error('Error with AND query:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `isa("v:docid", "Person")` - Type constraint
2. `triple("v:docid", "age", "v:age")` - Extract age for comparison
3. `triple("v:docid", "city", literal("San Francisco", "xsd:string"))` - City filter using typed literal
4. `greater("v:age", 30)` - Age comparison constraint
5. All five conditions must be true for a match

**Why This Pattern**: Extract value to variable (`v:age`), then use comparison operator on that variable. This is more flexible than inline comparisons.

{% callout type="note" %}
**Why use `literal()`?**

When you write a plain string in a WOQL triple like `"San Francisco"`, the system interprets it as a URI, prefixed by the base IRI configured in your graph (e.g., `http://mybase.com/San Francisco`). 

Properties in TerminusDB are stored as typed RDF literals with explicit datatypes:
- Strings: `"San Francisco"^^xsd:string`
- Integers: `28^^xsd:integer`
- Decimals: `3.14^^xsd:decimal`

Use `WOQL.literal(value, type)` to create properly typed literals that match your stored data. Without it, plain values won't match because they're interpreted as URIs instead of typed literals.
{% /callout %}

{% task-heading id="woql-step8" number="8" %}
Alternative Queries with or()
{% /task-heading %}

**Learning Objective**: Use disjunction (OR logic) to match documents satisfying any of multiple conditions.

**Concept: Disjunction in WOQL**

While `and()` requires **all** conditions to be true, `or()` succeeds if **at least one** condition is true:
- Each branch of `or()` is evaluated independently (own "world")
- A document matches if it satisfies any branch
- Useful for matching multiple possible values
- Can combine with `and()` for complex logic

**Code**:
```javascript
async function step8_orQuery() {
  console.log('\n=== STEP 8: OR Query ===');
  
  try {
    client.db(DB_NAME);
    
    // Find people in New York OR San Francisco
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.or(
        WOQL.triple("v:docid", "city", WOQL.literal("New York", "xsd:string")),
        WOQL.triple("v:docid", "city", WOQL.literal("San Francisco", "xsd:string"))
      )
    );
    
    const results = await client.query(query);
    
    console.log(`✓ Found ${results.bindings.length} people in NY or SF:`);
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.name['@value']}`);
    });
    
  } catch (error) {
    console.error('Error with OR query:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `and()` wrapper ensures ALL outer conditions are met
2. `or()` nested inside - city must be "New York" **OR** "San Francisco"
3. Document matches if city equals either value
4. Both branches use `literal()` for proper type matching
5. Results include people from both cities

**Logical Structure**: `and(type=Person, has_name, (city=NY or city=SF))` - combination of conjunction (and) and disjunction (or).

{% task-heading id="woql-step9" number="9" %}
Optional Data with opt()
{% /task-heading %}

**Learning Objective**: Handle optional fields gracefully using WOQL's `opt()` operator.

**Concept: Optional Patterns**

Not all documents have all properties (e.g., some people don't have email addresses). The `opt()` operator handles this:
- `opt(Pattern)` - Tries to match the pattern
- If it succeeds: variable is bound to the value
- If it fails: query continues (doesn't fail), variable remains unbound
- Essential for queries that should succeed regardless of optional field presence

**Without `opt()`**: Query fails if any document lacks the property
**With `opt()`**: Query succeeds, variable is `undefined` if property is missing

**Code**:
```javascript
async function step9_optionalData() {
  console.log('\n=== STEP 9: Optional Data ===');
  
  try {
    client.db(DB_NAME);
    
    // Get all people, with email if it exists
    const query = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.triple("v:docid", "name", "v:name"),
      WOQL.opt(
        WOQL.triple("v:docid", "email", "v:email")
      )
    );
    
    const results = await client.query(query);
    
    console.log(`✓ Found ${results.bindings.length} people:`);
    results.bindings.forEach(binding => {
      const email = binding.email ? binding.email['@value'] : 'no email';
      console.log(`  - ${binding.name['@value']}: ${email}`);
    });
    
  } catch (error) {
    console.error('Error with optional data:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `isa()` and first `triple()` are required - must match
2. `opt(triple("v:docid", "email", "v:email"))` - Email triple is optional
3. If document has email: `binding.email` is bound to the email value
4. If document lacks email: `binding.email` is `undefined`, query still succeeds
5. Check `binding.email` existence before accessing to avoid errors

**Use Case**: Query all people, showing email when available, without filtering out people without emails.

{% task-heading id="woql-step10" number="10" %}
Order and Limit Results
{% /task-heading %}

**Learning Objective**: Control result ordering and quantity using WOQL's ordering and limiting operators.

**Concept: Result Ordering and Limiting**

Once you've matched documents, you often want to control **how many** results and in what **order**:
- `order_by(Variable, Direction, Query)` - Sorts results by a variable
  - Direction: `"asc"` (ascending) or `"desc"` (descending)
- `limit(Count, Query)` - Returns only the first N results
- These compose: `limit(3, order_by("v:age", "asc", ...))` = "3 youngest"

**Pattern**: Extract the property to sort by, then order and limit:
```
limit(N,
  order_by("v:property", "asc",
    and(
      isa(...),
      triple("v:docid", "property", "v:property"),
      ...
    )
  )
)
```

**Code**:
```javascript
async function step10_orderAndLimit() {
  console.log('\n=== STEP 10: Order and Limit ===');
  
  try {
    client.db(DB_NAME);
    
    // Get the 3 youngest people
    const query = WOQL.limit(3,
      WOQL.order_by("v:age", "asc",
        WOQL.and(
          WOQL.isa("v:docid", "Person"),
          WOQL.triple("v:docid", "name", "v:name"),
          WOQL.triple("v:docid", "age", "v:age")
        )
      )
    );
    
    const results = await client.query(query);
    
    console.log('✓ The 3 youngest people:');
    results.bindings.forEach((binding, index) => {
      console.log(`  ${index + 1}. ${binding.name['@value']}, age ${binding.age['@value']}`);
    });
    
  } catch (error) {
    console.error('Error with order/limit:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. Inner `and()` - Match all Persons, extract name and age
2. `order_by("v:age", "asc", ...)` - Sort results by age in ascending order (youngest first)
3. `limit(3, ...)` - Take only the first 3 results after sorting
4. Result: 3 youngest people in order

**Composition**: WOQL operators wrap each other like functions. Read inside-out: match → sort → limit.

{% task-heading id="woql-step11" number="11" %}
Group and Aggregate
{% /task-heading %}

**Learning Objective**: Perform aggregations by grouping documents and counting group members using pure WOQL.

**Concept: Grouping and Aggregation**

WOQL provides powerful aggregation through `group_by()` - think SQL's GROUP BY:
- `group_by(GroupVars, TemplateVars, OutputVar, Query)` groups results
  - **GroupVars**: Array of variables to group by (e.g., `["city"]`)
  - **TemplateVars**: Array of variables to collect in each group (e.g., `["docid"]`)
  - **OutputVar**: Variable holding the list of collected items per group
  - **Query**: The pattern to execute and group
- `length(ListVar, CountVar)` - Counts elements in a list
- Combine them for "count by category" queries

**Pattern**: Group → Count
```
and(
  group_by(["v:category"], ["v:item"], "v:group",
    query_pattern
  ),
  length("v:group", "v:count")
)
```

**Code**:
```javascript
async function step11_groupAndAggregate() {
  console.log('\n=== STEP 11: Group and Aggregate ===');
  
  try {
    client.db(DB_NAME);
    
    // Group people by city and count each group
    const query = WOQL.and(
      WOQL.group_by(
        ["city"],           // Group by city
        ["docid"],          // Collect docids in each group
        "v:city_group",       // Output variable for grouped list
        WOQL.and(
          WOQL.isa("v:docid", "Person"),
          WOQL.triple("v:docid", "city", "v:city")
        )
      ),
      WOQL.length("v:city_group", "v:count")
    );
    
    const results = await client.query(query);
    
    console.log('✓ People per city:');
    results.bindings.forEach(binding => {
      console.log(`  - ${binding.city['@value']}: ${binding.count['@value']} people`);
    });
    
  } catch (error) {
    console.error('Error with grouping:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. Inner query finds all Persons with cities
2. `group_by(["city"], ["docid"], "v:city_group", ...)` groups results:
   - For each unique `v:city` value (specifies the variable name)
   - Collect all `docid` values into a list (variable name)
   - Store list in `v:city_group` variable
3. `length("v:city_group", "v:count")` - Count items in each group's list
4. Results: One binding per city with count

**Pure WOQL Aggregation**: No JavaScript loops or counting needed. The query engine handles all aggregation. This is the declarative power of datalog!

**Concept: Aggregation**
- `group_by()` groups results by a variable
- `count()` gives us the number in each group

{% task-heading id="woql-step12" number="12" %}
Negation with not() and Document Deletion
{% /task-heading %}

**Learning Objective**: Use negation to find documents that DON'T match a pattern, then delete them based on that criteria.

**Concept: Negation in WOQL**

WOQL supports logical negation with `not()`, which succeeds when the inner pattern **fails to match**:
- `not(Pattern)` - True when Pattern is false
- Use to find documents **missing** properties
- Combine with deletion to remove incomplete records
- Demonstrates complete CRUD operations (Create, Read, Update, Delete)

**Key Operations**:
- `not()` - Logical negation operator
- `triple()` inside `not()` - Tests for absence of a property
- Query to find, then delete by ID
- Clean ID extraction (remove prefix)

**Code**:
```javascript
async function step12_deleteDocuments() {
  console.log('\n=== STEP 12: Negation and Deletion ===');

  try {
    client.db(DB_NAME);

    // Find and delete people WITHOUT an email address using not()
    const deleteQuery = WOQL.and(
      WOQL.isa("v:docid", "Person"),
      WOQL.not(WOQL.triple("v:docid", "email", "v:email")),  // Negation: no email
      WOQL.delete_document("v:docid")  // Delete matched documents
    );

    const results = await client.query(deleteQuery);

    console.log(`✓ Deleted ${results.bindings.length} people without email`);

  } catch (error) {
    console.error('Error deleting documents:', error.message);
    throw error;
  }
}
```

**What's Happening**:
1. `isa("v:docid", "Person")` - Find all Person documents
2. `not(triple("v:docid", "email", "v:email"))` - **Key**: Matches documents that DON'T have an email property
3. `delete_document("v:docid")` - Deletes all matched documents in one declarative operation
4. No JavaScript loops needed - WOQL handles the iteration

**Negation Logic**: 
- `triple("v:docid", "email", "v:email")` succeeds if email exists
- `not(triple(...))` inverts: succeeds if email does NOT exist
- `delete_document()` operates on all bindings that satisfy the negation

**Declarative Deletion**: Unlike imperative loops, `delete_document()` is part of the WOQL query itself. The query finds and deletes in a single operation - this is the power of declarative data modification!

**Why This Pattern**: Negation with `delete_document()` is powerful for data cleanup - find and remove incomplete records, test for missing relationships, or implement "must not have" constraints, all in pure WOQL.

---

## Key WOQL Concepts Explained

### 1. Variables and Binding

Variables in WOQL use the `v:` prefix. When a variable appears in multiple predicates, it must unify to the same value:

```javascript
WOQL.and(
  WOQL.triple("v:person", "@schema:age", "v:age"),  // Binds v:age
  WOQL.greater("v:age", 30)                          // Uses bound v:age
)
```

### 2. Triple Patterns

TerminusDB stores data as triples (subject, predicate, object). You query by matching patterns:

```javascript
// Match: subject=v:person, predicate=@schema:name, object=v:name
WOQL.triple("v:person", "@schema:name", "v:name")
```

### 3. Declarative vs Imperative

**Imperative** (JavaScript): "Do these steps in order"
```javascript
const people = await fetchPeople();
const filtered = people.filter(p => p.age > 30);
const names = filtered.map(p => p.name);
```

**Declarative** (WOQL): "Describe the logic of what you want"
```javascript
WOQL.and(
  WOQL.triple("v:person", "@schema:age", "v:age"),
  WOQL.greater("v:age", 30),
  WOQL.triple("v:person", "@schema:name", "v:name")
)
```

### 4. Unification

When the same variable appears multiple times, WOQL ensures it has the same value everywhere:

```javascript
// Find two people with the same age
WOQL.and(
  WOQL.triple("v:person1", "@schema:age", "v:age"),
  WOQL.triple("v:person2", "@schema:age", "v:age"),  // Same v:age!
  WOQL.not().eq("v:person1", "v:person2")             // But different people
)
```

---

## Experiment and Extend

Now that you've completed the tutorial, try these challenges:

1. **Add more fields**: Extend the Person schema with `occupation` and `country`
2. **Complex filters**: Find people aged 25-35 in specific cities
3. **Update documents**: Create a function that updates a person's age
4. **Relationships**: Add a `Friend` class connecting people together
5. **Path queries**: Find friends of friends using WOQL's path capabilities

---

## Advanced WOQL Guides

Ready to go deeper? Explore these specialized guides:

### Core Operations
* [WOQL Basics](/docs/woql-basics/) - Comprehensive language reference
* [WOQL Add Docs](/docs/add-documents-with-woql/) - Document creation patterns
* [WOQL Edit Docs](/docs/edit-documents-with-woql/) - Update strategies
* [WOQL Delete Docs](/docs/delete-documents-with-woql/) - Safe deletion patterns
* [WOQL Read Docs](/docs/read-documents-with-woql/) - Advanced reading techniques

### Advanced Queries
* [WOQL Filter](/docs/filter-with-woql/) - Complex filtering patterns
* [WOQL Order By](/docs/order-by-with-woql/) - Sorting and pagination
* [WOQL Query Arrays](/docs/query-arrays-and-sets-in-woql/) - Working with collections
* [WOQL Group Results](/docs/group-query-results/) - Aggregation and analytics
* [WOQL Path Queries](/docs/path-queries-in-woql/) - Graph traversal
* [WOQL Math Queries](/docs/maths-based-queries-in-woql/) - Calculations and expressions
* [WOQL Schema Queries](/docs/schema-queries-with-woql/) - Querying your schema

---

## Troubleshooting

### Connection Errors

If you can't connect to TerminusDB:

```javascript
// Check your server URL
const SERVER_URL = 'http://localhost:6363';

// Verify authentication
client.setApiKey('root');  // Default password
```

### Schema Errors

If schema creation fails, the fastest way is to fix it might be to recreate the database:

```javascript
await client.deleteDatabase(DB_NAME);
await client.createDatabase(DB_NAME, { label: DB_LABEL });
```

### Empty Results

If queries return no results:

1. Check that you've run steps 1-3 first
2. Verify documents exist: `await step4_readAllDocuments()`
3. Check variable names match exactly (case-sensitive)

---

## Understanding the Output

When you run a WOQL query, you get back bindings - solutions that satisfy your query:

```javascript
{
  bindings: [
    { "v:name": "Alice Johnson", "v:age": 28 },
    { "v:name": "Carol Williams", "v:age": 28 }
  ]
}
```

Each binding is one complete solution. Multiple bindings mean multiple solutions were found.

---

## Cleanup

After completing the tutorial, you may want to clean up the test database and data.

{% callout type="warning" %}
**Be Careful with Deletion**

Deleting a database or documents is permanent and cannot be undone. Make sure you're targeting the correct database before running cleanup commands.
{% /callout %}

### Remove Tutorial Documents

To remove the specific documents created in this tutorial:

```bash
# Delete all Person documents created in the tutorial
curl -X DELETE "http://localhost:6363/api/document/admin/woql_tutorial?author=admin&message=Cleanup%20tutorial%20documents" \
  -u "admin:root" \
  -H "Content-Type: application/json" \
  -d '["Person/1", "Person/2", "Person/3", "Person/4", "Person/5"]'
```

Or delete documents individually:

```bash
# Delete one document at a time
curl -X DELETE "http://localhost:6363/api/document/admin/woql_tutorial?id=Person/1&author=admin&message=Delete%20Person1" \
  -u "admin:root"
  
curl -X DELETE "http://localhost:6363/api/document/admin/woql_tutorial?id=Person/2&author=admin&message=Delete%20Person2" \
  -u "admin:root"
  
# ... and so on for Person/3, Person/4, Person/5
```

### Delete the Tutorial Database

To completely remove the tutorial database:

```javascript
const TerminusClient = require('terminusdb');

const client = new TerminusClient.WOQLClient('http://localhost:6363', {
  user: 'admin',
  organization: 'admin',
  key: 'root'
});

// Delete the tutorial database
await client.deleteDatabase('woql_tutorial');
console.log('✓ Tutorial database deleted');
```

Or run from command line:

```bash
curl -X DELETE "http://localhost:6363/api/db/admin/woql_tutorial" \
  -u "admin:root"
```

---

## Next Steps

1. ✅ Complete this hands-on tutorial
2. 📖 Read [WOQL Explanation](/docs/woql-explanation/) for deeper concepts
3. 🔧 Explore the [JavaScript Client API](/docs/javascript/)
4. 🐍 Try the [Python Client](/docs/python/) for Python developers
5. 🎯 Build your first real application with TerminusDB

---

**Congratulations!** You've learned the fundamentals of WOQL through hands-on practice. You now understand variable binding, unification, triple patterns, and how to build complex declarative queries. Keep experimenting and building!