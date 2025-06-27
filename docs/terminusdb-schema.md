# TerminusCMS Schema Reference Guide

The TerminusDB schema language enables documents and their relationships to be
specified using simple JSON syntax. This syntax makes it as easy as possible to
specify a JSON object to automatically convert to a graph. This approach enables
data to be viewed as collections of documents or as knowledge graphs of
interconnected objects.

### Schema Objects

A JSON object in TerminusDB schema is composed of **key-value** pairs.

#### Key

A key is one of two values, **keyword** or **property**, described in the table
below. The full schema definition is a stream or list of these values or JSON
objects.

#### Table: Types of keys

Key type

Example

Description

**keyword**

`@id`

Starts with `@`, has a value with a special meaning.

**property**

`name`

Does not start with `@`, has a value with a **range** type.

### Class definition

The basic unit of specification is a **class**. A class definition is a schema
object with the keyword `@type` with type value `Class`. The keyword `@id`
specifies the name of the class. The example below define a class named `Person`
with a property `name` of type `xsd:string`. Search XSD definitions for more
information about types.

#### Code: The basic unit of specification

    { 
        "@type" : "Class",
        "@id"   : "Person",
        "name"  : "xsd:string" 
    }

### Context object

The **context object** is a special schema object affecting the entire schema.
The context object is specified by the special `@type` value `@context`. An
example:

#### Code: The context object

    {   "@type"            : "@context",
        "@schema"          : "http://terminusdb.com/schema/woql#",
        "@base"            : "terminusdb://woql/data/",
        "xsd"              : "http://www.w3.org/2001/XMLSchema#",
        "@documentation"   : 
        {
            "@title"       : "WOQL schema",
            "@authors"     : ["Gavin Mendel-Gleason"],
            "@description" : "The WOQL schema providing a complete specification of the WOQL syntax. 
                             This enables: 
                             * WOQL queries to be checked for syntactic correctness.
                             * Storage and retrieval of queries. 
                             * Queries to be associated with data products.
                             * Helps to prevent errors and detect conflicts in merge of queries.",   
        } 
    }

This example does the following:

- Defines default prefixes in `@schema` and `@base` to use for the schema and
  data.
- Defines the prefix `xsd` enabling vocabulary based on different URL prefixes.
- For example, specify `xsd:string` to denote
  `http://www.w3.org/2001/XMLSchema#string`
- Documents the schema in the `@documentation` value, providing:
- `@title`
- `@authors`
- `@description`

#### Context Prefixes

All properties in the context object that do not start with `@`, such as `xsd`,
are URI definitions. They must be of the form shown below. Prefix and URI are
defined by their respective regular expressions. That is, a prefix has an
identifier starting with an alphabetic character followed by alphanumeric
characters. The URI has a protocol followed by valid URI characters. Each prefix
is paired with a URI.

    Prefix := ":alpha::alphaNum:*"
    URI    := ":alpha:alphaNum:*://:uriChar:*"

    {   ...
        Prefix : URI
        ... }

## Context keywords

A list of keywords used in the context object.

### @schema

The `@schema` keyword specifies the default URI expansion to use for all
elements of the schema. In the example below, the class name `NamedQuery`
expands to `http://terminusdb.com/schema/woql#NamedQuery`.

#### Code: Context keyword @schema

    { 
        "@type"       : "@context",
        "@schema"     : "http://terminusdb.com/schema/woql#",
        "@base"       : "terminusdb://woql/data/" 
    }
    { 
        "@id"         : "NamedQuery",
        "@type"       : "Class",
        "@key"        :
        { 
            "@type"   : "Lexical",
            "@fields" : [ "name" ]
        },
        "name"        : "xsd:string",
        "query"       : "Query" 
    }

### @base

`@base` specifies the default URI expansion used for all elements of instance
data. In the previous schema definition, and given the document in the instance
graph example below, the id `NamedQuery_my_query` expands to
`terminusdb://woql/data/NamedQuery_my_query`.

#### Code: A document in the instance graph

    { 
        "@type"     : "NamedQuery",
        "@id"       : "NamedQuery_my_query",
        "name"      : "my_query",
        "query"     : 
        { 
            "@type" : "True" 
        }
    }

### @documentation

`@documentation` specifies documentation global to the entire schema. See the
`@documentation` section in the previous context object example. The
`@documentation` tag can be a single value, or it can be a list with each
element having an additional `@langugage` tag. The `@language` tag must have an
IANA language code, and this will be used to select appropriate descriptions
when internationalising the schema.

The documentation section contains the keywords:

#### @title

The `@title` of the schema to display.

#### @description

A long-form `@description` of the purpose of the schema, the type of documents
contained in the schema, and keywords useful for searching for the type of
content that the schema encodes.

A list of strings of `@authors` involved in writing the schema.

#### @metadata

If you would like to add arbitrary JSON structured metadata to a schema, you can
place it in the `@metadata` field of the context object. This can be used to
store data product-wide information in a structured format. For instance:

    { "@type" : "@context",
      "@base" : "http://my_stuff/",
      "@schema" : "http://my_schema#",
      "@metadata" : { "configuration" : { "frob" : 29 } }
    }

#### @language

If you use the `@language` code, specific documentation results can appear in
different circumstances depending on the users language preferences.

