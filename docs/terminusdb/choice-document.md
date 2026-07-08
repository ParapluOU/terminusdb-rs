---
title: Choice Document in the Document UI
nextjs:
  metadata:
    title: Choice Document in the Document UI
    description: How document choice works in the document UI
    keywords: terminusdb, choice document in the document ui, data model, document, document type, javascript, schema, terminusdb javascript client
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/choice-document/
media: []
tags:
  - typescript
  - schema
  - reference
  - dashboard
---

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


This example shows how `<FrameViewer/>` appears for a document `Guy` with choice document fields in Create/ Edit or View mode.

In this example `Guy` has a mandatory property called `favorite_group` with the choice of documents called Art, Dance or Music, an optional `"second_favorite_group"` property also with the choice Art, Dance or Music, a set `member_of` property with the same choices, and a list `attends_group_in_order` property with the above same choices, in an ordered fashion.

## Demo

Take a look at the **[](https://github.com/terminusdb/dashboard-examples-sandbox)**[Demo Playground](https://github.com/terminusdb/dashboard-examples-sandbox) to view `<FrameViewer/>` with Choice properties in Create, edit or view mode.

The below Frames show the definition of Art, Dance or Music which are other document classes.

## Frame

The below Frame consists of the `Guy` document

```javascript
  let frame = {
    "@context": {
      "@base": "terminusdb:///data/",
      "@schema": "terminusdb:///schema#",
      "@type": "Context",
      "xsd": "http://www.w3.org/2001/XMLSchema#"
    },
    "Guy": {
      "@key": {
        "@type": "Random"
      },
      "@type": "Class", 
      "attends_group_in_order": {
        "@class": "Group",
        "@type": "List"
      },
      "favorite_group": "Group",
      "member_of": {
          "@class": "Group",
          "@type": "Set"
      },
      "second_favorite_group": {
          "@class": "Group",
          "@type": "Optional"
      }
    },
    "Dance": {
      "@documentation": [
        {
          "@language": "en",
          "@properties": {
            "capacity": {
              "@comment": "Max number of people in group",
              "@label": "Capacity"
            },
            "name": {
              "@comment": "Title of the group",
              "@label": "Name"
            }
          }
        },
        {
          "@language": "ka",
          "@properties": {
            "capacity": {
              "@comment": "ადამიანების მაქსიმალური რაოდენობა ჯგუფში",
              "@label": "ტევადობა"
            },
            "name": {
              "@comment": "ჯგუფის სათაური",
              "@label": "სახელი"
            }
          }
        }
      ],
      "@key": {
        "@fields": [
          "name"
        ],
        "@type": "Lexical"
      },
      "@type": "Class",
      "capacity": "xsd:decimal",
      "name": "xsd:string"
    },
    "Art": {
      "@documentation": [
        {
          "@language": "en",
          "@properties": {
            "capacity": {
              "@comment": "Max number of people in group",
              "@label": "Capacity"
            },
            "name": {
              "@comment": "Title of the group",
              "@label": "Name"
            }
          }
        },
        {
          "@language": "ka",
          "@properties": {
            "capacity": {
              "@comment": "ადამიანების მაქსიმალური რაოდენობა ჯგუფში",
              "@label": "ტევადობა"
            },
            "name": {
              "@comment": "ჯგუფის სათაური",
              "@label": "სახელი"
            }
          }
        }
      ],
      "@key": {
        "@fields": [
          "name"
        ],
        "@type": "Lexical"
      },
      "@type": "Class",
      "capacity": "xsd:decimal",
      "name": "xsd:string"
    },
    "Music": {
      "@documentation": [
        {
          "@language": "en",
          "@properties": {
            "capacity": {
              "@comment": "Max number of people in group",
              "@label": "Capacity"
            },
            "name": {
              "@comment": "Title of the group",
              "@label": "Name"
            }
          }
        },
        {
          "@language": "ka",
          "@properties": {
            "capacity": {
              "@comment": "ადამიანების მაქსიმალური რაოდენობა ჯგუფში",
              "@label": "ტევადობა"
            },
            "name": {
              "@comment": "ჯგუფის სათაური",
              "@label": "სახელი"
            }
          }
        }
      ],
      "@key": {
        "@fields": [
          "name"
        ],
        "@type": "Lexical"
      },
      "@type": "Class",
      "capacity": "xsd:decimal",
      "name": "xsd:string"
    }
  }
```

### Create

```javascript
import { FrameViewer } from '@terminusdb/terminusdb-documents-ui'

  return <FrameViewer
    frame={frame}               // above defined frame          
    formData={{}}               // formData will be empty
    mode={"Create"}             // mode 
    type={"Guy"}/>           // type of document 
```

### Edit & View

Note - make sure to provide document values for View mode. The form will be in read only mode for View.

```javascript
let data = {
    "@id": "Guy/4489199036b83dbf79a6e7527a1594fbd416d11b9dde2f8a67fe6fa495dae433",
    "@type": "Guy",
    "favorite_group": "Art/Charcoal%20Art%20Group",
    "attends_group_in_order": [
        "Dance/Dance%20Everyday",
      "Art/Pastel%20Art%20Group",
      "Music/Music%2220Pop"
    ],
    "member_of": [
      "Art/Pastel%20Art%20Group",
      "Dance/Dance%20Everyday"
    ],
    "second_favorite_group": "Dance/Dance%20Everyday",
}

return <FrameViewer
  frame={frame}
  mode={"View"}
  formData={data}
  type={"Guy"}/>
```
