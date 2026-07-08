---
title: DocumentClassesSummary Component
nextjs:
  metadata:
    title: DocumentClassesSummary Component
    description: The DocumentClassesSummary component allows you to visualize document classes using interactive cards.
    keywords: terminusdb, document, document database, documentclassessummary component, documents, javascript, json-ld, terminusdb javascript client
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/documentclassessummary/
media: []
tags:
  - typescript
  - documents
  - reference
  - dashboard
---

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


{% callout type="warning" title="Unmaintained package" %}
These packages (`@terminusdb/terminusdb-documents-ui` etc.) were last published December 2023 and are no longer actively maintained. The documentation is preserved for reference. Community contributions and pull requests are welcome.
{% /callout %}


The `DocumentClassesSummary` component allows you to visualize document classes using interactive cards.

## Installation

Install the dependencies from npm

```bash
npm install @terminusdb/terminusdb-documents-ui
npm install @terminusdb/terminusdb-react-table
npm install @terminusdb/terminusdb-documents-ui-templates
```

## Properties

{% table %}

- Properties
- Description

---

- `totalDocumentCount`
- The total number of documents

---

- `perDocumentCount`
- The number of documents for a type

---

- `onDocumentClick`
- A function that acts as a callback when the document class card is clicked

{% /table %}

## Example

```javascript
import React, {useEffect} from "react"
import {DocumentClassesSummary,useTDBDocuments} from "@terminusdb/terminusdb-documents-ui-template"

export const Documents = ({tdbClient}) => {   
    const {perDocumentCount,
        totalDocumentCount, 
        getDocumentNumbers,
        setError,
        loading,
        error}=useTDBDocuments(tdbClient)


    useEffect(() => {
       if(tdbClient)getDocumentNumbers()
    }, [tdbClient])

    function handleCardClick (doc) {
        // do something after click the card, 
        // maybe navigate in the document list page
    }

    if(!frames) return  <div>{`Fetching frames for document type ${type} ...`}</div>
    const errorMessage = typeof error === "object" ? JSON.stringify(error,null,4) : error

    return <div className="w-100">
            {error && <div>Server Error: {errorMessage} </div>}
            <DocumentClassesSummary 
                    totalDocumentCount={totalDocumentCount}
                    perDocumentCount={perDocumentCount} 
                    onDocumentClick={handleCardClick}/>
        </div>
}
```

View the DocumentClassesSummary integrated inside a dashboard here

[DocumentClassesSummary full example JS code](https://github.com/terminusdb/dashboard-examples-sandbox/blob/main/terminusdb-documents-ui-template-example/dashboard-demo/src/pages/Documents.js)

[Code Sandbox](https://codesandbox.io/s/github/terminusdb/dashboard-examples-sandbox/tree/main/terminusdb-documents-ui-template-example/dashboard-demo)
