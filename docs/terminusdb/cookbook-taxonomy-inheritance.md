---
tags:
  - woql
  - cookbook
  - path-queries
  - advanced
title: "Cookbook: Taxonomy Inheritance and Property Inheritance"
nextjs:
  metadata:
    title: "Cookbook: Taxonomy Inheritance and Property Inheritance"
    description: Learn how to implement value inheritance and property inheritance in hierarchical data structures, including product taxonomies and legal entity relationships
    keywords: taxonomy, inheritance, property inheritance, hierarchical data, reference data, classification
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://medium.com/the-semantic-data-practitioner/build-advanced-collaborative-taxonomies-using-hypergraphs-f752403887db
media: []
---

In the enterprise world you’re often faced with digital business information that is hard to process using traditional approaches. Far too many treat their business information as other data without ensuring its structure, context and meaning during processing.

We believe that human agency can be improved by offering generic, highly performant and accurate platforms for rapid digital innovation.

All images were generated via the dfrnt.com data product builder.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*V1qW8MHyao1Gk2UmSqoYrw.png" alt="Visualisation showing how SKUs are connected through a product taxonomy. This is a narrow set, supporting the example where some record properties get inherited through the classes of the taxonomy." caption="Visualisation showing how SKUs are connected through a product taxonomy. This is a narrow set, supporting the example where some record properties get inherited through the classes of the taxonomy." %}
{% /figure %}

It is crucial to have generic problem-solving capabilities at hand. We believe digital exoskeletons is an apt term for what we provide, let us know in the comments below what you think of as more approachable terms!

There is a specific challenge that enterprise information architects and information modellers grapple with. It is the information at the heart of every enterprise. It includes product information, legal entity relationships within and beyond the company, and other reference data for classifying and labelling digital twins of phenomena in the enterprise.

## An AI for 100% correct answers, to a set of rules

These information structures often require custom-built precise information models tied to the specific competitive edge of the firm, advanced logic for retrieval, and special features such as value inheritance or materialisation between records, either following a taxonomical (classification) structure, or other more complex graph-based relationships. This is not suitable for anything less than 100% quality.

Granted, these concepts are not easy, and solving them in extensible ways is also far from easy. Subscriptions to systems to solve these kinds of problems often cost $100k or more, so insights into how to solve such issues is scattered and hard to come by.

As a founder building a business that can solve for these kinds of problems I decided to share some of the methods used for implementing value inheritance, methods that enables a large dose of flexibility.

## Examples of hierarchical records with inheritance

In my work I regularly encounter challenges related to property inheritance where the lifecycle of the information in the record is important to consider.

Records that inherit properties are often highly interlinked, such as the customer and supplier views of a legal entity, where that lifecycle of all three are linked (the company either exists, or it doesn’t).

Or, at other times, such reference data records have different lifecycles, such as in a product taxonomy where some categories and labels are either inherited through the tree, or overridden. The linkage must be there to a parent, but each record in the taxonomy enjoys an independent lifecycle.

Each of these cases require a specific logic and products that have open modeller and logic engines, where each case can be designed for to the precise needs.

We can segregate the above into two cases:

* the legal entity challenge, and
* the product taxonomy challenge.

We will use two different information models and similar logic for implementing their property inheritance. The reason we can use similar logic is that good systems enables information to be traversed regardless of record boundaries, as the information model is used for structural integrity, and can be referenced when needed.

### The legal entity challenge, to be both supplier and customer

A few years back in a master-data project we were working on problems related to legal entities where legal entities took on various roles in relation to the group of companies. Some of them were both suppliers and customers, and some even partners and/or part of the group of companies too. Not easy.

In essence, what is needed is the ability to build a data structure where you have a Legal Entity record. Many use a DUNS number as identifier across their systems, and some modern organisations are looking to use LEI numbers issued via the GLEIF system. A good practice is for the identifier to be globally unique and understandable at first glance, such as by using a standards-based IRI.

The Legal Entity record would then have connected sub-objects for the legal entity’s Supplier Record and Customer Record respectively (and more), that are part of the Legal Entity record. Ideally, they are connected to the lifecycle with the Legal Entity top-level record in the system-of-record. When the Legal Entity record is deleted, the more specific records of it will be deleted as well. Setting up such restrictions is important to support.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*5xUSuv-6mfgvdI17I9HqWQ.png" alt="Information on a Legal Entity Record could be mirrored into Supplier or Customer representations when defined as part of the Legal Entity document. And they should include relevant properties of LegalEntity." caption="Information on a Legal Entity Record could be mirrored into Supplier or Customer representations when defined as part of the Legal Entity document. And they should include relevant properties of LegalEntity." %}
{% /figure %}

