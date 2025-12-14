# Structured Output Demo for TerminusDB MCP Server

The TerminusDB MCP Server now supports the 2025-06-18 protocol version, which includes structured output support. This means that WOQL query results are returned both as:

1. **Text content** - Pretty-printed JSON for human readability
2. **Structured content** - The actual JSON object for programmatic processing

## Example Tool Response

When executing a WOQL query, the response now includes both formats:

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\n  \"bindings\": [\n    {\"x\": \"doc:person1\", \"y\": \"Person\"},\n    {\"x\": \"doc:person2\", \"y\": \"Person\"}\n  ]\n}"
    }
  ],
  "structuredContent": {
    "bindings": [
      {"x": "doc:person1", "y": "Person"},
      {"x": "doc:person2", "y": "Person"}
    ]
  }
}
```

## Benefits

1. **Better LLM Integration** - LLMs can directly process the structured JSON without re-parsing
2. **Token Efficiency** - Avoids string serialization/deserialization overhead
3. **Rich Client Display** - MCP clients can render results in tables, trees, or other formats
4. **Type Safety** - Structured data maintains its original types (numbers, booleans, etc.)

## Protocol Version Changes

- **Old**: 2024-11-05 (text-only responses)
- **New**: 2025-06-18 (structured content support)

The upgrade maintains backward compatibility - the text content is still provided for clients that don't support structured output.