An example of the `@language` tag for a context is as follows:

    { "@base": "terminusdb:///data/",
      "@schema": "terminusdb:///schema#",
      "@type": "@context",
      "@documentation" : [{
          "@language" : "en",
          "@title" : "Example Schema",
          "@description" : "This is an example schema. We are using it to demonstrate the ability to display information in multiple languages about the same semantic content.",
          "@authors" : ["Gavin Mendel-Gleason"]
       },
       {  "@language" : "ka",
          "@title" : "მაგალითი სქემა",
          "@description" : "ეს არის მაგალითის სქემა. ჩვენ ვიყენებთ მას, რათა ვაჩვენოთ ინფორმაციის მრავალ ენაზე ჩვენების შესაძლებლობა ერთი და იმავე სემანტიკური შინაარსის შესახებ.",
          "@authors" : ["გავინ მენდელ-გლისონი"]
       }
      ],
      "xsd" : "http://www.w3.org/2001/XMLSchema#"
    }

## Document definition keywords

A document definition includes several properties, and the keywords, prefixed
`@`, describing class behavior.

### @type

The `@type` of the object. At the schema level, this is one of: `Enum`, `Class`,
`TaggedUnion` and `Unit`.

### @metadata

If you would like to add arbitrary JSON structured metadata to a class, you can
place it in the `@metadata` field of the class. This can be used to direct
various approaches to display of the class, or associated information for
backend or front-ends which may have different requirements. It is generally
good practice to keep important metadata one level deeper in a JSON object so as
to leave space for other kinds of metadata. For instance:

    { "@type" : "Class",
      "@id" : "MyClass",
      "@metadata" : { "display_format" : { "colour" : "Blue", "size" : [100,400]}},
      "name" : "xsd:string" }

#### Render as Markdown

To render properties as Markdown to enable users to curate data using a Markdown
editor, specify `render_as` and the property in the `@metadata` field. The
example below shows a document with the ID `People` with the `desc` string
property set to render as Markdown:

    {
            "@id": "People",
            "@type": "Class",
            "@metadata": {
                "render_as": {
                    "desc": "markdown"
                }
            },
            "desc": {
                "@class": "xsd:string",
                "@type": "Optional"
            },
            },
            "name": {
                "@class": "xsd:string",
                "@type": "Optional"
            },
            "gender": {
                "@class": "xsd:string",
                "@type": "Optional"
        }

#### Order By

You can specify the order of properties within your schema so that it is
displayed in the document explorer in an order specified by you. Using
`order_by` in the `metadata` field in square brackets, list the property order
you want, for example:

    {
            "@id": "People",
            "@type": "Class",
            "@metadata": {
                "order_by": [
                    "name",
                    "hair_color",
                    "eye_color",
                    "gender",
                    "birth_year"
                ],
            },
    ...

### Class

`Class` designates a standard class document. It contains the definition of
several properties and keywords describing various class attributes. An example
of a class, and an instance of the class:

#### Code: An example of a class

    { 
        "@id"         : "Dog",
        "@type"       : "Class",
        "@base"       : "Dog_",
        "@key"        : 
        { 
            "@type"   : "Lexical",
            "@fields" : [ "name" ] 
        },
        "name"        : "xsd:string",
        "hair_colour" : "Colour" 
    }

#### Code: An example of a class instance

    { 
        "@type"       : "@context",
        "@base"       : "http://i/",
        "@schema"     : "http://s#" 
    }
    { 
        "@type"       : "Dog",
        "@id"         : "Dog_Cerberus",
        "name"        : "Cerberus",
        "hair_colour" : "Grey" 
    }

### Enum

An `Enum` is a non-standard class in which each instance is a simple URI with no
additional structure. To be a member of the class, you must be one of the
referent URIs. An `Enum` example with an extension `Blue` is s shown below. In
the database, the actual URI for an Enum is expanded with the preceding type
name, so the `Blue` extension becomes `http://s#PrimaryColour/Blue`

#### Code: An example of an enum class

    { 
        "@type"   : "Enum",
        "@id"     : "PrimaryColour",
        "@value" : 
        [
            "Red", 
            "Blue", 
            "Yellow"
        ] 
    }


    "Blue"

### TaggedUnion

A `TaggedUnion` specifies mutually exclusive properties. This is useful when
there is a disjoint choice between options.

Examples below of a schema with a TaggedUnion and a concrete TaggedUnion class
extension. In these examples, the `BinaryTree` class specifies a `TaggedUnion`
enabling a choice between a `leaf` (with no value), or a `node` class with a
value and branches.

#### Code: An example schema with a TaggedUnion

    { 
        "@type"     : "@context",
        "@base"     : "http://i/",
        "@schema"   : "http://s#" 
    }
    { 
        "@id"       : "BinaryTree",
        "@type"     : "TaggedUnion",
        "@base"     : "binary_tree_",
        "@key"      : 
        { 
            "@type" : "ValueHash" 
        },
        "leaf"      : "sys:Unit",
        "node"      : "Node" 
    }
    { 
        "@id"       : "Node",
        "@type"     : "Class",
        "@key"      : 
        { 
            "@type" : "ValueHash" 
        },
        "value"     : "xsd:integer",
        "left"      : "BinaryTree",
        "right"     : "BinaryTree" 
    }

#### Code: An example TaggedUnion class extension

    { 
        "@type"     : "Node",
        "value"     : 0,
        "left"      : 
        {
            "@type" : "BinaryTree",
            "leaf"  : []
        },
        "right": 
        { 
            "@type" : "BinaryTree",
            "leaf"  : []
        }
    }

### @oneOf

The `TaggedUnion` is a special case and syntactic sugar for the more general
case of collections of disjoint properties. These more complex cases can be
represented by inheriting from a number of `TaggedUnion`s, but they may also be
given explicitly using the `@oneOf` field, together with a Class.

The value of the `@oneOf` field is a set, so can be any number of documents all
of which have mutually disjoint properties, but which can coexist. Examples with
more than one disjoint property are given below.

#### Code: An example schema with @oneOf

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s#"
    }
    {
        "@id"        : "IntOrString",
        "@type"      : "Class",
        "@oneOf"     :
        {
            "integer": "xsd:integer",
            "string" : "xsd:string"
        }
    }
    {
        "@id"        : "Friend",
        "@type"      : "Class",
        "@key"       :
        {
            "@type"  : "Lexical",
            "@fields": ["name"]
        },
        "name"       : "xsd:string"
    }
    {
        "@id"        : "Toy",
        "@type"      : "Class",
        "@key"       :
        {
            "@type"  : "Lexical",
            "@fields": ["name"]
        },
        "name"       : "xsd:string"
    }
    {
        "@id"       : "Pet",
        "@type"     : "Class",
        "name"      : "xsd:string",
        "@oneOf"    : [
            {
                "cat" : "Toy",
                "dog" : "Friend"
            },
            {
                "employers" : "xsd:positiveInteger",
                "unemployed": "xsd:string"
            },
        ]
    }

