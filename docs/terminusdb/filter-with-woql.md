---
tags:
  - woql
  - how-to
  - intermediate
title: How to filter with WOQL
nextjs:
  metadata:
    title: How to filter with WOQL
    description: A guide showing how to filter with WOQL in your TerminusDB projects
    keywords: terminusdb, condition, datalog, filter, filter with woql, query language, terminusdb query, where
    openGraph:
      images: https://assets.terminusdb.com/docs/woql-filter.png
    alternates:
      canonical: https://terminusdb.org/docs/filter-with-woql/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to filter query results using WOQL.
{% /callout %}

> **Prerequisites:** TerminusDB running on `localhost:6363` with the Star Wars dataset cloned. If you haven't done this yet, follow the [Explore a Real Dataset](/docs/explore-a-real-dataset/) tutorial (Steps 1–2), or run:
>
> ```bash
> curl -u admin:root -X POST http://localhost:6363/api/clone/admin/star-wars \
>   -H "Content-Type: application/json" \
>   -H "Authorization-Remote: Basic cHVibGljOnB1YmxpYw==" \
>   -d '{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "Star Wars", "comment": "Star Wars dataset"}'
> ```

Since WOQL is a datalog, filters are just part of the query. You can express negative information, or constraints on the variables in order to get a restriction down to the things you want.

For instance, we can write the following query in the query panel for the Star Wars demo:

```javascript
let v = Vars("person","person_name","vehicle","vehicle_name");
limit(10)
.select(v.person_name, v.vehicle_name)
  .and(triple(v.vehicle, "pilot", v.person),
       triple(v.vehicle, "label", v.vehicle_name),
       triple(v.person, "label", v.person_name))
```

This results in:

```json
[ {"person_name": {"@type":"xsd:string", "@value":"Chewbacca"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Han Solo"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Lando Calrissian"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Nien Nunb"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Luke Skywalker"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Wedge Antilles"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Jek Tono Porkins"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Biggs Darklighter"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Darth Vader"},
   "vehicle_name": {"@type":"xsd:string", "@value":"TIE Advanced x1"}},
  {"person_name": {"@type":"xsd:string", "@value":"Boba Fett"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Slave 1"}}
]
```

We can ask for a _specific_ example of a vehicle name by filtering on equality.

For instance:

```javascript
let v = Vars("person","person_name","vehicle","vehicle_name");
select(v.person_name, v.vehicle_name)
  .and(triple(v.vehicle, "pilot", v.person),
       triple(v.vehicle, "label", v.vehicle_name),
       triple(v.person, "label", v.person_name),
       eq(v.vehicle_name, string("Millennium Falcon")))
```

Which results in:

```json
[ {"person_name": {"@type":"xsd:string", "@value":"Chewbacca"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Han Solo"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Lando Calrissian"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}},
  {"person_name": {"@type":"xsd:string", "@value":"Nien Nunb"},
   "vehicle_name": {"@type":"xsd:string", "@value":"Millennium Falcon"}}
]
```

We can also write:

```javascript
let v = Vars("person","person_name","vehicle","vehicle_name");
select(v.person_name, v.vehicle_name)
  .and(triple(v.vehicle, "pilot", v.person),
       triple(v.vehicle, "label", v.vehicle_name),
       triple(v.person, "label", v.person_name),
       not(eq(v.vehicle_name, string("Millennium Falcon"))))
```

In which we get the complement of the above.

Or, we can use the regex operator to get a wider variety, for instance:

```javascript
let v = Vars("person","person_name","vehicle","vehicle_name","pattern");
select(v.person_name, v.vehicle_name)
  .and(triple(v.vehicle, "pilot", v.person),
       triple(v.vehicle, "label", v.vehicle_name),
       triple(v.person, "label", v.person_name),
       regex(".*wing", v.vehicle_name, [v.pattern]))
```

In this case, we get all vehicles whose names end in "wing" — the PCRE pattern `.*wing` matches any string ending with "wing":

```json
[ {"person_name": {"@type":"xsd:string", "@value":"Luke Skywalker"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Wedge Antilles"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Jek Tono Porkins"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Biggs Darklighter"},
   "vehicle_name": {"@type":"xsd:string", "@value":"X-wing"}},
  {"person_name": {"@type":"xsd:string", "@value":"Arvel Crynyd"},
   "vehicle_name": {"@type":"xsd:string", "@value":"A-wing"}}
]
```