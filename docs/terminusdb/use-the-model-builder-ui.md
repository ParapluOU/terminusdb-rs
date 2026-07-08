---
tags:
  - how-to
  - schema
  - dashboard
title: Use the Model Builder UI
nextjs:
  metadata:
    title: Use the Model Builder UI
    description: The model builder UI allows you to construct classes of objects and define what data they have, and what links (or relationships) they have between them.
    keywords: terminusdb, dashboard, data model, dfrnt hub, document type, schema, terminuscms, use the model builder ui
    openGraph:
      images: https://assets.terminusdb.com/docs/schema-ui-doc-properties.png
    alternates:
      canonical: https://terminusdb.org/docs/use-the-model-builder-ui/
media:
  - alt: Create a new data product using the TerminusDB dashboard
    caption: ""
    media_type: Image
    title: Create a new data product using the TerminusDB dashboard
    value: https://assets.terminusdb.com/docs/new-data-product.png
  - alt: JSON schema editor in the TerminusDB dashboard
    caption: ""
    media_type: Image
    title: JSON schema editor in the TerminusDB dashboard
    value: https://assets.terminusdb.com/docs/schema-ui-no-docs.png
  - alt: Add document properties using the schema builder UI
    caption: ""
    media_type: Image
    title: Add document properties using the schema builder UI
    value: https://assets.terminusdb.com/docs/schema-ui-doc-properties.png
  - alt: Add link properties to a document using the schema builder UI
    caption: ""
    media_type: Image
    title: Add link properties to a document using the schema builder UI
    value: https://assets.terminusdb.com/docs/schema-ui-doc-link-properties.png
  - alt: Create an enum for your schema using the schema builder UI
    caption: ""
    media_type: Image
    title: Create an enum for your schema using the schema builder UI
    value: https://assets.terminusdb.com/docs/schema-ui-doc-enum.png
  - alt: Add an enum property to a document using the schema builder UI
    caption: ""
    media_type: Image
    title: Add an enum property to a document using the schema builder UI
    value: https://assets.terminusdb.com/docs/schema-ui-doc-add-enum-property.png
---

{% callout type="note" %}
**Prerequisites**
- A DFRNT Hub account or TerminusDB with the dashboard
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have built a schema using the visual model builder UI.
{% /callout %}

## Make a New Data Product

First, log in to DFRNT TerminusDB cloud, choose (or create) a team, and then click on `New Data Product`.

![Create a new data product using the TerminusDB dashboard](https://assets.terminusdb.com/docs/new-data-product.png)

## Create a Class

Now click on the pink bubbles on the left panel. This takes you to the schema builder page.

Hover over the gray schema bubble in the center of the graph view.

![JSON schema editor in the TerminusDB dashboard](https://assets.terminusdb.com/docs/schema-ui-no-docs.png)

This will give you a `+` icon. This will allow you to add either a document class or an enum. Choose _document_.

The document will appear as an orange Square and on the right-hand side you will have a panel for editing the schema. You can choose a name for your new document class under the field `Unique ID*`.

Once you have chosen an id, you can (optionally) choose the _printed_ name of the document under `Label`.

## Add Properties

![Add document properties using the schema builder UI](https://assets.terminusdb.com/docs/schema-ui-doc-properties.png)

Now you can switch the properties tab, and click on `Add Property`. This will give you a choice of several different property types. You can choose `String` for instance for various string properties.

Again you will have to give it a unique id, and by default the property will be _optional_, but you can change this to _mandatory_, _list_ or _set_.

When you are done, click the Disk icon above (meaning save).

## Add Link Property

![Add link properties to a document using the schema builder UI](https://assets.terminusdb.com/docs/schema-ui-doc-link-properties.png)

You can also add a link property by choosing `Link Property` under the `AddProperty` selector once you have saved at least one document class.

You must again specify an ID, and link to an already created document class.

## Add Enum

![Create an enum for your schema using the schema builder UI](https://assets.terminusdb.com/docs/schema-ui-doc-enum.png)

You can add an enum by clicking the `+` on the gray schema bubble and selecting `Add Enum`.

After you have chosen a name for your enum, click on the `Values` tab on the right, and begin entering valid values for this enum.

## Add an Enum Property

![Add an enum property to a document using the schema builder UI](https://assets.terminusdb.com/docs/schema-ui-doc-add-enum-property.png)

Now it is possible to link to this enum from any document class. You can do this by selecting `Enum Property` under the `AddProperty` selector.