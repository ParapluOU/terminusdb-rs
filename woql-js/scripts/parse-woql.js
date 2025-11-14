#!/usr/bin/env node

/**
 * WOQL JS-syntax to JSON-LD parser
 *
 * Reads JS-syntax WOQL from stdin and outputs JSON-LD to stdout.
 * Uses the official terminusdb-client-js library for parsing.
 *
 * Exit codes:
 * 0 - Success
 * 1 - Parse error or evaluation error
 * 2 - Missing input
 */

const TerminusClient = require('@terminusdb/terminusdb-client');

// Read from stdin
let inputData = '';

process.stdin.setEncoding('utf8');

process.stdin.on('data', (chunk) => {
  inputData += chunk;
});

process.stdin.on('end', () => {
  try {
    const queryString = inputData.trim();

    if (!queryString) {
      console.error('Error: No input provided');
      process.exit(2);
    }

    // Get the WOQL object
    const WOQL = TerminusClient.WOQL;

    // Generate the prelude that defines all WOQL functions in the eval scope
    const prelude = WOQL.emerge();

    // Evaluate the query string with the prelude
    // This is the same approach used in the dashboard
    const woqlQuery = eval(prelude + "\n" + queryString);

    if (!woqlQuery) {
      console.error('Error: Query evaluation returned null/undefined');
      process.exit(1);
    }

    // Convert to JSON-LD
    const jsonLD = woqlQuery.json();

    // Output as single-line JSON to stdout
    console.log(JSON.stringify(jsonLD));

    process.exit(0);
  } catch (error) {
    console.error('Parse error:', error.message);
    if (error.stack) {
      console.error(error.stack);
    }
    process.exit(1);
  }
});

process.stdin.on('error', (error) => {
  console.error('Input error:', error.message);
  process.exit(1);
});