Below is what a very simplified JSON-LD record of what a Legal Entity could look like. Note the customerEntity and supplierEntity subdocuments in the JSON-LD document that must share their lifecycle with the master record. This would be controlled through for example cascading deletes in a traditional database.

```json
{
  "@type": "LegalEntity",
  "duns": "351323425",
  "name": "DFRNT AB",
  "steward": "Person/JohnDoe",
  "customerEntity": {
    "@type": "CustomerEntity",
    "steward": "Person/JaneSmith"
  },
  "supplierEntity": {
    "@type": "SupplierEntity"
  }
}
```

Ideally, we should also be able to search for a SupplierEntity and see the duns and name from the LegalEntity main record inherited down, along with specific values on the supplier such as payment terms, that could be different than for the customer. And doing this without copying the value.

The record steward in this case is a link to a separate Person record, John Doe — COO, which should be an inherited property value in the Supplier Entity, but overridden by a specific steward, Jane Smith — head of sales, on the customer record.

Such logic would entail quite some work to get the information properly represented as a traditional database. Using structure-enforced hypergraphs, the best of both worlds is possible, hierarchical records tied to a model, with an object representation, but also the ability to traverse the records using logic.

### The product taxonomy challenge, inheriting classifications

In a product taxonomy, it’s important for all products to be classified in a tree to ensure consistency. Adding facets and structure-enforced relationships that understands the is-a relationships between records enables almost any flexibility. A common challenge to solve for is to tie the internal Linnaean classification view of the products in a taxonomy to a digital customer journey with a personalised experience based on customer characteristics, where such additional logic shines.

We have to differentiate between the internal view and the external view. Connecting a personalised customer segmentation, targeting and positioning of our products, will likely rest on both the taxonomy and its faceting. Such as what information should show up in specific experiences.

Consider a product taxonomy, here is a generic one with 6 levels that is sane enough, quickly generated using Microsoft Co-pilot as an industry-agnostic one. Such classifications enables easy faceted search and category browsing.

1. Product Category
   Example: Electronics, Apparel, Furniture, Vehicles
2. Product Subcategory
   Example for electronics: Smartphones, Laptops, Televisions
3. Product Line
   Example: Galaxy Series, ThinkPad Series, Bravia Series
4. Product Model
   Example: Galaxy S21, ThinkPad X1 Carbon, Bravia X90J
5. Product Variant
   Example: 128GB, 256GB; Black, Silver; 55-inch, 65-inch
6. Stock Keeping Unit (SKU)
   Example: Galaxy S21 128GB Black

Each Stock Keeping Unit would be fantastic to define in the same way, inheriting properties from the taxonomy but also keeping consistency across all SKU. For this example, we keep the Taxonomy Elements and Stock Keeping Units separate, but link them together.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*n976E91YPP3bT8IqejPijQ.png" alt="This is what the taxonomy looks like, with the SKUs for the examples on the left, all the way to the top level categories on the right" caption="This is what the taxonomy looks like, with the SKUs for the examples on the left, all the way to the top level categories on the right" %}
{% /figure %}

A common mistake to make is to not think through the relationship properly, each Taxonomy Element has an is-part-of relationship with the parent, and is not an is-a relationship tied to the type classifications (super-classing, sub-classing). This is why keep each type in a flat list as per below.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*Tqm3qld4cY2QI_gyKCuMSw.png" alt="The SKU has an isPartOf relationship to 5-ProductVariant, to 4, 3, 2, 1. SKU could arguably also be placed in the product taxonomy, here it was a preference to make the product taxonomy more manageable." caption="The SKU has an isPartOf relationship to 5-ProductVariant, to 4, 3, 2, 1. SKU could arguably also be placed in the product taxonomy, here it was a preference to make the product taxonomy more manageable." %}
{% /figure %}

All sub-classed types of the TaxonomyElements will include all properties from TaxonomyElement, including the faceting properties we want to support.

Each SKU should inherit the names of the taxonomy elements of each level, and it should be possible to ask which SKUs are part of a set of classifications in the taxonomy, such as SmartPhones, Galaxy Series etc. By tying a set of taxonomy elements to customer segmentations, it becomes possible to have a narrow working set for further personalisation of product recommendations. Connecting to the journey is for a later article.

A category field would enable category managers to be founds easily in the graph by adding category records. And an inheritance override model would enable flexible assignment across the hierarchy that could even be overridden per SKU if they also implement the property.