#### Code: Examples of `@oneOf` class extensions

    {
        "@type"     : "IntOrString",
        "integer"   : 0
    }


    {
        "@type"     : "IntOrString",
        "string"    : "zero"
    }


    {
        "@type"    : "Pet",
        "cat"      : {
            "@type": "Toy",
            "name" : "ball of string"
        },
        "employers": 5
    }


    {
        "@type"    : "Pet",
        "dog"      : {
            "@type": "Person",
            "name" : "Jim"
        },
        "unemployed": "A house pet."
    }


    {
        "@type"     : "Pet",
        "string"    : "zero"
    }

But not:

    {
        "@type"     : "IntOrString",
        "integer"   : 0,
        "string"    : "zero"
    }

### Unit

The `Unit` type has a single extension `[]`. This is used when only the presence
of the property is interesting, but it has no interesting value. See the
`BinaryTree` in the
[TaggedUnion class extension](#codeanexampletaggedunionclassextension) example
above.

### @id

The `@id` key of a class defines the class name and identifier. The name
uniquely defines the class, enabling the class to be updated, retrieved, and
deleted. In the example below, the class is named `NamedQuery`. It does not have
a fully qualified URL or prefix, so it is implicitly based on the URI given for
`@schema`.

#### Code: The @id key of a class

    { 
        "@id"         : "NamedQuery",
        "@type"       : "Class",
        "@key"        : 
        { 
            "@type"   : "Lexical",
            "@fields" : [ "name" ] 
        },
        "name"        : "xsd:string",
        "query"       : "Query" 
    }

### @key

`@key` specifies the mechanism to define the `@id` of documents in the database,
similar to a primary key in relational database terms. Valid key types are
`Lexical`, `Hash`, `ValueHash`, `Random`.

If the key `@base` is specified in the class, then this is pre-pended to the
key. If this is a fully qualified URI then it is complete, otherwise, it is
combined with the value of `@base` from the context.

#### Lexical

A `Lexical` key specifies a URI name formed from a URI encoded combination of
all `@fields` arguments provided, in the order provided. An example is shown
below. With this key type (or key strategy) a URI is formed from the combination
of `first_name` and `last_name`. If `@base` is specified in the class, this is
prepended.

Given the [simple document definition](#codeasimpledocumentdefinition) below,
this will either generate (if `@id` is not supplied) or check that the URI
`http://example.com/people/Person_Hasdrupal_Barca` is the `@id` element.

#### Code: An example Lexical key

    { 
        "@type"         : "@context",
        "@schema"       : "http://example.com/people#",
        "@base"         : "http://example.com/people/" 
    }
    { 
        "@id"           : "Person",
        "@type"         : "Class",
        "@base"         : "Person_",
        "@key"          : 
        {
            "@type"     : "Lexical",
            "@fields"   : 
            [ 
                "first_name", 
                "last_name" 
            ]
        },
        "first_name"    : "xsd:string",
        "last_name"     : "xsd:string",
        "year_of_birth" : "xsd:gYear"
    }

#### Code: A simple document definition

    { 
        "@type"         : "Person",
        "first_name"    : "Hasdrupal",
        "last_name"     : "Barca",
        "year_of_birth" : "-245" 
    }

#### Hash

`Hash` is generated in the same way as `Lexical` except that values are first
hashed using the SHA-256 hash algorithm.

Use this where there:

- Are numerous items that form the key making the URI unwieldy.
- Is no need for the URI to inform the user of the content of the object.
- Is a requirement that data about the object is not be revealed by the key.

Define a `Hash` in the same way as the
[Lexical key strategy](#codeanexamplelexicalkey) example in the previous
section, replacing the `@key` `@type` value from `Lexical` to `Hash`.

Given the [simple document definition](#codeasimpledocumentdefinition) in the
previous section, the `@id` `Person_5dd7004081e437b3e684075fa3132542f5cd06c1` is
generated.

#### ValueHash

The `ValueHash` key generates a key defined as the downward transitive closure
of the directed acyclic graph from the root of the document. This means you can
produce a key that is entirely based on the entire data object. Note
`ValueHash`:

- Takes no additional keywords.
- Objects must be directed acyclic graphs, they **cannot be cyclic**.

In the example below, `ValueHash` is formed only from the value of
`layer:identifier`.

#### Code: An example ValueHash key

    { 
        "@id"              : "layer:Layer",
        "@type"            : "Class",
        "@documentation"   : 
        {
            "@comment"     : "A layer object which has the identifier used in storage.",
            "@properties"  : 
            { 
                "layer:identifier"  : "The layer identifier." 
            }
        },
        "@base"            : "layer_data:Layer_",
        "@key"             : 
        { 
            "@type"        : "ValueHash" 
        },
        "layer:identifier" : "xsd:string" 
    }

#### Random

Use `Random` as a convenient key type when an object has no important
characteristics that inform a key or does not need to be constructed such that
it is reproducible. In the example below, the `@key` `@type` is defined as
`Random`, meaning each new database that is added is unique regardless of label.

#### Code: An example of a Random key

    {
        "@id"                   : "UserDatabase",
        "@type"                 : "Class",
        "@documentation"        : 
        {
            "@comment"          : "A normal user database.",
            "@properties"       : 
            {
                "label"         : "The label name of the database.",
                "comment"       : "A comment associated with the database.",
                "creation_date" : "The time of creation of the database.",
                "state"         : "The system transaction state of the database." 
            }
        },
        "@inherits"             : "Database",
        "@key"                  : 
        {
            "@type"             : "Random" 
        },
        "label"                 : "xsd:string",
        "comment"               : "xsd:string",
        "creation_date"         : "xsd:dateTime",
        "state"                 : "DatabaseState" 
    }

### @documentation

Use `@documentation` to add documentation to the class and the property fields
or values of the class. The `@documentation` can either be an object, or a list
of objects with specified languages (and at most one default unspecified). An
example using multiple languages might be:

    { "@id" : "Example",
      "@type" : "Class",
      "@documentation" : [
         {
           "@label" : "Example",
           "@comment" : "An example class",
           "@properties" : { "name" : { "@label" : "name",
                                        "@comment" : "The name of the example object" },
                             "choice" : { "@label" : "choice",
                                          "@comment" : "A thing to choose" }}
         },
         {
            "@language" : "ka",
            "@label" : "მაგალითი",
            "@comment" : "მაგალითი კლასი",
            "@properties" : { "name" : { "@label" : "სახელი",
                                         "@comment" : "მაგალითის ობიექტის სახელი" },
                              "choice" : { "@label" : "არჩევანი",
                                           "@comment" : "რაც უნდა აირჩიოთ" }}
         }
      ],
      "name" : "xsd:string"
    }

The keywords of the `@documentation` object are `@comment` and either
`@properties` or `@values` for standard classes or `Enums` respectively. Each of
the `@properties` or `@values` can likewise have either a simple label, or an
object with `@label` and `@comment (as above)`.

For `Enum` we can write as follows:

    {
        "@id": "Pet",
        "@type": "Enum",
        "@documentation" : {
            "@comment" : "What kind of pet?",
            "@values" : {
                "dog" : "A doggie",
                "cat" : "A kitty"
            }
        },
       "@value" : ["dog","cat"]
    }

For a standard `Class` with one default language, we can write as follows:

    {
        "@id": "Person",
        "@type": "Class",
        "@documentation" : {
            "@comment" : "Information about people",
            "@values" : {
                "name" : "The persons name",
                "friends" : "The kinds of company someone keeps"
            }
        },
       "name" : "xsd:string",
       "friends" : {
           "@type" : "Set",
           "@class" : "Person"
       }
    }

The `@comment` is the class description.

#### @properties

The `@properties` keyword is a JSON object with pairs of the form:

    {
        "property_1" : "description_1",

        ...

        "property_n" : "description_n"
    }

or with properties pointing to JSON objects, as:

    {
        "property_1" : { "@label" : "description_1", "@comment" : "comment_1" },

        ...

        "property_n" : { "@label" : "description_2", "@comment" : "comment_2" }
    }

### @base

`@base` specifies a prefix to prepare to the `@key`. This prefix is absolute if
`@base` is a fully qualified URI, otherwise, it will, in turn, be prefixed by
the system-wide `@base` definition. In the example below, the `@base` for the
class is fully qualified after the `layer_data` prefix is expanded. This means
the layer URIs have the form `terminusdb://layer/data/Layer_` followed by a
random string.

#### Code: An example of the @base keyword

    { 
        "@type"            : "@context",
        "@documentation"   :
        {
            "@title"       : "The Ref schema",
            "@description" : "This is the Ref schema. It gives a specification for storage of references, branches and commits in our commit graph.",
            "@authors"     :
            [
                "Gavin Mendel-Gleason",
                "Matthijs van Otterdijk"
            ]
        },
        "@base"            : "terminusdb://ref/data/",
        "@schema"          : "http://terminusdb.com/schema/ref#",
        "layer"            : "http://terminusdb.com/schema/layer#",
        "layer_data"       : "terminusdb://layer/data/",
        "xsd"              : "http://www.w3.org/2001/XMLSchema#"
    }
    {
        "@id"              : "layer:Layer",
        "@type"            : "Class",
        "@documentation"   :
        {
            "@comment"     : "A layer object which has the identifier used in storage.",
            "@properties"  :
            {
                "layer:identifier" : "The layer identifier."
            }
        },
        "@base"            : "layer_data:Layer_",
        "@key"             :
        {
            "@type"        : "ValueHash"
        },
        "layer:identifier" : "xsd:string"
    }

### @subdocument

The `@subdocument` key is present with the value `[]` or it is not present.

A class designated as a sub-document is considered to be completely owned by its
containing document. It is not possible to directly update or delete a
subdocument, but it must be done through the containing document. Currently,
subdocuments **must have a key** that is `Random` or `ValueHash` (this
restriction may be relaxed in the future.)

See below for examples of a subdocument declaration in a schema, and a
corresponding subdocument.

#### Code: An example subdocument declaration

    { 
        "@type"        : "@context",
        "@base"        : "terminusdb://i/",
        "@schema"      : "terminusdb://s#" 
    }
    { 
        "@type"        : "Class",
        "@id"          : "Person",
        "age"          : "xsd:integer",
        "name"         : "xsd:string",
        "address"      : "Address" 
    }
    { 
        "@type"        : "Class",
        "@id"          : "Address",
        "@key"         : 
        {
            "@type"    : "Random"
        },
        "@subdocument" : [],
        "country"      : "xsd:string",
        "postal_code"  : "xsd:string",
        "street"       : "xsd:string"
    }

#### Code: An example subdocument

    { 
        "@type"           : "Person",
        "@id"             : "doug",
        "name"            : "Doug A. Trench",
        "address"         : 
        { 
            "@type"       : "Address",
            "country"     : "Neverlandistan",
            "postal_code" : "3",
            "street"      : "Cool Harbour lane"
        }
    }

### @abstract

The `@abstract` key is present with the value `[]` or it is not present.

An abstract class has no concrete referents. It provides a common superclass and
potentially several properties shared by all of its descendants. Create useful
concrete members using the `@inherits` keyword.

An example of the abstract keyword in a schema, and a concrete instance of the
`Person` class, but not of the `NamedEntity` class:

#### Code: An example of the abstract keyword

    { 
        "@type"     : "@context",
        "@base"     : "terminusdb://i/",
        "@schema"   : "terminusdb://s#" 
    }
    { 
        "@type"     : "Class",
        "@abstract" : [],
        "@id"       : "NamedEntity",
        "name"      : "xsd:string" 
    }
    { 
        "@type"     : "Person",
        "@id"       : "Person",
        "@inherits" : ["NamedEntity"] 
    }


    { 
        "@type" : "Person",
        "@id"   : "doug",
        "name"  : "Doug A. Trench" 
    }

### @inherits

`@inherits` enables classes to inherit properties (and the `@subdocument`
designation) from parent classes. It does **not** inherit key strategies.

This inheritance tree is also available as a `subsumption` relation in the WOQL
query language and provides semantics for **frames** in the **schema API**.

The range of `@inherits` can be a class or a list of classes. For example:

    { 
        ...,

        "@inherits" : "MyClass",

        ... 
    }

Or

    { 
        ...,

        "@inherits" : 
        [
            "MyFirstClass", "MySecondClass"
        ]

        ... 
    }

#### Multiple inheritance

Multiple inheritance is allowed as long as all inherited properties of the same
name have the same range class. If range classes conflict, the schema check
fails.

An example of inheritance of properties and an object meeting this
specification:

#### Code: An example of inheritance

    { 
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/" 
    }
    { 
        "@id"        : "RightHanded",
        "@type"      : "Class",
        "right_hand" : "xsd:string" 
    }
    { 
        "@id"        : "LeftHanded",
        "@type"      : "Class",
        "left_hand"  : "xsd:string" 
    }
    { 
        "@id"        : "TwoHanded",
        "@type"      : "Class",
        "@inherits"  : 
        [ 
            "RightHanded", "LeftHanded"
        ] 
    }


    { 
        "@type"      : "TwoHanded",
        "@id"        : "a two-hander",
        "left_hand"  : "Pretty sinister",
        "right_hand" : "But this one is dexterous" 
    }

## @unfoldable

The `@unfoldable` key is present with the value `[]` or it is not present.

In the document API, when retrieving documents, the default behavior is for any
linked document to be returned as an IRI, while subdocuments are fully unfolded
and returned as a nested document. With the `@unfoldable` option set, linked
documents will behave just like subdocuments, and will also be unfolded on
retrieval.

The `@unfoldable` option can only be set on a class which does not directly or
indirectly link to itself. This prevents a self-referencing document from being
unfolded infinitely.

The purpose of `@unfoldable` is to be able to treat linked (top-level) documents
as subdocuments in representation. Subdocuments can only be linked by one
document, its owner, whereas normal documents can be linked by any number of
other documents. If the desired result is to have a document linked by several
other documents, but still have it fully unfolded on retrieval like a
subdocument, use this option.

### Code: An example unfoldable

    {
        "@type"        : "@context",
        "@base"        : "terminusdb://i/",
        "@schema"      : "terminusdb://s#"
    }
    {
        "@type"        : "Class",
        "@id"          : "Person",
        "name"         : "xsd:string",
        "address"      : "Address"
    }
    {
        "@type"        : "Class",
        "@id"          : "Address",
        "@unfoldable"  : [],
        "country"      : "xsd:string",
        "postal_code"  : "xsd:string",
        "street"       : "xsd:string"
    }

#### Code: an example set of documents

    {
        "@type"        : "Address",
        "@id"          : "Address/1",
        "country"      : "Neverlandistan",
        "postal_code"  : "3",
        "street"       : "Cool Harbour lane"
    }

    {
       "@type"         : "Person",
       "@id"           : "Person/doug",
       "name"          : "Doug A. Trench",
       "address"       : "Address/1"
    }

    {
       "@type"         : "Person",
       "@id"           : "Person/phil",
       "name"          : "Phil A. Trench",
       "address"       : "Address/1"
    }

The above example shows both Doug and Phil using the same address document. On
retrieval of all Persons, the document API returns these documents:

    {
        "@type"        : "Person",
        "@id"          : "Person/doug",
        "name"         : "Doug A. Trench",
        "address"      : { "@type"       : "Address",
                           "@id"         : "Address/1",
                           "country"     : "Neverlandistan",
                           "postal_code" : "3",
                           "street"      : "Cool Hasrbour lane" }
    }
    {
        "@type"        : "Person",
        "@id"          : "Person/phil",
        "name"         : "Phil A. Trench",
        "address"      : { "@type"       : "Address",
                           "@id"         : "Address/1",
                           "country"     : "Neverlandistan",
                           "postal_code" : "3",
                           "street"      : "Cool Hasrbour lane" }
    }

The address is fully unfolded in both documents despite not being a subdocument.

## Class properties

All non-keywords are treated as properties of the class, with the form:

    <property> : <Class>

Or

    <property> : { "@type" : <TypeFamily>,  "@class" : <Class> }

#### Range classes

A range class is a concrete base type defined as any of the xsd types (see XSD),
or a class defined in the current schema, including the current class.

In the example range class below, `first_name` and `last_name` are strings,
`year_of_birth` is a year, and `friend` is any number of `Person` objects, in no
particular order and without duplication. Also, see below
[an example of a concrete set of documents](#codeanexampleofaconcretesetofdocuments)
with this form.

#### Code: An example range class

    { 
        "@type"         : "@context",
        "@schema"       : "http://example.com/people#",
        "@base"         : "http://example.com/people/" 
    }
    { 
        "@id"           : "Person",
        "@type"         : "Class",
        "@base"         : "Person/",
        "@key"          : 
        { 
            "@type"     : "Lexical",
            "@fields"   : 
            [ 
                "first_name", "last_name" 
            ]
        },
        "first_name"    : "xsd:string",
        "last_name"     : "xsd:string",
        "knows"         : 
        { 
            "@type"     : "Set", 
            "@class"    : "Person"
        },
        "year_of_birth" : "xsd:gYear" 
    }

#### Code: An example of a concrete set of documents

    { 
        "@type"         : "Person",
        "@id"           : "Person/Hasdrubal_Barca",
        "first_name"    : "Hasdrubal",
        "last_name"     : "Barca",
        "knows"         :  
        [
            "Person/Imilce_Barca",
            "Person/Hannibal_Barca"
        ],
        "year_of_birth" : "-245"
    }
    { 
        "@type"         : "Person",
        "@id"           : "Person/Imilce_Barca",
        "first_name"    : "Imilce",
        "last_name"     : "Barca",
        "knows"         :  
        [
            "Person/Hasdrupal_Barca",
            "Person/Hannibal_Barca"
        ],
        "year_of_birth" : "-255" 
    }
    { 
        "@type"         : "Person",
        "@id"           : "Person/Hannibal_Barca",
        "first_name"    : "Hannibal",
        "last_name"     : "Barca",
        "knows"         : 
        [
            "Person/Imilce_Barca","Person/Hannibal_Barca"
        ],
        "year_of_birth" : "-247" 
    }

#### JSON Type

Two special JSON types exist in TerminusDB. One is for use as a subdocument, and
is called `"sys:JSON"` and the type `"sys:JSONDocument"` which is used for type
level. Both allow un-constrained and untypechecked documents which can be stored
or retrieved as apparently unmodified JSON, but which are still indexed and
searchable using WOQL.

Ids for subdocuments of type `"sys:JSON"` are formed from a hash of the content,
meaning that subdocuments are shared if their content is the same.

However, those of type `"sys:JSONDocument"` are assigned a random id, such that
they can be retrieved, modified etc. Alternatively they can be assigned an id by
passing in an id of the form `{ "@id" : "JSONDocument/my_id_here", ...}` making
sure to use the prefix `"JSONDocument"` so as not to ensure we do not have any
id conflicts with other document types.

#### Code: An example of `"sys:JSON"`

    {
        "@type"      : "@context",
        "@schema"    : "http://example.com/people#",
        "@base"      : "http://example.com/people/"
    }
    {
        "@type"      : "Class",
        "@id"        : "Person",
        "name"       : "xsd:string",
        "metadata"   : "sys:JSON"
    }

We can now have a well typed `"Person"` which contains a metadata field of type
`"sys:JSON"` which is unconstrained JSON as follows:

    {
        "@type"      : "Person",
        "name"       : "John",
        "metadata"   : { "theme" : "Dark", "last_visit" : "10-01-02" }
    }

#### Code: An example of `"sys:JSONDocument"`

Using the `{ "json" : true }` option to the insert API, or using the TerminusDB
CLI with the `-j` or `--json=true` flag we can insert an arbitrary JSON
document.

Using the CLI we can write:

    echo '{ "size" : 12.0, "name" : "Bob" }' | terminusdb doc insert admin/example -j
    Document inserted ["terminusdb:///json/JSONDocument/9cb4de0ff0b46b6035149a6b763e087d6c59cba2b417de3eedfd26063bee6bdf"]

## Type families

Use type families to construct optionality or collections of values. Type
families are `List`, `Set`, `Array`, and `Optional`.

### Optional

Use `Optional` as a type family where a property is not required.

#### Code: An example of type family Optional

    { 
        "@type"      : "@context",
        "@schema"    : "http://example.com/people#",
        "@base"      : "http://example.com/people/" }

    { 
        "@type"      : "Class",
        "@id"        : "CodeBlock",
        "code"       : "xsd:string",
        "comment"    : 
        { 
            "@type"  : "Optional",
            "@class" : "xsd:string" 
        }
    }

Supply an optional `comment` field in `CodeBlock`. Both of the following
documents are valid:

    { 
        "@type"   : "CodeBlock",
        "@id"     : "my_code_block",
        "code"    : "print('hello world')",
        "comment" : "This is a silly bit of code" 
    }

OR

    { 
        "@type" : "CodeBlock",
        "@id"   : "my_code_block",
        "code"  : "print('hello world')" 
    }

### List

Use `List` to specify an ordered collection, with multiplicity, of values of a
class or datatype.

#### Code: An example of type family List

    { 
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/" 
    }
    { 
        "@id"        : "TaskList",
        "@type"      : "Class",
        "tasks"      : 
        { 
            "@type"  : "List",
            "@class" : "Task"
        } 
    }
    {
        "@id"        : "Task",
        "@type"      : "Class",
        "@key"       : "ValueHash",
        "name"       : "xsd:string" 
    }

An example of an object `Task` contained in a `List` of elements known as a
`TaskList`. This list is retrieved in the same order that it is inserted. It is
also capable of storing duplicates.

    { 
        "@id"   : "my_task_list",
        "@type" : "TaskList",
        "tasks" : 
        [
            {     
                "@type" : "Task", 
                "name"  : "Laundry" 
            },
            { 
                "@type" : "Task", 
                "name"  : "Take_Garage_Out" 
            }
        ]
    }

### Set

Use `Set` to specify an unordered set of values of a class or datatype.

#### Code: An example of type family Set

    { 
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/" 
    }
    { 
        "@id"        : "Person",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "friends"    : 
        { 
            "@type"  : "Set",
            "@class" : "Person" 
        }
    }

An example of an object `Person` that can have 0 to any number of friends. This
list has no order and is retrieved from the database in a potentially different
order. Inserted duplicates do not create additional linkages and only a single
of the multiple supplied results are returned.

    { 
        "@id"           : "Me",
        "@type"         : "Person",
        "friends"       : 
        [
            { 
                "@type" : "Person",
                "@id"   : "you",
                "name"  : "You" 
            },
            { 
                "@type" : "Person",
                "@id"   : "someone_else",
                "name"  : "Someone Else"
            }
        ]
    }

### Foreign Types

Use `Foreign` to specify types that are to be references to external data
products. Foreign types are types that are opaque in the current data product.
This allows us to give them identifiers although we don't actually store the
objects locally. Foreign types have _no_ referential integrity checking, and as
they refer to opaque identifiers, the schema is checked by the data product in
which they are referred.

A foreign type must be declared explicitly by giving the name of the type to be
treated as foreign using the `Foreign` designation in the schema.

#### Code: An example of adding a foreign Person type

For instance, to add a foreign type of type Person, we can write:

    { "@type" : "Foreign",
      "@id" : "Person"}

The actual definition of person might be given in its home data product as:

    { "@type" : "Person",
      "@key" : { "@type" : "Lexical",
                 "@fields" : [ "name " ] },
      "name" : "xsd:string" }

#### Code: An example creating and referring to a foreign type

From the command line we can see how an HR data product might interact with an
Events data product.

Create the HR data product:

    terminusdb db create admin/hr

Add the HR schema:

    echo '{ "@type" : "Class", "@id" : "Person", "@key" : { "@type" : "Lexical", "@fields" : ["name"]}, "name" : "xsd:string" }' | terminusdb doc insert admin/hr --graph_type=schema

Create the Events data product:

    terminusdb db create admin/events

Add events, and a foreign type designation:

    echo '{ "@type" : "Foreign", "@id" : "Person"}{ "@type" : "Class", "@id" : "Event", "date" : "xsd:date", "person" : "Person" }' | terminusdb doc insert admin/events --graph_type=schema

Add a person to HR:

    echo '{ "@type" : "Person", "name" : "Gavin" }' | terminusdb doc insert admin/hr

Add an event referring to the person:

    echo '{ "@type" : "Event", "date" : "2022-10-05", "person" : "terminusdb:///data/Person/Gavin"}' | terminusdb doc insert admin/events

Recover the event:

    terminusdb doc get admin/events --id='Event/9b3c5b174cb1f157dcdcedb692ed57f82ba31193fb81652dc602915732ae94e1'

### Cardinality

Use `Cardinality` to specify an unordered set of values of a class or datatype
in which the property has a limited number of elements as specified by the
cardinality constraint properties.

The relevant properties are:

#### `@cardinality`

When specified, the number of elements for the given property must be _exactly_
the cardinality specified. This is equivalent to specifying both
`@min_cardinality` and `@max_cardinality` as the same cardinality.

#### Code: An example of type family Cardinality with `@cardinality`

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/"
    }
    {
        "@id"        : "Person",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "friends"    :
        {
            "@type"  : "Cardinality",
            "@class" : "Person"
            "@cardinality" : 3
        }
    }

An example of an object `Person` that can have exactly three friends. As with
`Set` This list has no order and is retrieved from the database in a potentially
different order.

    {
        "@id"           : "Person/Me",
        "@type"         : "Person",
        "friends"       :
        [
            {
                "@type" : "Person",
                "@id"   : "Person/you",
                "name"  : "You"
            },
            {
                "@type" : "Person",
                "@id"   : "Person/someone_else",
                "name"  : "Someone Else"
            },
            {
                "@type" : "Person",
                "@id"   : "Person/Another",
                "name"  : "Another"
            }
        ]
    }

#### `@min_cardinality`

When specified, the number of elements for the given property must be _at least_
the cardinality specified.

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/"
    }
    {
        "@id"        : "Person",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "friends"    :
        {
            "@type"  : "Set",
            "@class" : "Person"
            "@min_cardinality" : 1
        }
    }

#### `@max_cardinality`

When specified, the number of elements for the given property must be _no more
than_ the cardinality specified.

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/"
    }
    {
        "@id"        : "Person",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "friends"    :
        {
            "@type"  : "Set",
            "@class" : "Person"
            "@min_cardinality" : 1
        }
    }

When set to 1, this is functionally equivalent to the `Optional` constraint.

### Array

Use `Array` to specify an ordered collection, with multiplicity, of values of a
class or datatype in which you may want random access to the data and which may
be multi-dimensional. `Array` is implemented with intermediate indexed objects,
with a `sys:value` and indexes placed at `sys:index`, `sys:index2`, …
`sys:indexN` for each of the array indices of the multi-dimensional array.
However when extracted as JSON they will appear merely as lists (possibly of
lists), with possible null values representing gaps in the array.

#### Code: An example of type family Array

    { 
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/" 
    }
    { 
        "@id"        : "GeoPolygon",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "coordinates"    : 
        { 
            "@type"  : "Array",
            "@dimensions" : 2,
            "@class" : "xsd:decimal" 
        }
    }

An example of a polygon object `GeoPolygon` points to a 2D array of coordinates
which specify a polygon encompassing the Phoneix Park.

    {
        "@id"           : "PhoenixPark",
        "@type"         : "GeoPolygon",
        "name"          : "The Pheonix Park",
        "coordinates"   :
        [
          [
            -6.3491535,
            53.3700669
          ],
          [
            -6.3364506,
            53.3717056
          ],
          [
            -6.349411,
            53.3699645
          ]
        ]
    }

## Inference

TerminusDB is equipped with a type inference engine that allows types to be
inferred under certain conditions.

The algorithm attempts to find a _unique_ type that can successfully be ascribed
to a document. In the event that no type is found, you will get an error that no
type applies. If _several_ types might apply, you will see the list of candidate
types in the error. If TerminusX is able to find the unique type which applies,
it will ascribe the type automatically.

Type ascription is perhaps most useful in cases in which abstract types are used
as ranges of a property, but in which there are only _sibling_ concrete types
that might apply. In this case, it is easy to ensure a unique typing for the
range class and improves the flexibility of the interface.

It should also be considered that the type being ascribed is based on the schema
_as it is_ when the document is inserted. For this reason, in some cases it may
be better to tag the document explicitly with the `@type` keyword.

#### Code: An example of type inference

Given the following schema:

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/"
    }
    {
        "@id"        : "Person",
        "@type"      : "Class",
        "name"       : "xsd:string",
        "friends"    :
        {
            "@type"  : "Set",
            "@class" : "Person"
        }
    }

We can insert the following document through the document interface:

    { "name" : "Gavin",
      "friends" : [ { "name" : "Tim"}, { "name" : "Julie" }] }

This document will be ascribed type `"Person"` and the two documents linked will
likewise be typed as `"Person"`

#### Code: An example of unambiguous inference

In the case of certain well-defined JSON documents schemata however, such as
GeoJSON, there is never a possibility of ambiguity and so the type-inferencing
helps to make it much more convenient.

    {
        "@type"      : "@context",
        "@base"      : "http://i/",
        "@schema"    : "http://s/"
    }
    {
        "@type" : "Class",
        "@id" : "Geometry",
        "@abstract" : []
    }
    {
        "@type" : "Enum",
        "@id" : "Point_Type",
        "@value" : [ "Point" ]
    }
    {
        "@type" : "Class",
        "@id" : "Point",
        "@inherits" : "Geometry",
        "type" : "Point_Type",
        "coordinates" : {
            "@type" : "Array",
            "@dimensions" : 1,
            "@class" : "xsd:decimal"
        }
    }

This schema provides the `"Point"` type with a singleton enum tag. This
singleton enum tag will help to uniquely assign the type.

We can then insert a point document which might be written as:

    {
        "type" : "Point",
        "coordinates" : [33.2,24.0]
    }
