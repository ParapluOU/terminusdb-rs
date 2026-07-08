---
title: EditDocumentComponent
nextjs:
  metadata:
    title: EditDocumentComponent
    description: The EditDocumentComponent allows you to edit an existing document using the FrameViewer component
    keywords: terminusdb, document, document database, documents, edit, editdocumentcomponent, javascript, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/edit-document-component/
media: []
tags:
  - typescript
  - documents
  - reference
  - dashboard
---

{% callout type="warning" title="Unmaintained package" %}
These packages (`@terminusdb/terminusdb-documents-ui` etc.) were last published December 2023 and are no longer actively maintained. The documentation is preserved for reference. Community contributions and pull requests are welcome.
{% /callout %}


The `EditDocumentComponent` allows you to edit an existing document using the [FrameViewer](/docs/document-ui-sdk/) component.

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

- `type`
- The document type

---

- `documentJson`
- The document object

---

- `documentID`
- The document ID

---

- `frames`
- The database Class Frame, or object of all class frames

---

- `closeButtonClick`
- A function that acts as a callback when the panel exit `x` button is clicked

---

- `updateDocument`
- A function that acts as a callback when the `submit` button is clicked

---

- `SearchComponent`
- A react component used as search component

{% /table %}

## Example

```python
import React, {useEffect}  from "react";
import {EditDocumentComponent,useTDBDocuments} from "@terminusdb/terminusdb-documents-ui-template"

export const DocumentEdit = ({type, documentID, tdbClient}) => { 
    const {
        updateDocument,
        getDocument,
        selectedDocument,
        getDocumentFrames,
        frames,
        error,
        setError
    } = useTDBDocuments(tdbClient)

     const  updateDocumentHandler = async (jsonDoc) =>{
        const docUp = await updateDocument(jsonDoc)
        if(docUp){
            getDocument(documentID)
            // do somethig after update document
        }
   }
    // implement the chage method
    useEffect(() => {
        getDocumentFrames()
        getDocument(documentID)
    },[])

    const closeButtonClick = () =>{
       // do something after click the close panel button the interface
    }

    const DocumentSearchComponent = () =>{
        //make you document search component
        return </div>
    }

    if(!frames) return  <div>{`Fetching frames for document type ${type} ...`}</div>
    const errorMessage = typeof error === "object" ? JSON.stringify(error,null,4) : error

    return <React.Fragment>
            {error && <div>Server Error: {errorMessage} </div>}
            <EditDocumentComponent
                SearchComponent={DocumentSearchComponent}
                documentID={documentID} 
                updateDocument={updateDocumentHandler}
                type={type}
                frames={frames}
                closeButtonClick={closeButtonClick}
                documentJson={selectedDocument}
            />
        </React.Fragment>
}
```

View the EditDocumentComponent component integrated inside a dashboard here

[EditDocumentComponent full example JS code](https://github.com/terminusdb/dashboard-examples-sandbox/blob/main/terminusdb-documents-ui-template-example/dashboard-demo/src/pages/DocumentEdit.js)

[Code Sandbox](https://codesandbox.io/s/github/terminusdb/dashboard-examples-sandbox/tree/main/terminusdb-documents-ui-template-example/dashboard-demo)