## Using logic for the inheritance “magic”

The ability to support inheritance varies across tools, and is implemented in various ways. Having the ability to implement it using logic makes it possible to change the rules later or add additional rules to the inheritance logic as insights and requirements develop.

The model presented hinges on the ability to perform a graph path traversal up the taxonomy tree to find the nearest parent with a value and materialise the record elements. This enables the ability to add additional constraints on the parent that has the value, such as skipping certain levels, or only inherit from parents that have certain properties set. The possibilities are endless by adding logical constructs to the endpoints.

A path traversal is specified by sharing which property name to follow in the graph, such as the “parent” property, and optionally how far to travel the graph if necessary, measured in record “hops”. In this simpler example we won’t need that.

### Getting CustomerEntity records from the LegalEntity master

The legal entity master contains records that represent all the companies connected to our business, and elegantly, they can have any add-on roles such as Supplier, Customer etc. This is all modelled using structure-enforced hierarchical records using dfrnt.com.

In the legal entity example, below logic would bring us information for all customers:

filter for all records that are (is-a) of the type CustomerEntity
ensure the parent record is-a LegalEntity record
get name either from this record, or from parent record, via path
get duns either from this record, or from parent record, via path
optionally, if defined, get steward either from this record, or from parent record, via path. Limit to the first solution found along the path.

By allowing the business logic to be expressed simply and the information, as above, it is easy to understand, and it becomes easier to make a more formal representation. Again, here is the simple model, which can also be exported as JSON Schema for use outside of the system.

{% figure src="https://miro.medium.com/v2/resize:fit:1400/format:webp/1*CVaKpw8vjWcLV38l1Qg8fw.png" alt="A UML diagram indicating composition of CustomerEntry and SupplierEntry to LegalEntity" caption="A UML diagram indicating composition of CustomerEntry and SupplierEntry to LegalEntity" %}
{% /figure %}

The duns, name and steward are mandatory on the LegalEntity record information model, to ensure there is at least one value in the hierarchy, and optional in the sub-objects. Illustration is directly from the tool.

Below is the equivalent logic expressed as WOQL, that performs a set of unifications on the variable names (prefixed by v:). The path has a direction pointer, and the star (*) means to follow the path 0 or more times, so that the own record is also traversed. The select statements means that the other variables will be discarded in the response and the < means direction of traversal. We add the CustomerEntity manually as @type for clarity.

```woql
select("v:name", "v:duns", "v:steward", "v:@type").
and(
  triple("v:customerId", "rdf:type", "@schema:CustomerEntity"),
  triple("v:legalEntityId", "customerEntity", "v:customerId"),
  triple("v:legalEntityId", "rdf:type", "@schema:LegalEntity"),
  eq("v:@type", "CustomerEntity"),
  limit(1).and(
    path("v:customerId", "<customerEntity*", "v:host-duns"),
    triple("v:host-duns", "duns", "v:duns"),
  ),
  limit(1).and(
    path("v:customerId", "<customerEntity*", "v:host-name"),
    triple("v:host-name", "name", "v:name"),
  ),
  opt().limit(1).and(
    path("v:customerId", "<customerEntity*", "v:host-steward"),
    triple("v:host-steward", "steward", "v:steward"),
  ),
)
```

In this example, we get the following record back:

```woql
{
  "@type": "CustomerEntity",
  "name": "DFRNT AB",
  "duns": "351323425",
  "steward": "Person/JaneSmith"
}
```

### Inheriting taxonomy information through the product taxonomy

In the previous section we explored getting information from a hierarchical record in multiple levels with a shared lifecycle. Next we will be dynamically traversing a taxonomy to get necessary information for presenting a SKU.

When presenting our SKU, we want to present below information based on the logic, including the category so that we can find the category manager:

* **filter** for all records that are (is-a) of the type SKU
* get **name** from the record itself
* get **sku** from the record itself
* get **ProductCategory** traversing via taxonomy parent paths
* get **ProductSubCategory** traversing via taxonomy parent paths
* get **ProductLine** traversing via taxonomy parent paths
* get **ProductModel** traversing via taxonomy parent paths
* get **ProductVariant** traversing via taxonomy parent paths
* **optionally**, if defined, get **steward** either from this record, or from parent record, via path. Limit to the first solution found along the path.

The above rules enables a rich presentation of the record including specific information determined by traversing the classification tree. By adding additional constraints, additional logic can easily be added.

The above rules would look like below, expressed in the WOQL declarative datalog logical query language:

