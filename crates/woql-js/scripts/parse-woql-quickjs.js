/**
 * WOQL JS-syntax to JSON-LD parser for QuickJS
 *
 * This is a QuickJS-compatible entry point that doesn't use Node.js APIs.
 * It exports a global parseWoql function that can be called from Rust.
 *
 * We import our custom WOQL-only module which avoids the HTTP client dependencies.
 */

// Import our WOQL-only module that doesn't have Node.js dependencies
const WOQL = require('./woql-only');

/**
 * Parse a JavaScript-syntax WOQL query string into JSON-LD format.
 * @param {string} queryString - The JS-syntax WOQL query
 * @returns {string} JSON string of the JSON-LD representation
 * @throws {Error} If the query cannot be parsed
 */
globalThis.parseWoql = function(queryString) {
  if (!queryString || typeof queryString !== 'string') {
    throw new Error('Query must be a non-empty string');
  }

  const trimmed = queryString.trim();
  if (!trimmed) {
    throw new Error('Query must be a non-empty string');
  }

  // Generate the prelude that defines all WOQL functions in the eval scope
  const prelude = WOQL.emerge();

  // Evaluate the query string with the prelude
  // This is the same approach used in the dashboard
  const woqlQuery = eval(prelude + "\n" + trimmed);

  if (!woqlQuery) {
    throw new Error('Query evaluation returned null/undefined');
  }

  // Convert to JSON-LD
  const jsonLD = woqlQuery.json();

  // Return as JSON string
  return JSON.stringify(jsonLD);
};