```woql
and(
  eq("v:skuId", "SKU/00000001"),
  
  triple("v:skuId", "rdf:type", "@schema:SKU"),
  triple("v:skuId", "name", "v:name"),
  triple("v:skuId", "sku", "v:sku"),

  select("productCategory").and(
    path("v:skuId", "parent+>", "v:parent-category"),
    triple("v:parent-category", "rdf:type", "@schema:1-ProductCategory"),
    triple("v:parent-category", "name", "v:productCategory")
  ),
  select("productSubcategory").and(
    path("v:skuId", "parent+>", "v:parent-subcategory"),
    triple("v:parent-subcategory", "rdf:type", "@schema:2-ProductSubcategory"),
    triple("v:parent-subcategory", "name", "v:productSubcategory")
  ),
  select("productLine").and(
    path("v:skuId", "parent+>", "v:parent-line"),
    triple("v:parent-line", "rdf:type", "@schema:3-ProductLine"),
    triple("v:parent-line", "name", "v:productLine")
  ),
  select("productModel").and(
    path("v:skuId", "parent+>", "v:parent-model"),
    triple("v:parent-model", "rdf:type", "@schema:4-ProductModel"),
    triple("v:parent-model", "name", "v:productModel")
  ),
  select("productVariant").and(
    path("v:skuId", "parent+>", "v:parent-variant"),
    triple("v:parent-variant", "rdf:type", "@schema:5-ProductVariant"),
    triple("v:parent-variant", "name", "v:productVariant")
  ),
  select("steward").limit(1).opt().and(
    path("v:skuId", "parent*>", "v:host-steward"),
    triple("v:host-steward", "steward", "v:steward")
  )
)
```

And below is the result of the query. Note that we selected a single record, the first SKU, just to see the structure of the inherited records that we materialised. Jane Smith is set as the steward on the 1-ProductCategory level and was just set as a string name to simplify the example.

```json
{
    "skuId": "SKU/00000001",
    "name": "Bravia X90J - 55-inch",
    "sku": "00000001",
    "steward": "Jane Smith",
    "productCategory": "Electronics",
    "productSubcategory": "Televisions",
    "productLine": "Bravia Series",
    "productModel": "Bravia X90J",
    "productVariant": "Bravia X90J - 55-inch"
}
```

## Collaboration on business information

Working with business information and creating a system of record of companies, products and other reference data and metadata requires accurate information tied to a model and logic.

It is hard to store such information in regular databases due to the complexity of the information models. Storing such information in document databases or traditional knowledge graphs without structural enforcement makes record processing, editing and correctness hard to ensure as the structural integrity is hard to uphold in such systems.

Many say SHACL (shapes and constraints language for RDF databases) is just painful to author and use, and document databases lack advanced schema and information model capabilities. There is a strong need for a more database-like knowledge graph in the market, and this way of processing information helps engineers, metadata specialists and information professionals a lot.

The best systems not only solve for a structure-enforced knowledge graph with datalog logic, but also add version controlled collaboration that supports collaboration processes for such content graphs in a way equivalent to processes that software engineers use to exchange changes and modifications to code. But purpose-built for business information management, including safe roll-back using the version history and the ability to safely test changes in branches and merge them back to the main repository later if proven good.

We went so far as to enable collaborative aspects that also enables synchronisation into environments where traditional systems can’t reach, including git-like push and pull synchronisation of content repositories that includes one or more content graphs such as the ones in the examples.

## Conclusion

In this article we showed two powerful ways to use logic to perform advanced logic over a content graph to implement record inheritance.

The first example shows how to build single representations of complex phenomena, Legal Entities for Suppliers and Customers, with structure-enforced hierarchical records using attached subdocuments tied to a defined schema.

The second example built on the foundations to show how a taxonomy can be traversed and constrained using logic to implement inheritance of property values and bring complete records with significant flexibility.

The collaborative aspects of structure-enforced knowledge graphs hosted at DFRNT enable safe experimentation on critical information, which translates into significantly faster iterations, business innovation and to sharpen our customers’ competitive edge, with a generic ability to process business information into structure-enforced collaborative knowledge graphs.

If the schema definitions are interesting as well, add a comment below and I’ll add a second post digging deeper.

---

This page was adapted from a Medium article by `@hoijnet` with permission: [Build advanced collaborative taxonomies using hypergraphs](https://medium.com/the-semantic-data-practitioner/build-advanced-collaborative-taxonomies-using-hypergraphs-f752403887db).

Images are from the [dfrnt.com data product builder](https://dfrnt.com/hypergraph-content-studio/) in action, where the WOQL, and solution development was done.

---