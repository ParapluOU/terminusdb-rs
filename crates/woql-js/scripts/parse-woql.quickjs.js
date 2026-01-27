(() => {
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __commonJS = (cb, mod) => function __require() {
    return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
  };

  // node_modules/@terminusdb/terminusdb-client/lib/utils.js
  var require_utils = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/utils.js"(exports2, module2) {
      var Utils = {};
      Utils.checkValidName = function(str2) {
        if (typeof str2 !== "string" || str2.trim() === "")
          return false;
        return /^[a-zA-Z0-9_]*$/.test(str2);
      };
      Utils.ACTIONS = {
        CREATE_DATABASE: "create_database",
        DELETE_DATABASE: "delete_database",
        SCHEMA_READ_ACCESS: "schema_read_access",
        SCHEMA_WRITE_ACCESS: "schema_write_access",
        INSTANCE_READ_ACCESS: "instance_read_access",
        INSTANCE_WRITE_ACCESS: "instance_write_access",
        COMMIT_READ_ACCESS: "commit_read_access",
        COMMIT_WRITE_ACCESS: "commit_write_access",
        META_READ_ACCESS: "meta_read_access",
        META_WRITE_ACCESS: "meta_write_access",
        CLASS_FRAME: "class_frame",
        BRANCH: "branch",
        CLONE: "clone",
        FETCH: "fetch",
        PUSH: "push",
        REBASE: "rebase"
        /* MANAGE_CAPABILITIES: 'manage_capabilities', */
      };
      Utils.encodeURISegment = function(str2) {
        if (typeof str2 !== "string")
          return str2;
        str2 = encodeURI(str2);
        str2 = str2.replace(/\?/g, "%3F");
        str2 = str2.replace(/&/g, "%26");
        str2 = str2.replace(/\+/g, "%2B");
        str2 = str2.replace(/#/g, "%23");
        return str2;
      };
      Utils.decodeURISegment = function(str2) {
        if (typeof str2 !== "string")
          return str2;
        str2 = str2.replace(/%3F/g, "?");
        str2 = str2.replace(/%2B/g, "+");
        str2 = str2.replace(/%23/g, "#");
        str2 = decodeURI(str2);
        return str2;
      };
      Utils.removeDocType = function(str2) {
        if (typeof str2 === "string" && str2.lastIndexOf("/") > -1) {
          return str2.substr(str2.lastIndexOf("/") + 1);
        }
        return str2;
      };
      Utils.standard_urls = {
        rdf: "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        rdfs: "http://www.w3.org/2000/01/rdf-schema#",
        xsd: "http://www.w3.org/2001/XMLSchema#",
        xdd: "http://terminusdb.com/schema/xdd#",
        owl: "http://www.w3.org/2002/07/owl#",
        system: "http://terminusdb.com/schema/system#",
        vio: "http://terminusdb.com/schema/vio#",
        repo: "http://terminusdb.com/schema/repository#",
        layer: "http://terminusdb.com/schema/layer#",
        woql: "http://terminusdb.com/schema/woql#",
        ref: "http://terminusdb.com/schema/ref#",
        api: "http://terminusdb.com/schema/api#"
      };
      Utils.URIEncodePayload = function(payload) {
        if (typeof payload === "string")
          return encodeURIComponent(payload);
        const payloadArr = [];
        for (const key of Object.keys(payload)) {
          if (typeof payload[key] === "object") {
            for (const keyElement of Object.keys(payload[key])) {
              const valueElement = payload[key][keyElement];
              payloadArr.push(
                `${encodeURIComponent(keyElement)}=${encodeURIComponent(valueElement)}`
              );
            }
          } else {
            payloadArr.push(`${encodeURIComponent(key)}=${encodeURIComponent(payload[key])}`);
          }
        }
        return payloadArr.join("&");
      };
      Utils.addURLPrefix = function(prefix, url) {
        this.standard_urls[prefix] = url;
      };
      Utils.empty = function(obj) {
        if (!obj)
          return true;
        if (obj.length > 0)
          return false;
        if (obj.length === 0)
          return true;
        if (typeof obj === "object") {
          for (const key of Object.keys(obj)) {
            if (Object.prototype.hasOwnProperty.call(obj, key))
              return false;
          }
        }
        return true;
      };
      Utils.genBNID = function(base) {
        base = base || "";
        const r = Math.random().toString(36).substring(7);
        const d = /* @__PURE__ */ new Date();
        const bnid = `${base}${r}${d.getTime()}`;
        return bnid;
      };
      Utils.getShorthand = function(link) {
        if (typeof link === "object" && link.length)
          link = link[0];
        if (link && typeof link === "string") {
          for (const pref2 of Object.keys(this.standard_urls)) {
            const full = this.standard_urls[pref2];
            if (link.substring(0, full.length) === full) {
              const sh = `${pref2}:${link.substring(full.length)}`;
              return sh;
            }
          }
        }
        return false;
      };
      Utils.compareIDs = function(ida, idb) {
        if (ida === idb)
          return true;
        if (this.unshorten(ida) === idb)
          return true;
        if (this.shorten(ida) === idb)
          return true;
        if (this.unshorten(ida) === this.unshorten(idb))
          return true;
        return false;
      };
      Utils.shorten = function(url, prefixes) {
        if (!url)
          return void 0;
        if (url.indexOf("#") !== -1) {
          const urlArr = url.split("#");
          return urlArr.pop();
        }
        if (url.indexOf("terminusdb://") !== -1) {
          const urlArr = url.split("/");
          return urlArr.pop();
        }
        return url;
      };
      Utils.unshorten = function(url) {
        if (!url)
          return void 0;
        if (this.validURL(url))
          return url;
        if (!url)
          return url;
        const bits = url.split(":");
        if (bits[1]) {
          if (this.standard_urls[bits[0]]) {
            return this.standard_urls[bits[0]] + bits[1];
          }
        }
        return url;
      };
      Utils.json_shorten = function(jsonld, prefixes) {
        const shorten_json_val = (val, prefixes2) => {
          if (Array.isArray(val)) {
            const nvals = [];
            for (let i = 0; i < val.length; i++) {
              nvals.push(shorten_json_val(val[i], prefixes2));
            }
            return nvals;
          }
          if (typeof val === "object") {
            return this.json_shorten(val, prefixes2);
          }
          if (typeof val === "string") {
            return this.shorten(val, prefixes2);
          }
          return val;
        };
        prefixes = prefixes || jsonld["@context"];
        const nujson = {};
        for (const key in jsonld) {
          const nkey = key[0] === "@" ? key : this.shorten(key, prefixes);
          const nval = shorten_json_val(jsonld[key], prefixes);
          nujson[nkey] = nval;
        }
        return nujson;
      };
      Utils.json_unshorten = function(jsonld, prefixes) {
        const unshorten_json_val = (val, prefixes2) => {
          if (Array.isArray(val)) {
            const nvals = [];
            for (let i = 0; i < val.length; i++) {
              nvals.push(unshorten_json_val(val[i], prefixes2));
            }
            return nvals;
          }
          if (typeof val === "object") {
            return this.json_unshorten(val, prefixes2);
          }
          if (typeof val === "string") {
            return this.unshorten(val, prefixes2);
          }
          return val;
        };
        prefixes = prefixes || jsonld["@context"];
        const nujson = {};
        for (const key in jsonld) {
          const nkey = key[0] === "@" ? key : this.unshorten(key, prefixes);
          const nval = unshorten_json_val(jsonld[key], prefixes);
          nujson[nkey] = nval;
        }
        return nujson;
      };
      Utils.validURL = function(str2) {
        if (str2 && typeof str2 !== "string")
          str2 = `${str2}`;
        if (str2 && (str2.substring(0, 7) === "http://" || str2.substring(0, 8) === "https://"))
          return true;
        return false;
      };
      Utils.isIRI = function(str2, context, allow_shorthand) {
        if (!str2)
          return false;
        if (allow_shorthand && context && context[str2.split(":")[0]])
          return true;
        if (context) {
          for (pref in context) {
            if (str2.substring(0, context[pref].length) === context[pref])
              return true;
          }
        }
        const prot = str2.split(":")[0];
        const valids = ["http", "https", "terminusdb"];
        if (valids.indexOf(prot) !== -1)
          return true;
        return false;
      };
      Utils.labelFromURL = function(url) {
        let nurl = this.urlFragment(url);
        nurl = nurl || url;
        if (nurl.lastIndexOf("/") < nurl.length - 1) {
          nurl = nurl.substring(nurl.lastIndexOf("/") + 1);
        }
        nurl = nurl.replace(/_/g, " ");
        return nurl.charAt(0).toUpperCase() + nurl.slice(1);
      };
      Utils.labelFromVariable = function(v) {
        v = v.replace(/_/g, " ");
        return v.charAt(0).toUpperCase() + v.slice(1);
      };
      Utils.urlFragment = function(url) {
        url = typeof url !== "string" ? window.location.href : url;
        let bits = url.split("#");
        if (bits.length <= 1) {
          if (!this.validURL(url)) {
            bits = url.split(":");
          }
        }
        if (bits.length >= 1) {
          const [, urlStr] = bits;
          if (urlStr) {
            const [baseUrl] = urlStr.split("?");
            url = baseUrl;
          }
        }
        return url;
      };
      Utils.lastURLBit = function(url) {
        url = typeof url === "undefined" ? window.location.href : url;
        const [urlFirst] = url.split("#");
        const [urlTmp] = urlFirst.split("?");
        url = urlTmp.substring(url.lastIndexOf("/") + 1);
        return url;
      };
      Utils.getStdURL = function(pref2, ext, url) {
        if (this.standard_urls[pref2]) {
          if (url) {
            if (url === this.standard_urls[pref2] + ext)
              return url;
          } else {
            return this.standard_urls[pref2] + ext;
          }
        }
        return false;
      };
      Utils.addNamespacesToVariables = function(vars) {
        const nvars = [];
        for (let i = 0; i < vars.length; i++) {
          if (vars[i])
            nvars.push(this.addNamespaceToVariable(vars[i]));
        }
        return nvars;
      };
      Utils.addNamespaceToVariable = function(v) {
        if (typeof v === "string" && v.substring(0, 2) !== "v:")
          return `v:${v}`;
        return v;
      };
      Utils.removeNamespaceFromVariable = function(mvar) {
        if (mvar.substring(0, 2) === "v:")
          return mvar.substring(2);
        return mvar;
      };
      Utils.removeNamespacesFromVariables = function(vars) {
        const nvars = [];
        for (let i = 0; i < vars.length; i++) {
          nvars.push(this.removeNamespaceFromVariable(vars[i]));
        }
        return nvars;
      };
      Utils.getConfigValue = function(val, row) {
        if (typeof val === "string")
          val = this.removeNamespaceFromVariable(val);
        if (typeof val === "string" && row[val]) {
          const rad = row[val];
          if (rad && rad["@value"])
            return rad["@value"];
          return rad;
        }
        return val;
      };
      Utils.TypeHelper = {};
      Utils.TypeHelper.isStringType = function(stype) {
        if (stype === "http://www.w3.org/2001/XMLSchema#string")
          return true;
        if (stype === "xsd:string")
          return true;
        return false;
      };
      Utils.TypeHelper.isDatatype = function(stype) {
        const sh = Utils.shorten(stype);
        if (sh && sh.substring(0, 4) === "xsd:" || sh.substring(0, 4) === "xdd:")
          return true;
        return false;
      };
      Utils.TypeHelper.numberWithCommas = function(value, separator) {
        separator = separator || ",";
        if (value >= 1e3 || value <= -1e3) {
          const parts = value.toString().split(".");
          if (value <= -1e3)
            parts[0] = parts[0].substring(1);
          parts[0] = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, separator);
          if (value <= -1e3)
            value = `-${parts.join(".")}`;
          else
            value = parts.join(".");
        }
        return value;
      };
      Utils.TypeHelper.formatBytes = function(bytes, decimals = 2) {
        if (bytes === 0)
          return "0 Bytes";
        const k = 1024;
        const dm = decimals < 0 ? 0 : decimals;
        const sizes = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return `${parseFloat((bytes / k ** i).toFixed(dm))} ${sizes[i]}`;
      };
      Utils.TypeHelper.datatypes = [
        "xdd:coordinate",
        "xdd:coordinatePolyline",
        "xdd:coordinatePolygon",
        "xdd:dateRange",
        "xdd:gYearRange",
        "xdd:integerRange",
        "xdd:decimalRange",
        "xdd:cc",
        "xdd:email",
        "xdd:html",
        "xdd:url",
        "xsd:anySimpleType",
        "xsd:string",
        "xsd:boolean",
        "xsd:decimal",
        "xsd:double",
        "xsd:float",
        "xsd:time",
        "xsd:date",
        "xsd:dateTime",
        "xsd:dateTimeStamp",
        "xsd:gYear",
        "xsd:gMonth",
        "xsd:gDay",
        "xsd:gYearMonth",
        "xsd:gMonthDay",
        "xsd:duration",
        "xsd:yearMonthDuration",
        "xsd:dayTimeDuration",
        "xsd:byte",
        "xsd:short",
        "xsd:integer",
        "xsd:long",
        "xsd:unsignedByte",
        "xsd:unsignedInt",
        "xsd:unsignedLong",
        "xsd:nonNegativeInteger",
        "xsd:positiveInteger",
        "xsd:negativeInteger",
        "xsd:nonPositiveInteger",
        "xsd:base64Binary",
        "xsd:anyURI",
        "xsd:language",
        "xsd:normalizedString",
        "xsd:token",
        "xsd:NMTOKEN",
        "xsd:Name",
        "xsd:NCName",
        "xsd:NOTATION",
        "xsd:QName",
        "xsd:ID",
        "xsd:IDREF",
        "xsd:ENTITY",
        "rdf:XMLLiteral",
        "rdf:PlainLiteral",
        "rdfs:Literal",
        "xdd:json"
      ];
      Utils.TypeHelper.parseRangeValue = function(val, dividor) {
        dividor = dividor || ",";
        let vals = [];
        if (typeof val === "object" && val.length) {
          vals = val;
        } else if (val) {
          if (typeof val !== "string") {
            val = `${val}`;
          }
          if (val.length && val.charAt(0) === "[" && val.charAt(val.length - 1) === "]") {
            vals.push(val.substring(1, val.indexOf(dividor)));
            vals.push(val.substring(val.indexOf(dividor) + 1, val.length - 1));
          } else {
            vals.push(val);
          }
        }
        return vals;
      };
      Utils.DateHelper = {};
      Utils.DateHelper.parseXsdTime = function(val) {
        if (!val)
          return {};
        const tz = this.extractXsdTimezone(val);
        if (tz) {
          val = val.substring(0, val.length - tz.length);
        }
        const parsed = {
          hour: val.substring(0, 2),
          minute: val.substring(3, 5),
          second: val.substring(6),
          timezone: tz
        };
        return parsed;
      };
      Utils.DateHelper.parseXsdDate = function(val) {
        const tz = this.extractXsdTimezone(val);
        if (tz) {
          val = val.substring(0, val.length - tz.length);
        }
        let year;
        if (val.substring(0, 1) === "-") {
          year = val.substring(0, 5);
        } else {
          year = val.substring(0, 4);
        }
        let parsed;
        if (year && Math.abs(year) < 1e4) {
          let month = val.substring(year.length + 1, year.length + 3);
          if (month) {
            month = parseInt(month, 10);
          } else
            return false;
          let day = val.substring(year.length + 4);
          if (day)
            day = parseInt(day, 10);
          else
            return false;
          parsed = {
            year,
            month,
            day,
            timezone: tz
          };
        }
        return parsed;
      };
      Utils.DateHelper.parseDate = function(ty, value) {
        let parsed;
        if (ty === "xsd:date") {
          parsed = this.parseXsdDate(value);
        } else if (ty === "xsd:time") {
          parsed = this.parseXsdTime(value);
        } else if (ty === "xsd:dateTime") {
          parsed = this.parseXsdDateTime(value);
        } else if (ty === "xsd:gYear") {
          parsed = { year: value };
        } else if (ty === "xsd:gYearRange") {
          parsed = { year: value };
        } else if (ty === "xsd:gMonth") {
          parsed = { month: value.substring(2) };
        } else if (ty === "xsd:gDay") {
          parsed = { day: value };
        } else if (ty === "xsd:gYearMonth") {
          const bits = value.split("-");
          while (bits.length < 2)
            bits.push("");
          parsed = { year: bits[0], month: bits[1] };
        } else if (ty === "xsd:gMonthDay") {
          const bits = value.split("-");
          while (bits.length < 2)
            bits.push("");
          parsed = { month: bits[0], day: bits[1] };
        } else if (ty === "xsd:dateTimeStamp") {
          parsed = this.parseXsdDateTime(value);
        }
        return parsed;
      };
      Utils.DateHelper.addXsdPadding = function(parsed) {
        const nparsed = {};
        if (typeof parsed.year !== "undefined" && parsed.year !== false && parsed.year < 1e3) {
          if (Math.abs(parsed.year) < 10)
            nparsed.year = parsed.year < 0 ? `-000${Math.abs(parsed.year)}` : `000${parsed.year}`;
          else if (Math.abs(parsed.year) < 100)
            nparsed.year = parsed.year < 0 ? `-00${Math.abs(parsed.year)}` : `00${parsed.year}`;
          else
            nparsed.year = parsed.year < 0 ? `-0${Math.abs(parsed.year)}` : `0${parsed.year}`;
        } else if (parsed.year) {
          nparsed.year = parsed.year;
        }
        if (typeof parsed.month !== "undefined" && parsed.month !== false && parsed.month < 10) {
          nparsed.month = `0${parsed.month}`;
        } else if (parsed.month) {
          nparsed.month = parsed.month;
        }
        if (typeof parsed.day !== "undefined" && parsed.day !== false && parsed.day < 10) {
          nparsed.day = `0${parsed.day}`;
        } else if (parsed.day) {
          nparsed.day = parsed.day;
        }
        if (typeof parsed.hour !== "undefined" && parsed.hour !== false && parsed.hour < 10) {
          nparsed.hour = `0${parsed.hour}`;
        } else if (parsed.hour) {
          nparsed.hour = parsed.hour;
        }
        if (typeof parsed.minute !== "undefined" && parsed.minute !== false && parsed.minute < 10) {
          nparsed.minute = `0${parsed.minute}`;
        } else if (parsed.minute) {
          nparsed.minute = parsed.minute;
        }
        if (typeof parsed.second !== "undefined" && parsed.second !== false && parsed.second < 10) {
          nparsed.second = `0${parsed.second}`;
        } else if (parsed.second) {
          nparsed.second = parsed.second;
        }
        return nparsed;
      };
      Utils.DateHelper.xsdFromParsed = function(parsed, ty) {
        const xparsed = this.addXsdPadding(parsed);
        let ret;
        if (ty === "xsd:gYear") {
          ret = xparsed.year ? xparsed.year : false;
        } else if (ty === "xsd:time") {
          return xparsed.hour && xparsed.minute && xparsed.second ? `${xparsed.hour}:${xparsed.minute}:${xparsed.second}` : false;
        } else if (ty === "xsd:date") {
          return xparsed.year && xparsed.month && xparsed.day ? `${xparsed.year}-${xparsed.month}-${xparsed.day}` : false;
        } else if (ty === "xsd:dateTime" || ty === "xsd:dateTimeStamp") {
          ret = xparsed.year && xparsed.month && xparsed.day ? `${xparsed.year}-${xparsed.month}-${xparsed.day}T` : false;
          if (ret) {
            ret += `${xparsed.hour ? xparsed.hour : "12"}:${xparsed.minute ? xparsed.minute : "00"}:${xparsed.second ? xparsed.second : "00"}`;
          }
        } else if (ty === "xsd:gMonth") {
          ret = xparsed.month ? `--${xparsed.month}` : false;
        } else if (ty === "xsd:gDay") {
          ret = xparsed.day ? `---${xparsed.day}` : false;
        } else if (ty === "xsd:gYearMonth") {
          ret = xparsed.year && xparsed.month ? `${xparsed.year}-${xparsed.month}` : false;
        } else if (ty === "xsd:gMonthDay") {
          ret = xparsed.day && xparsed.month ? `--${xparsed.month}-${xparsed.day}` : false;
        }
        if (xparsed.timezone) {
          ret += xparsed.timezone;
        }
        return ret;
      };
      Utils.DateHelper.convertTimestampToXsd = function(val) {
        const a = new Date(val * 1e3);
        const parsed = {
          year: a.getFullYear(),
          month: a.getMonth() + 1,
          day: a.getDate(),
          hour: a.getHours(),
          minute: a.getMinutes(),
          second: a.getSeconds()
        };
        return parsed;
      };
      Utils.DateHelper.parseXsdDateTime = function(val) {
        if (!val)
          return {};
        if (typeof val === "number") {
          return this.convertTimestampToXsd(val);
        }
        const tz = this.extractXsdTimezone(val);
        if (tz) {
          val = val.substring(0, val.length - tz.length);
        }
        const datetime = this.parseXsdDate(val);
        const ptime = this.parseXsdTime(val.substring(val.indexOf("T") + 1));
        for (const i of Object.keys(ptime)) {
          datetime[i] = ptime[i];
        }
        datetime.timezone = tz;
        return datetime;
      };
      Utils.DateHelper.extractXsdTimezone = function(val) {
        if (typeof val === "string" && val.endsWith("Z")) {
          return "Z";
        }
        if (typeof val === "string" && (val.charAt(val.length - 6) === "+" || val.charAt(val.length - 6) === "-")) {
          val.substring(val.length - 6);
        }
        return false;
      };
      module2.exports = Utils;
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlPrinter.js
  var require_woqlPrinter = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlPrinter.js"(exports2, module2) {
      function WOQLPrinter(vocab, language) {
        this.vocab = vocab;
        this.language = language;
        this.indent_spaces = 4;
        this.boxed_predicates = [
          "variable",
          "array_element",
          "node"
          // 'woql:arithmetic_value',
          // 'woql:variable_name',
        ];
        this.subject_cleaned_predicates = ["subject", "element"];
        this.schema_cleaned_predicates = [
          "predicate",
          "uri",
          "of_type"
        ];
        this.list_operators = ["ValueList", "Array", "NamedAsVar", "IndexedAsVar", "AsVar"];
        this.query_list_operators = ["And", "Or"];
        this.operator_maps = {
          IDGenerator: "idgen",
          IsA: "isa",
          PostResource: "post",
          QueryResource: "remote",
          AsVars: "as",
          NamedAsVars: "as",
          IndexedAsVars: "as",
          DeletedTriple: "removed_triple"
        };
        this.shortcuts = {
          optional: "opt",
          substring: "substr",
          regexp: "re",
          subsumption: "sub",
          equals: "eq",
          concatenate: "concat"
        };
        this.pythonic = {
          and: "woql_and",
          or: "woql_or",
          as: "woql_as",
          with: "woql_with",
          from: "woql_from",
          not: "woql_not"
        };
        this.show_context = false;
      }
      WOQLPrinter.prototype.printJSON = function(json, level, fluent, newline) {
        level = level || 0;
        fluent = fluent || false;
        let str2 = "";
        if (!json["@type"]) {
          console.log("Bad structure passed to print json, no type: ", json);
          return "";
        }
        if (["Value", "NodeValue", "DataValue", "ArithmeticValue", "OrderTemplate"].indexOf(json["@type"]) > -1) {
          return this.pvar(json);
        }
        let operator = json["@type"];
        if (typeof json["@type"] === "string" && operator.indexOf(":") > -1) {
          operator = json["@type"].split(":")[1];
        }
        if (operator === "QueryResource") {
          return this.getQueryResourceStr(json, level, fluent, newline);
        }
        if (operator) {
          const ujson = this.unboxJSON(operator, json);
          if (ujson) {
            const meat = this.printArgument(
              operator,
              this.getBoxedPredicate(operator, json),
              ujson,
              level,
              fluent
            );
            if (this.isListOperator(operator))
              return `[${meat}]`;
            return meat;
          }
          if (this.isListOperator(operator)) {
            str2 += "[";
          } else {
            const call = this.getFunctionForOperator(operator, json);
            const indent = newline ? level * this.indent_spaces : 0;
            str2 += `${this.getWOQLPrelude(call, fluent, indent)}(`;
          }
          const args = this.getArgumentOrder(operator, json);
          const divlimit = args.indexOf("query") === -1 ? args.length - 1 : args.length - 2;
          args.forEach((item, i) => {
            let nfluent = !!(item === "query" && operator !== "Put" || item === "consequent" || item === "resource");
            if (item === "resource" && typeof json[item] === "string")
              nfluent = false;
            str2 += this.printArgument(operator, item, json[item], level, nfluent);
            if (i < divlimit && operator !== "Get")
              str2 += ", ";
          });
          if (this.isListOperator(operator))
            str2 += "]";
          else {
            if (this.argumentTakesNewline(operator))
              str2 += `
${nspaces(level * this.indent_spaces)}`;
            if (!fluent)
              str2 += ")";
          }
        } else {
          console.log("wrong structure passed to print json ", json);
        }
        return str2;
      };
      WOQLPrinter.prototype.getQueryResourceStr = function(json, level, fluent, newline) {
        if (!json.source) {
          console.log("wrong structure passed to print json ", json);
          return "";
        }
        const functName = json.source.url ? "remote" : "file";
        const indent = newline ? level * this.indent_spaces : 0;
        let str2 = `${this.getWOQLPrelude(functName, fluent, indent)}(`;
        const source = json.source.file ? `"${json.source.file}"` : `"${json.source.url}"`;
        const format = json.format === "csv" ? "" : json.format;
        str2 += source;
        if (format)
          str2 += `, ${format}`;
        return str2;
      };
      WOQLPrinter.prototype.getArgumentOrder = function(operator, json) {
        const args = Object.keys(json);
        args.splice(args.indexOf("@type"), 1);
        return args;
      };
      WOQLPrinter.prototype.argumentTakesNewline = function(operator) {
        return this.isQueryListOperator(operator);
      };
      WOQLPrinter.prototype.argumentRequiresArray = function(predicate, entries) {
        if ((predicate === "group_by" || predicate === "list") && entries.length > 1)
          return true;
        return false;
      };
      WOQLPrinter.prototype.printArgument = function(operator, predicate, arg, level, fluent) {
        let str2 = "";
        if (fluent)
          str2 += ")";
        const newline = this.argumentTakesNewline(operator);
        if (newline)
          str2 += `
${nspaces((level + 1) * this.indent_spaces)}`;
        if (arg["@type"] === "True")
          return "true";
        if (predicate === "document")
          return this.decompileDocument(arg);
        if (predicate === "variables")
          return this.decompileVariables(arg);
        if (predicate === "group_by" || predicate === "template")
          return this.decompileVariables(arg, true);
        if (predicate === "columns")
          return this.decompileAsVars(arg, level + 1);
        if (predicate === "pattern")
          return this.decompileRegexPattern(arg, level + 1);
        if (Array.isArray(arg)) {
          const arr_entries = [];
          for (let j = 0; j < arg.length; j++) {
            const nlevel = newline ? level + 1 : level;
            arr_entries.push(this.printJSON(arg[j], nlevel, fluent, newline));
          }
          const jstr = newline ? `,
${nspaces(++level * this.indent_spaces)}` : ",";
          if (this.argumentRequiresArray(predicate, arr_entries)) {
            str2 += `[${arr_entries.join(jstr)}]`;
          } else
            str2 += arr_entries.join(jstr);
        } else if (typeof arg === "object") {
          const reet = this.printJSON(arg, level, fluent);
          str2 += reet;
        } else if (typeof arg === "string") {
          str2 += this.uncleanArgument(arg, operator, predicate);
        } else if (typeof arg === "number")
          return arg;
        return str2;
      };
      WOQLPrinter.prototype.decompileDocument = function(args) {
        const jsonDoc = {};
        this.decompileDictionary(jsonDoc, args);
        return `WOQL.doc(${JSON.stringify(jsonDoc)})`;
      };
      WOQLPrinter.prototype.decompileDictionary = function(jsonDoc, args) {
        if (args.dictionary && args.dictionary.data && Array.isArray(args.dictionary.data)) {
          args.dictionary.data.forEach((item) => {
            this.decompileFieldValuePair(jsonDoc, item);
          });
        }
      };
      WOQLPrinter.prototype.decompileFieldValuePair = function(jsonDoc, fieldValue) {
        const type = fieldValue.field || "";
        let value = false;
        if (fieldValue.value && fieldValue.value) {
          if (fieldValue.value.data) {
            value = fieldValue.value.data["@value"];
          } else if (fieldValue.value.dictionary && Array.isArray(fieldValue.value.dictionary.data)) {
            const valueDoc = {};
            this.decompileDictionary(valueDoc, fieldValue.value);
            value = valueDoc;
          }
        }
        jsonDoc[type] = value;
      };
      WOQLPrinter.prototype.decompileVariables = function(args, checkIsArray = false) {
        if (Array.isArray(args)) {
          let str2 = "";
          args.forEach((varName, index) => {
            str2 += `"v:${varName}"`;
            if (index < args.length - 1)
              str2 += ", ";
          });
          if (checkIsArray && args.length > 1)
            str2 = `[${str2}]`;
          return str2;
        }
        return "";
      };
      WOQLPrinter.prototype.decompileRegexPattern = function(json) {
        if (typeof json === "object" && json["@type"] === "DataValue") {
          return this.pvar(json);
        }
        if (json["@type"].startsWith("Path")) {
          return `"${this.decompilePathPattern(json)}"`;
        }
        const str2 = json;
        return `"${str2.replace("\\", "\\\\")}"`;
      };
      WOQLPrinter.prototype.pvar = function(json) {
        if (json.variable) {
          let varname = json.variable;
          const order = json.order ? json.order : "";
          if (varname.indexOf(":") === -1) {
            varname = `v:${varname}`;
          }
          return order !== "" && order !== "asc" ? `["${varname}","${order}"]` : `"${varname}"`;
        }
        if (json.node) {
          return `"${json.node}"`;
        }
        if (json.data) {
          return JSON.stringify(json.data);
        }
        if (json.list) {
          const listArr = json.list;
          if (Array.isArray(listArr)) {
            const listTmp = [];
            listArr.forEach((listItem, index) => {
              listTmp.push(this.pvar(listItem));
            });
            return `[${listTmp.join(", ")}]`;
          }
          return this.pvar(json.list);
        }
        return false;
      };
      WOQLPrinter.prototype.getWOQLPrelude = function(operator, fluent, inline) {
        if (operator === "true" || operator === "false") {
          if (this.language === "python") {
            return operator.charAt(0).toUpperCase() + operator.slice(1);
          }
          return operator;
        }
        let prelude2 = "WOQL.";
        if (this.language === "python") {
          this.pythonic[operator] && (operator = this.pythonic[operator]);
          prelude2 = "WOQLQuery().";
        }
        if (fluent) {
          return `.${operator}`;
        }
        return (inline ? `
${nspaces(inline)}` : "") + prelude2 + operator;
      };
      WOQLPrinter.prototype.uncleanArgument = function(arg, operator, predicate) {
        if (arg.indexOf(":") !== -1) {
          for (const s in this.vocab) {
            if (this.vocab[s] === arg)
              return `"${s}"`;
          }
        }
        return `"${arg}"`;
      };
      WOQLPrinter.prototype.isListOperator = function(operator) {
        return this.list_operators.indexOf(operator) !== -1;
      };
      WOQLPrinter.prototype.isQueryListOperator = function(operator) {
        return this.query_list_operators.indexOf(operator) !== -1;
      };
      WOQLPrinter.prototype.getFunctionForOperator = function(operator, json) {
        if (this.operator_maps[operator])
          return this.operator_maps[operator];
        if (operator === "Triple" && json.graph)
          return "quad";
        const f = camelToSnake(operator);
        if (this.shortcuts[f])
          return this.shortcuts[f];
        return f;
      };
      WOQLPrinter.prototype.getBoxedPredicate = function(operator, json) {
        for (let i = 0; i < this.boxed_predicates.length; i++) {
          if (json[this.boxed_predicates[i]]) {
            return this.boxed_predicates[i];
          }
        }
        if (operator === "QueryListElement") {
          return "woql:query";
        }
        return false;
      };
      WOQLPrinter.prototype.unboxJSON = function(operator, json) {
        const bp = this.getBoxedPredicate(operator, json);
        if (bp) {
          return json[bp];
        }
        return false;
      };
      WOQLPrinter.prototype.decompileAsVars = function(asvs, level) {
        let str2 = "";
        if (!Array.isArray(asvs))
          return "";
        asvs.forEach((wasv, i) => {
          str2 += `
${nspaces(level * this.indent_spaces)}${i === 0 ? "WOQL" : ""}`;
          if (wasv["@type"] === "Column" && wasv.indicator) {
            const source = wasv.indicator.name || wasv.indicator.index;
            const target = `v:${wasv.variable}`;
            const { type } = wasv.indicator;
            str2 += `.as("${source}", "${target}"`;
            if (type)
              str2 += `, "${type}")`;
            else
              str2 += ")";
          }
        });
        return str2;
      };
      WOQLPrinter.prototype.decompilePathPattern = function(pstruct) {
        const t = pstruct["@type"];
        switch (t) {
          case "InversePathPredicate":
            return pstruct.predicate ? `<${pstruct.predicate}` : "<.";
          case "PathPredicate":
            return pstruct.predicate ? `${pstruct.predicate}` : ".";
          case "PathPlus":
            var next = pstruct.plus;
            if (Array.isArray(next))
              next = next[0];
            if (needsParentheses(next))
              return `(${this.decompilePathPattern(next)})+`;
            return `${this.decompilePathPattern(next)}+`;
          case "PathStar":
            var next = pstruct.star;
            if (Array.isArray(next))
              next = next[0];
            if (needsParentheses(next))
              return `(${this.decompilePathPattern(next)})*`;
            return `${this.decompilePathPattern(next)}*`;
          case "PathTimes":
            var next = pstruct.times;
            var astr = ` {${pstruct.from},${pstruct.to}}`;
            if (Array.isArray(next))
              next = next[0];
            if (needsParentheses(next))
              return `(${this.decompilePathPattern(next)})${astr}`;
            return this.decompilePathPattern(next) + astr;
          case "PathSequence":
            const sequenceArr = pstruct.sequence;
            if (Array.isArray(sequenceArr) && sequenceArr.length === 2) {
              let next1 = sequenceArr[0];
              const next2 = sequenceArr[1];
              if (Array.isArray(next1))
                next1 = next1[0];
              var seqstr = "";
              if (needsParentheses(next1))
                seqstr += "(";
              seqstr += this.decompilePathPattern(next1);
              if (needsParentheses(next1))
                seqstr += ")";
              seqstr += ",";
              if (needsParentheses(next2))
                seqstr += "(";
              seqstr += this.decompilePathPattern(next2);
              if (next1["@type"] === "InversePathPredicate") {
                seqstr += ">";
              }
              if (needsParentheses(next2))
                seqstr += ")";
              return seqstr;
            }
          case "PathOr":
            const orArr = pstruct.or;
            if (Array.isArray(orArr) && orArr.length === 2) {
              let next1 = orArr[0];
              const next2 = orArr[1];
              if (Array.isArray(next1))
                next1 = next1[0];
              var seqstr = "";
              if (needsParentheses(next1))
                seqstr += "(";
              seqstr += this.decompilePathPattern(next1);
              if (needsParentheses(next1))
                seqstr += ")";
              seqstr += "|";
              if (needsParentheses(next2))
                seqstr += "(";
              seqstr += this.decompilePathPattern(next2);
              if (needsParentheses(next2))
                seqstr += ")";
              return seqstr;
            }
        }
        return "error";
      };
      function needsParentheses(obj) {
        const noparens = ["PathPredicate", "PathPlus", "PathTimes", "InversePathPredicate"];
        if (noparens.indexOf(obj["@type"]) !== -1)
          return false;
        return true;
      }
      function camelToSnake(string) {
        return string.replace(/[\w]([A-Z])/g, (m) => `${m[0]}_${m[1]}`).toLowerCase();
      }
      function nspaces(n) {
        let spaces = "";
        for (let i = 0; i < n; i++) {
          spaces += " ";
        }
        return spaces;
      }
      module2.exports = WOQLPrinter;
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlDoc.js
  var require_woqlDoc = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlDoc.js"(exports2, module2) {
      function convert(obj) {
        if (obj == null) {
          return null;
        }
        if (typeof obj === "number") {
          return {
            "@type": "Value",
            data: {
              "@type": "xsd:decimal",
              "@value": obj
            }
          };
        }
        if (typeof obj === "boolean") {
          return {
            "@type": "Value",
            data: {
              "@type": "xsd:boolean",
              "@value": obj
            }
          };
        }
        if (typeof obj === "string") {
          if (obj.indexOf("v:") === -1) {
            return {
              "@type": "Value",
              data: {
                "@type": "xsd:string",
                "@value": obj
              }
            };
          }
          return {
            "@type": "Value",
            variable: obj.split(":")[1]
          };
        }
        if (obj instanceof Var2) {
          return {
            "@type": "Value",
            variable: obj.name
          };
        }
        if (typeof obj === "object" && !Array.isArray(obj)) {
          const pairs = [];
          for (const [key, value] of Object.entries(obj)) {
            pairs.push({
              "@type": "FieldValuePair",
              field: key,
              value: convert(value)
            });
          }
          return {
            "@type": "Value",
            dictionary: {
              "@type": "DictionaryTemplate",
              data: pairs
            }
          };
        }
        if (typeof obj === "object" && Array.isArray(obj)) {
          const list = obj.map(convert);
          return {
            "@type": "Value",
            list
          };
        }
      }
      function Var2(name) {
        this.name = name;
        this.json = function() {
          return {
            "@type": "Value",
            variable: this.name
          };
        };
      }
      var uniqueVarCounter = 0;
      function VarUnique2(name) {
        uniqueVarCounter += 1;
        const localName = `${name}_${uniqueVarCounter}`;
        this.name = localName;
        this.json = function() {
          return {
            "@type": "Value",
            variable: this.name
          };
        };
      }
      VarUnique2.prototype = Object.create(Var2.prototype);
      function SetVarsUniqueCounter2(start) {
        uniqueVarCounter = start;
      }
      function Doc2(obj) {
        this.doc = obj;
        this.encoded = convert(obj);
        return this.encoded;
      }
      function Vars2(...args) {
        const varObj = {};
        for (let i = 0, j = args.length; i < j; i += 1) {
          const argumentName = args[i];
          varObj[argumentName] = new Var2(argumentName);
        }
        return varObj;
      }
      function VarsUnique2(...args) {
        const varObj = {};
        for (let i = 0, j = args.length; i < j; i += 1) {
          const argumentName = args[i];
          uniqueVarCounter += 1;
          varObj[argumentName] = new Var2(argumentName + (uniqueVarCounter ? `_${uniqueVarCounter}` : ""));
        }
        return varObj;
      }
      module2.exports = {
        Vars: Vars2,
        VarsUnique: VarsUnique2,
        Var: Var2,
        VarUnique: VarUnique2,
        Doc: Doc2,
        SetVarsUniqueCounter: SetVarsUniqueCounter2
      };
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/typedef.js
  var require_typedef = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/typedef.js"(exports2, module2) {
      var Utils = require_utils();
      var { ACTIONS } = Utils.ACTIONS;
      module2.exports = {};
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlCore.js
  var require_woqlCore = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlCore.js"(exports2, module2) {
      var UTILS = require_utils();
      var WOQLPrinter = require_woqlPrinter();
      var { Var: Var2, Vars: Vars2, Doc: Doc2 } = require_woqlDoc();
      var typedef = require_typedef();
      var WOQLQuery2 = class {
        triple_builder_context = {};
        query = null;
        counter = 1;
        errors = [];
        cursor = {};
        chain_ended = false;
        contains_update = false;
        // operators which preserve global paging
        paging_transitive_properties = ["select", "from", "start", "when", "opt", "limit"];
        update_operators = [
          "AddTriple",
          "DeleteTriple",
          "AddQuad",
          "DeleteQuad",
          "InsertDocument",
          "DeleteDocument",
          "UpdateDocument"
        ];
        vocab = this.loadDefaultVocabulary();
        // object used to accumulate triples from fragments to support usage like node("x").label("y");
        tripleBuilder = false;
        /**
         * defines the internal functions of the woql query object - the
         * language API is defined in WOQLQuery
         * @module WOQLQuery
         * @constructor
         * @param {object} [query] json-ld query for initialisation
         * @returns {WOQLQuery}
         */
        constructor(query) {
          this.query = query || {};
          this.cursor = this.query;
          return this;
        }
      };
      WOQLQuery2.prototype.parameterError = function(msg) {
        this.errors.push({ type: this.cursor["@type"], message: msg });
        return this;
      };
      WOQLQuery2.prototype.hasErrors = function() {
        return this.errors.length > 0;
      };
      WOQLQuery2.prototype.addSubQuery = function(Subq) {
        if (Subq) {
          this.cursor.query = this.jobj(Subq);
        } else {
          const nv = {};
          this.cursor.query = nv;
          this.cursor = nv;
        }
        return this;
      };
      WOQLQuery2.prototype.containsUpdate = function(json) {
        json = json || this.query;
        if (this.update_operators.indexOf(json["@type"]) !== -1)
          return true;
        if (json.consequent && this.containsUpdate(json.consequent))
          return true;
        if (json.query)
          return this.containsUpdate(json.query);
        if (json.and) {
          for (var i = 0; i < json.and.length; i++) {
            if (this.containsUpdate(json.and[i]))
              return true;
          }
        }
        if (json.or) {
          for (var i = 0; i < json.or.length; i++) {
            if (this.containsUpdate(json.or[i]))
              return true;
          }
        }
        return false;
      };
      WOQLQuery2.prototype.updated = function() {
        this.contains_update = true;
        return this;
      };
      WOQLQuery2.prototype.jlt = function(val, type) {
        if (!type)
          type = "xsd:string";
        else
          type = type.indexOf(":") === -1 ? `xsd:${type}` : type;
        return { "@type": type, "@value": val };
      };
      WOQLQuery2.prototype.varj = function(varb) {
        if (varb instanceof Var2)
          varb = varb.name;
        if (varb.substring(0, 2) === "v:")
          varb = varb.substring(2);
        if (typeof varb === "string") {
          return {
            "@type": "Value",
            variable: varb
          };
        }
        return varb;
      };
      WOQLQuery2.prototype.rawVar = function(varb) {
        if (varb instanceof Var2)
          return varb.name;
        if (varb.substring(0, 2) === "v:")
          varb = varb.substring(2);
        return varb;
      };
      WOQLQuery2.prototype.rawVarList = function(vl) {
        const ret = [];
        for (let i = 0; i < vl.length; i++) {
          const co = this.rawVar(vl[i]);
          ret.push(co);
        }
        return ret;
      };
      WOQLQuery2.prototype.jobj = function(qobj) {
        if (qobj.json) {
          return qobj.json();
        }
        if (qobj === true)
          return { "@type": "True" };
        return qobj;
      };
      WOQLQuery2.prototype.asv = function(colname_or_index, variable, type) {
        const asvar = {};
        if (typeof colname_or_index === "number") {
          asvar["@type"] = "Column";
          asvar.indicator = { "@type": "Indicator", index: colname_or_index };
        } else if (typeof colname_or_index === "string") {
          asvar["@type"] = "Column";
          asvar.indicator = { "@type": "Indicator", name: colname_or_index };
        }
        if (variable instanceof Var2) {
          asvar.variable = variable.name;
        } else if (variable.substring(0, 2) === "v:") {
          asvar.variable = variable.substring(2);
        } else {
          asvar.variable = variable;
        }
        if (type)
          asvar.type = type;
        return asvar;
      };
      WOQLQuery2.prototype.wform = function(opts2) {
        if (opts2 && opts2.type) {
          this.cursor.format = {
            "@type": "Format",
            format_type: { "@value": opts2.type, "@type": "xsd:string" }
          };
          if (typeof opts2.format_header !== "undefined") {
            const h = !!opts2.format_header;
            this.cursor.format.format_header = {
              "@value": h,
              "@type": "xsd:boolean"
            };
          }
        }
        return this;
      };
      WOQLQuery2.prototype.arop = function(arg) {
        if (typeof arg === "object") {
          return this.jobj(this.cleanArithmeticValue(arg));
        }
        return this.cleanArithmeticValue(arg, "xsd:decimal");
      };
      WOQLQuery2.prototype.dataList = function(wvar, string_only) {
        if (typeof wvar === "string")
          return this.expandDataVariable(wvar, true);
        if (Array.isArray(wvar)) {
          const ret = [];
          for (let i = 0; i < wvar.length; i++) {
            const co = this.cleanDataValue(wvar[i]);
            ret.push(co);
          }
          return ret;
        }
      };
      WOQLQuery2.prototype.valueList = function(wvar, string_only) {
        if (typeof wvar === "string")
          return this.expandValueVariable(wvar, true);
        if (Array.isArray(wvar)) {
          const ret = [];
          for (let i = 0; i < wvar.length; i++) {
            let co = this.cleanObject(wvar[i]);
            if (typeof co === "string")
              co = { node: co };
            ret.push(co);
          }
          return ret;
        }
      };
      WOQLQuery2.prototype.vlist = function(list) {
        const vl = [];
        for (let i = 0; i < list.length; i++) {
          const v = this.expandValueVariable(list[i]);
          vl.push(v.variable);
        }
        return vl;
      };
      WOQLQuery2.prototype.dataValueList = function(list) {
        const dvl = [];
        for (let i = 0; i < list.length; i++) {
          const o = this.cleanDataValue(list[i]);
          dvl.push(o);
        }
        return dvl;
      };
      WOQLQuery2.prototype.cleanSubject = function(s) {
        let subj = false;
        if (s instanceof Var2) {
          return this.expandNodeVariable(s);
        }
        if (typeof s === "object") {
          return s;
        }
        if (typeof s === "string") {
          if (s.indexOf("v:") !== -1)
            subj = s;
          else
            subj = s;
          return this.expandNodeVariable(subj);
        }
        this.parameterError("Subject must be a URI string");
        return `${s}`;
      };
      WOQLQuery2.prototype.cleanPredicate = function(p) {
        let pred = false;
        if (p instanceof Var2)
          return this.expandNodeVariable(p);
        if (typeof p === "object")
          return p;
        if (typeof p !== "string") {
          this.parameterError("Predicate must be a URI string");
          return `${p}`;
        }
        if (p.indexOf(":") !== -1)
          pred = p;
        else if (this.wellKnownPredicate(p))
          pred = p;
        else
          pred = p;
        return this.expandNodeVariable(pred);
      };
      WOQLQuery2.prototype.wellKnownPredicate = function(p, noxsd) {
        if (this.vocab && this.vocab[p]) {
          const full = this.vocab[p];
          const start = full.substring(0, 3);
          if (full === "system:abstract" || start === "xdd" || start === "xsd")
            return false;
          return true;
        }
        return false;
      };
      WOQLQuery2.prototype.cleanPathPredicate = function(p) {
        let pred = false;
        if (p.indexOf(":") !== -1)
          pred = p;
        else if (this.wellKnownPredicate(p))
          pred = this.vocab[p];
        else
          pred = p;
        return pred;
      };
      WOQLQuery2.prototype.cleanObject = function(o, t) {
        const obj = { "@type": "Value" };
        if (o instanceof Var2) {
          return this.expandValueVariable(o);
        }
        if (o instanceof Doc2) {
          return o.encoded;
        }
        if (typeof o === "string") {
          if (o.indexOf("v:") !== -1) {
            return this.expandValueVariable(o);
          }
          obj.node = o;
        } else if (typeof o === "number") {
          t = t || "xsd:decimal";
          obj.data = this.jlt(o, t);
        } else if (typeof o === "boolean") {
          t = t || "xsd:boolean";
          obj.data = this.jlt(o, t);
        } else if (typeof o === "object" && o) {
          if (typeof o["@value"] !== "undefined")
            obj.data = o;
          else
            return o;
        } else if (typeof o === "boolean") {
          t = t || "xsd:boolean";
          obj["woql:datatype"] = this.jlt(o, t);
        }
        return obj;
      };
      WOQLQuery2.prototype.cleanDataValue = function(o, t) {
        const obj = { "@type": "DataValue" };
        if (o instanceof Var2) {
          return this.expandDataVariable(o);
        }
        if (o instanceof Doc2) {
          return o.encoded;
        }
        if (typeof o === "string") {
          if (o.indexOf("v:") !== -1) {
            return this.expandDataVariable(o);
          }
          obj.data = this.jlt(o, t);
        } else if (typeof o === "number") {
          t = t || "xsd:decimal";
          obj.data = this.jlt(o, t);
        } else if (typeof o === "boolean") {
          t = t || "xsd:boolean";
          obj.data = this.jlt(o, t);
        } else if (Array.isArray(o)) {
          const res = [];
          for (let i = 0; i < o.length; i++) {
            res.push(this.cleanDataValue(o[i]));
          }
          obj.list = res;
        } else if (typeof o === "object" && o) {
          if (o["@value"])
            obj.data = o;
          else
            return o;
        }
        return obj;
      };
      WOQLQuery2.prototype.cleanArithmeticValue = function(o, t) {
        const obj = { "@type": "ArithmeticValue" };
        if (o instanceof Var2) {
          return this.expandArithmeticVariable(o);
        }
        if (typeof o === "string") {
          if (o.indexOf("v:") !== -1) {
            return this.expandArithmeticVariable(o);
          }
          obj.data = this.jlt(o, t);
        } else if (typeof o === "number") {
          t = t || "xsd:decimal";
          obj.data = this.jlt(o, t);
        } else if (typeof o === "object" && o) {
          if (o["@value"])
            obj.data = o;
          else
            return o;
        }
        return obj;
      };
      WOQLQuery2.prototype.cleanNodeValue = function(o, t) {
        const obj = { "@type": "NodeValue" };
        if (o instanceof Var2) {
          return this.expandNodeVariable(o);
        }
        if (typeof o === "string") {
          if (o.indexOf("v:") !== -1) {
            return this.expandNodeVariable(o);
          }
          obj.node = o;
        } else if (typeof o === "object" && o) {
          return o;
        }
        return obj;
      };
      WOQLQuery2.prototype.cleanGraph = function(g) {
        return g;
      };
      WOQLQuery2.prototype.expandVariable = function(varname, type, always) {
        if (varname instanceof Var2) {
          return {
            "@type": type,
            variable: varname.name
          };
        }
        if (varname.substring(0, 2) === "v:" || always) {
          if (varname.substring(0, 2) === "v:")
            varname = varname.substring(2);
          return {
            "@type": type,
            variable: varname
          };
        }
        return {
          "@type": type,
          node: varname
        };
      };
      WOQLQuery2.prototype.expandValueVariable = function(varname, always) {
        return this.expandVariable(varname, "Value", always);
      };
      WOQLQuery2.prototype.expandNodeVariable = function(varname, always) {
        return this.expandVariable(varname, "NodeValue", always);
      };
      WOQLQuery2.prototype.expandDataVariable = function(varname, always) {
        return this.expandVariable(varname, "DataValue", always);
      };
      WOQLQuery2.prototype.expandArithmeticVariable = function(varname, always) {
        return this.expandVariable(varname, "ArithmeticValue", always);
      };
      WOQLQuery2.prototype.defaultContext = function(DB_IRI) {
        const def = {};
        for (const pref2 in UTILS.standard_urls) {
          def[pref2] = UTILS.standard_urls[pref2];
        }
        def.scm = `${DB_IRI}/schema#`;
        def.doc = `${DB_IRI}/data/`;
        return def;
      };
      WOQLQuery2.prototype.getContext = function(q) {
        q = q || this.query;
        for (const prop of Object.keys(q)) {
          if (prop === "@context")
            return q[prop];
          if (this.paging_transitive_properties.indexOf(prop) !== -1) {
            const nq = q[prop][1];
            const nc = this.getContext(nq);
            if (nc)
              return nc;
          }
        }
      };
      WOQLQuery2.prototype.context = function(c) {
        this.query["@context"] = c;
      };
      WOQLQuery2.prototype.loadDefaultVocabulary = function() {
        const vocab = {};
        vocab.Class = "owl:Class";
        vocab.DatatypeProperty = "owl:DatatypeProperty";
        vocab.ObjectProperty = "owl:ObjectProperty";
        vocab.Document = "system:Document";
        vocab.abstract = "system:abstract";
        vocab.comment = "rdfs:comment";
        vocab.range = "rdfs:range";
        vocab.domain = "rdfs:domain";
        vocab.subClassOf = "rdfs:subClassOf";
        vocab.string = "xsd:string";
        vocab.integer = "xsd:integer";
        vocab.decimal = "xsd:decimal";
        vocab.boolean = "xdd:boolean";
        vocab.email = "xdd:email";
        vocab.json = "xdd:json";
        vocab.dateTime = "xsd:dateTime";
        vocab.date = "xsd:date";
        vocab.coordinate = "xdd:coordinate";
        vocab.line = "xdd:coordinatePolyline";
        vocab.polygon = "xdd:coordinatePolygon";
        return vocab;
      };
      WOQLQuery2.prototype.setVocabulary = function(vocab) {
        this.vocab = vocab;
      };
      WOQLQuery2.prototype.getVocabulary = function(vocab) {
        return this.vocab;
      };
      WOQLQuery2.prototype.execute = function(client, commit_msg) {
        return client.query(this, commit_msg);
      };
      WOQLQuery2.prototype.json = function(json) {
        if (json) {
          this.query = copyJSON(json);
          return this;
        }
        return copyJSON(this.query, true);
      };
      WOQLQuery2.prototype.prettyPrint = function(clang = "js") {
        const printer = new WOQLPrinter(this.vocab, clang);
        return printer.printJSON(this.query);
      };
      WOQLQuery2.prototype.findLastSubject = function(json) {
        if (json && json.and) {
          for (var i = json.and.length - 1; i >= 0; i--) {
            const lqs = this.findLastSubject(json.and[i]);
            if (lqs)
              return lqs;
          }
        }
        if (json && json.or) {
          for (var i = json.or.length - 1; i >= 0; i--) {
            const lqs = this.findLastSubject(json.or[i]);
            if (lqs)
              return lqs;
          }
        }
        if (json && json.query) {
          const ls = this.findLastSubject(json.query);
          if (ls)
            return ls;
        }
        if (json && json.subject) {
          return json;
        }
        return false;
      };
      WOQLQuery2.prototype.findLastProperty = function(json) {
        if (json && json.and) {
          for (var i = json.and.length - 1; i >= 0; i--) {
            const lqs = this.findLastProperty(json.and[i]);
            if (lqs)
              return lqs;
          }
        }
        if (json && json.or) {
          for (var i = json.or.length - 1; i >= 0; i--) {
            const lqs = this.findLastProperty(json.or[i]);
            if (lqs)
              return lqs;
          }
        }
        if (json && json.query) {
          const ls = this.findLastProperty(json.query);
          if (ls)
            return ls;
        }
        if (json && json.subject && this._is_property_triple(json.predicate, json.object)) {
          return json;
        }
        return false;
      };
      WOQLQuery2.prototype._is_property_triple = function(pred, obj) {
        const pred_str = pred.node ? pred.node : pred;
        const obj_str = obj.node ? obj.node : obj;
        if (obj_str === "owl:ObjectProperty" || obj_str === "owl:DatatypeProperty")
          return true;
        if (pred_str === "rdfs:domain" || pred_str === "rdfs:range")
          return true;
        return false;
      };
      WOQLQuery2.prototype.compilePathPattern = function(pat) {
        const toks = tokenize(pat);
        if (toks && toks.length)
          return tokensToJSON(toks, this);
        this.parameterError(`Pattern error - could not be parsed ${pat}`);
      };
      function tokenize(pat) {
        let parts = getClauseAndRemainder(pat);
        const seq = [];
        while (parts.length === 2) {
          seq.push(parts[0]);
          parts = getClauseAndRemainder(parts[1]);
        }
        seq.push(parts[0]);
        return seq;
      }
      function getClauseAndRemainder(pat) {
        pat = pat.trim();
        let open = 1;
        if (pat.charAt(0) === "(") {
          for (var i = 1; i < pat.length; i++) {
            if (pat.charAt(i) === "(")
              open++;
            else if (pat.charAt(i) === ")")
              open--;
            if (open === 0) {
              const rem = pat.substring(i + 1).trim();
              if (rem)
                return [pat.substring(1, i), rem];
              return getClauseAndRemainder(pat.substring(1, i));
            }
          }
          return [];
        }
        if (pat[0] === "+" || pat[0] === "," || pat[0] === "|" || pat[0] === "*") {
          const ret = [pat[0]];
          if (pat.substring(1))
            ret.push(pat.substring(1));
          return ret;
        }
        if (pat.charAt(0) === "{") {
          const ret = [pat.substring(0, pat.indexOf("}") + 1)];
          if (pat.substring(pat.indexOf("}") + 1))
            ret.push(pat.substring(pat.indexOf("}") + 1));
          return ret;
        }
        for (var i = 1; i < pat.length; i++) {
          if (pat[i] === "," || pat[i] === "|" || pat[i] === "+" || pat[i] === "{" || pat[i] === "*")
            return [pat.substring(0, i), pat.substring(i)];
        }
        return [pat];
      }
      function compilePredicate(pp, q) {
        if (pp.indexOf("<") !== -1 && pp.indexOf(">") !== -1) {
          const pred = pp.slice(1, pp.length - 1);
          const cleaned2 = pred === "." ? null : q.cleanPathPredicate(pred);
          return {
            "@type": "PathOr",
            or: [
              {
                "@type": "InversePathPredicate",
                predicate: cleaned2
              },
              {
                "@type": "PathPredicate",
                predicate: cleaned2
              }
            ]
          };
        }
        if (pp.indexOf("<") !== -1) {
          const pred = pp.slice(1, pp.length);
          const cleaned2 = pred === "." ? null : q.cleanPathPredicate(pred);
          return { "@type": "InversePathPredicate", predicate: cleaned2 };
        }
        if (pp.indexOf(">") !== -1) {
          const pred = pp.slice(0, pp.length - 1);
          const cleaned2 = pred === "." ? null : q.cleanPathPredicate(pred);
          return { "@type": "PathPredicate", predicate: cleaned2 };
        }
        const cleaned = pp === "." ? null : q.cleanPathPredicate(pp);
        return { "@type": "PathPredicate", predicate: cleaned };
      }
      function tokensToJSON(seq, q) {
        if (seq.length === 1) {
          const ntoks = tokenize(seq[0]);
          if (ntoks.length === 1) {
            const tok = ntoks[0].trim();
            return compilePredicate(tok, q);
          }
          return tokensToJSON(ntoks, q);
        }
        if (seq.indexOf("|") !== -1) {
          const left = seq.slice(0, seq.indexOf("|"));
          const right = seq.slice(seq.indexOf("|") + 1);
          return {
            "@type": "PathOr",
            or: [tokensToJSON(left, q), tokensToJSON(right, q)]
          };
        }
        if (seq.indexOf(",") !== -1) {
          const first = seq.slice(0, seq.indexOf(","));
          const second = seq.slice(seq.indexOf(",") + 1);
          return {
            "@type": "PathSequence",
            sequence: [tokensToJSON(first, q), tokensToJSON(second, q)]
          };
        }
        if (seq[1] === "+") {
          return {
            "@type": "PathPlus",
            plus: tokensToJSON([seq[0]], q)
          };
        }
        if (seq[1] === "*") {
          return {
            "@type": "PathStar",
            star: tokensToJSON([seq[0]], q)
          };
        }
        if (seq[1].charAt(0) === "{") {
          const meat = seq[1].substring(1, seq[1].length - 1).split(",");
          return {
            "@type": "PathTimes",
            from: meat[0],
            to: meat[1],
            times: tokensToJSON([seq[0]], q)
          };
        }
        q.parameterError(`Pattern error - could not be parsed ${seq[0]}`);
        return {
          "@type": "PathPredicate",
          "rdfs:label": `failed to parse query ${seq[0]}`
        };
      }
      function copyJSON(orig, rollup) {
        if (Array.isArray(orig))
          return orig;
        if (rollup) {
          if (orig["@type"] === "And") {
            if (!orig.and || !orig.and.length)
              return {};
            if (orig.and.length === 1)
              return copyJSON(orig.and[0], rollup);
          } else if (orig["@type"] === "Or") {
            if (!orig.or || !orig.or.length)
              return {};
            if (orig.or.length === 1)
              return copyJSON(orig.or[0], rollup);
          }
          if (typeof orig.query !== "undefined" && orig["@type"] !== "Comment") {
            if (!orig.query["@type"])
              return {};
          } else if (orig["@type"] === "Comment" && orig.comment) {
            if (!orig.query || !orig.query["@type"])
              return { "@type": "Comment", comment: orig.comment };
          }
          if (typeof orig.consequent !== "undefined") {
            if (!orig.consequent["@type"])
              return {};
          }
        }
        const nuj = {};
        for (const k in orig) {
          const part = orig[k];
          if (Array.isArray(part)) {
            const nupart = [];
            for (let j = 0; j < part.length; j++) {
              if (typeof part[j] === "object") {
                const sub = copyJSON(part[j], rollup);
                if (!sub || !UTILS.empty(sub))
                  nupart.push(sub);
              } else {
                nupart.push(part[j]);
              }
            }
            nuj[k] = nupart;
          } else if (part === null) {
          } else if (typeof part === "object") {
            const q = copyJSON(part, rollup);
            if (!q || !UTILS.empty(q))
              nuj[k] = q;
          } else {
            nuj[k] = part;
          }
        }
        return nuj;
      }
      module2.exports = WOQLQuery2;
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlQuery.js
  var require_woqlQuery = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlQuery.js"(exports2, module2) {
      var WOQLCore = require_woqlCore();
      var { Var: Var2, Vars: Vars2, Doc: Doc2 } = require_woqlDoc();
      var typedef = require_typedef();
      var WOQLQuery2 = class extends WOQLCore {
        /**
         * defines the internal functions of the woql query object - the
         * language API is defined in WOQLQuery
         * @module WOQLQuery
         * @constructor
         * @param {object} [query] json-ld query for initialisation
         * @returns {WOQLQuery}
         */
        /**
        * Update a pattern matching rule for the triple (Subject, Predicate, oldObjValue) with the
        * new one (Subject, Predicate, newObjValue)
        * @param {string|Var} subject - The IRI of a triple’s subject or a variable
        * @param {string|Var} predicate - The IRI of a property or a variable
        * @param {string|Var} newObjValue - The value to update or a literal
        * @param {string|Var} oldObjValue - The old value of the object
        * @returns {WOQLQuery} A WOQLQuery which contains the a Update Triple Statement
        */
        update_triple(subject, predicate, newObjValue, oldObjValue) {
          return this;
        }
        /**
        * Generates a query that by default matches all triples in a graph identified by "graph"
        * or in all the current terminusDB's graph
        * @param {string | boolean} [graph] - false or the resource identifier of a graph possible
        * value are schema/{main - myschema - *} | instance/{main - myschema - *}  |
        * inference/{main - myschema - *}
        * @param {string|Var} [subject] - The IRI of a triple’s subject or a variable,
        * default value "v:Subject"
        * @param {string|Var} [predicate] - The IRI of a property or a variable,
        *  default value "v:Predicate"
        * @param {string|Var} [object] - The IRI of a node or a variable, or a literal,
        * default value "v:Object"
        * @returns {WOQLQuery} A WOQLQuery which contains the pattern matching expression
        */
        star(graph, subject, predicate, object) {
          return this;
        }
        /**
        * Update a pattern matching rule for the quad [S, P, O, G] (Subject, Predicate, Object, Graph)
        * @param {string|Var} subject - The IRI of a triple’s subject or a variable
        * @param {string|Var} predicate - The IRI of a property or a variable
        * @param {string|Var} newObject - The value to update or a literal
        * @param {typedef.GraphRef} graphRef - A valid graph resource identifier string
        * @returns {WOQLQuery} A WOQLQuery which contains the a Update Quad Statement
        */
        update_quad(subject, predicate, newObject, graphRef) {
          return this;
        }
        /**
         * @param {string|Var} id - IRI string or variable containing
         * @param {string|Var} type  -  IRI string or variable containing the IRI of the
         * @param {typedef.GraphRef} [refGraph] - Optional Graph resource identifier
         * @returns {WOQLQuery} A WOQLQuery which contains the insert expression
         */
        insert(id, type, refGraph) {
          return this;
        }
        /**
        * Sets the graph resource ID that will be used for subsequent chained function calls
        * @param {typedef.GraphRef} [graphRef] Resource String identifying the graph which will
        * be used for subsequent chained schema calls
        * @returns {WOQLQuery} A WOQLQuery which contains the partial Graph pattern matching expression
        * @example
        */
        graph(graphRef) {
          return this;
        }
        /**
         * Specifies the identity of a node that can then be used in subsequent builder functions.
         * Note that node() requires subsequent chained functions to complete the triples / quads
         * that it produces - by itself it only generates the subject.
         * @param {string|Var} nodeid -  The IRI of a node or a variable containing an IRI which will
         * be the subject of the builder functions
         * @param {typedef.FuntionType} [chainType] - Optional type of builder function to build
         * (default is triple)
         * @returns {WOQLQuery} - A WOQLQuery which contains the partial Node pattern matching expression
         */
        node(nodeid, chainType) {
          return this;
        }
        /**
         * Deletes all triples in the passed graph (defaults to instance/main)
         * @param {typedef.GraphRef} [graphRef] - Resource String identifying the graph from
         * which all triples will be removed
         * @returns {WOQLQuery} - A WOQLQuery which contains the deletion expression
         * @example
         * nuke("schema/main")
         * //will delete everything from the schema/main graph
         */
        nuke(graphRef) {
          return this;
        }
        /**
         * @param {string|Var} [Subj] - The IRI of a triple’s subject or a variable
         * @param {string|Var} [Pred] - The IRI of a property or a variable
         * @param {string|Var} [Obj] - The IRI of a node or a variable, or a literal
         * @param {typedef.GraphRef} [Graph] - the resource identifier of a graph possible
         * @returns {WOQLQuery} - A WOQLQuery which contains the pattern matching expression
         */
        all(Subj, Pred, Obj, Graph) {
          return this;
        }
        /**
         * @param {boolean} tf
         * @returns {object}
         * @example
         */
        boolean(tf) {
          return {};
        }
        /**
         * @param {string} s
         * @returns {object}
         * @example
         */
        string(s) {
          return {};
        }
        /**
        * @param {any} s
        * @param {string} t
        * @returns {object}
        * @example
        */
        literal(s, t) {
          return {};
        }
        /**
        * @param {string} s
        * @returns {object}
        * @example
        */
        iri(s) {
          return {};
        }
        // eslint-disable-next-line no-underscore-dangle
        _set_context(ctxt) {
          return this;
        }
        /**
         * @param {WOQLQuery} Subq
         * @returns {WOQLQuery}
         */
        addSubQuery(Subq) {
          super.addSubQuery(Subq);
          return this;
        }
        /**
         * @param {string} msg
         * @returns {WOQLQuery}
         */
        parameterError(msg) {
          super.parameterError(msg);
          return this;
        }
        /**
         * @returns {WOQLQuery}
         */
        updated() {
          super.updated();
          return this;
        }
        // eslint-disable-next-line no-useless-constructor
        constructor(query) {
          super(query);
        }
      };
      WOQLQuery2.prototype.read_document = function(IRI, output) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "ReadDocument";
        this.cursor.identifier = this.cleanNodeValue(IRI);
        this.cursor.document = this.expandValueVariable(output);
        return this;
      };
      WOQLQuery2.prototype.insert_document = function(docjson, IRI) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "InsertDocument";
        if (typeof IRI !== "undefined")
          this.cursor.identifier = this.cleanNodeValue(IRI);
        this.cursor.document = this.cleanObject(docjson);
        return this.updated();
      };
      WOQLQuery2.prototype.update_document = function(docjson, IRI) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "UpdateDocument";
        if (typeof IRI !== "undefined")
          this.cursor.identifier = this.cleanNodeValue(IRI);
        this.cursor.document = this.cleanObject(docjson);
        return this.updated();
      };
      WOQLQuery2.prototype.delete_document = function(IRI) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "DeleteDocument";
        this.cursor.identifier = this.cleanNodeValue(IRI);
        return this.updated();
      };
      WOQLQuery2.prototype.wrapCursorWithAnd = function() {
        if (this.cursor && this.cursor["@type"] === "And") {
          const newby = this.cursor.and.length;
          this.and({});
          this.cursor = this.cursor.and[newby];
        } else {
          const nj = new WOQLQuery2().json(this.cursor);
          for (const k in this.cursor)
            delete this.cursor[k];
          this.and(nj, {});
          this.cursor = this.cursor.and[1];
        }
      };
      WOQLQuery2.prototype.using = function(refPath, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Using";
        if (!refPath || typeof refPath !== "string") {
          return this.parameterError("The first parameter to using must be a Collection ID (string)");
        }
        this.cursor.collection = refPath;
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.comment = function(comment, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Comment";
        this.cursor.comment = this.jlt(comment);
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.select = function(...varNames) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Select";
        if (!varNames || varNames.length <= 0) {
          return this.parameterError("Select must be given a list of variable names");
        }
        const last = varNames[varNames.length - 1];
        let embedquery = false;
        if (typeof last === "object" && !(last instanceof Var2) && last.json) {
          embedquery = varNames.pop();
        }
        this.cursor.variables = this.rawVarList(varNames);
        return this.addSubQuery(embedquery);
      };
      WOQLQuery2.prototype.distinct = function(...varNames) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Distinct";
        if (!varNames || varNames.length <= 0) {
          return this.parameterError("Distinct must be given a list of variable names");
        }
        const last = varNames[varNames.length - 1];
        let embedquery = false;
        if (typeof last === "object" && !(last instanceof Var2) && last.json) {
          embedquery = varNames.pop();
        }
        this.cursor.variables = this.rawVarList(varNames);
        return this.addSubQuery(embedquery);
      };
      WOQLQuery2.prototype.and = function(...subqueries) {
        if (this.cursor["@type"] && this.cursor["@type"] !== "And") {
          const nj = new WOQLQuery2().json(this.cursor);
          for (const k in this.cursor)
            delete this.cursor[k];
          subqueries.unshift(nj);
        }
        this.cursor["@type"] = "And";
        if (typeof this.cursor.and === "undefined")
          this.cursor.and = [];
        for (let i = 0; i < subqueries.length; i++) {
          const onevar = this.jobj(subqueries[i]);
          if (onevar["@type"] === "And" && onevar.and) {
            for (let j = 0; j < onevar.and.length; j++) {
              const qjson = onevar.and[j];
              if (qjson) {
                const subvar = this.jobj(qjson);
                this.cursor.and.push(subvar);
              }
            }
          } else {
            this.cursor.and.push(onevar);
          }
        }
        return this;
      };
      WOQLQuery2.prototype.or = function(...subqueries) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Or";
        if (typeof this.cursor.or === "undefined")
          this.cursor.or = [];
        for (let i = 0; i < subqueries.length; i++) {
          const onevar = this.jobj(subqueries[i]);
          this.cursor.or.push(onevar);
        }
        return this;
      };
      WOQLQuery2.prototype.from = function(graphRef, query) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "From";
        if (!graphRef || typeof graphRef !== "string") {
          return this.parameterError(
            "The first parameter to from must be a Graph Filter Expression (string)"
          );
        }
        this.cursor.graph = this.jlt(graphRef);
        return this.addSubQuery(query);
      };
      WOQLQuery2.prototype.into = function(graphRef, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Into";
        if (!graphRef || typeof graphRef !== "string") {
          return this.parameterError(
            "The first parameter to from must be a Graph Filter Expression (string)"
          );
        }
        this.cursor.graph = this.jlt(graphRef);
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.triple = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Triple";
        this.cursor.subject = this.cleanSubject(subject);
        this.cursor.predicate = this.cleanPredicate(predicate);
        this.cursor.object = this.cleanObject(object);
        return this;
      };
      WOQLQuery2.prototype.added_triple = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "AddedTriple";
        this.cursor.subject = this.cleanSubject(subject);
        this.cursor.predicate = this.cleanPredicate(predicate);
        this.cursor.object = this.cleanObject(object);
        return this;
      };
      WOQLQuery2.prototype.removed_triple = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "DeletedTriple";
        this.cursor.subject = this.cleanSubject(subject);
        this.cursor.predicate = this.cleanPredicate(predicate);
        this.cursor.object = this.cleanObject(object);
        return this;
      };
      WOQLQuery2.prototype.link = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Triple";
        this.cursor.subject = this.cleanSubject(subject);
        this.cursor.predicate = this.cleanPredicate(predicate);
        this.cursor.object = this.cleanSubject(object);
        return this;
      };
      WOQLQuery2.prototype.value = function(subject, predicate, objValue) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Triple";
        this.cursor.subject = this.cleanSubject(subject);
        this.cursor.predicate = this.cleanPredicate(predicate);
        this.cursor.object = this.cleanDataValue(objValue, "xsd:string");
        return this;
      };
      WOQLQuery2.prototype.quad = function(subject, predicate, object, graphRef) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        if (!graphRef)
          return this.parameterError("Quad takes four parameters, the last should be a graph filter");
        this.cursor["@type"] = "Triple";
        this.cursor.graph = this.cleanGraph(graphRef);
        return this;
      };
      WOQLQuery2.prototype.added_quad = function(subject, predicate, object, graphRef) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        if (!graphRef)
          return this.parameterError("Quad takes four parameters, the last should be a graph filter");
        this.cursor["@type"] = "AddedQuad";
        this.cursor.graph = this.cleanGraph(graphRef);
        return this;
      };
      WOQLQuery2.prototype.removed_quad = function(subject, predicate, object, graphRef) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        if (!graphRef)
          return this.parameterError("Quad takes four parameters, the last should be a graph filter");
        this.cursor["@type"] = "DeletedQuad";
        this.cursor.graph = this.cleanGraph(graphRef);
        return this;
      };
      WOQLQuery2.prototype.sub = function(classA, classB) {
        if (!classA || !classB)
          return this.parameterError("Subsumption takes two parameters, both URIs");
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Subsumption";
        this.cursor.parent = this.cleanNodeValue(classA);
        this.cursor.child = this.cleanNodeValue(classB);
        return this;
      };
      WOQLQuery2.prototype.subsumption = WOQLQuery2.prototype.sub;
      WOQLQuery2.prototype.eq = function(varName, varValue) {
        if (typeof varName === "undefined" || typeof varValue === "undefined")
          return this.parameterError("Equals takes two parameters");
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Equals";
        this.cursor.left = this.cleanObject(varName);
        this.cursor.right = this.cleanObject(varValue);
        return this;
      };
      WOQLQuery2.prototype.equals = WOQLQuery2.prototype.eq;
      WOQLQuery2.prototype.substr = function(string, before, length, after, subString) {
        if (!subString) {
          subString = after;
          after = 0;
        }
        if (!subString) {
          subString = length;
          length = subString.length + before;
        }
        if (!string || !subString || typeof subString !== "string") {
          return this.parameterError(
            "Substr - the first and last parameters must be strings representing the full and substring variables / literals"
          );
        }
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Substring";
        this.cursor.string = this.cleanDataValue(string, "xsd:string");
        this.cursor.before = this.cleanDataValue(before, "xsd:nonNegativeInteger");
        this.cursor.length = this.cleanDataValue(length, "xsd:nonNegativeInteger");
        this.cursor.after = this.cleanDataValue(after, "xsd:nonNegativeInteger");
        this.cursor.substring = this.cleanDataValue(subString, "xsd:string");
        return this;
      };
      WOQLQuery2.prototype.substring = WOQLQuery2.prototype.substr;
      WOQLQuery2.prototype.get = function(asvars, queryResource) {
        this.cursor["@type"] = "Get";
        this.cursor.columns = asvars.json ? asvars.json() : new WOQLQuery2().as(...asvars).json();
        if (queryResource) {
          this.cursor.resource = this.jobj(queryResource);
        } else {
          this.cursor.resource = {};
        }
        this.cursor = this.cursor.resource;
        return this;
      };
      WOQLQuery2.prototype.put = function(varsToExp, query, fileResource) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Put";
        if (Array.isArray(varsToExp) && typeof varsToExp[0] !== "object") {
          const nasvars = [];
          for (let i = 0; i < varsToExp.length; i++) {
            const iasv = this.asv(i, varsToExp[i]);
            nasvars.push(iasv);
            this.cursor.columns = nasvars;
          }
        } else {
          this.cursor.columns = varsToExp.json ? varsToExp.json() : new WOQLQuery2().as(...varsToExp).json();
        }
        this.cursor.query = this.jobj(query);
        if (fileResource) {
          this.cursor.resource = this.jobj(fileResource);
        } else {
          this.cursor.resource = {};
        }
        this.cursor = this.cursor.resource;
        return this;
      };
      WOQLQuery2.prototype.as = function(...varList) {
        if (!Array.isArray(this.query))
          this.query = [];
        if (Array.isArray(varList[0])) {
          if (!varList[1]) {
            for (var i = 0; i < varList[0].length; i++) {
              const iasv = this.asv(i, varList[0][i]);
              this.query.push(iasv);
            }
          } else {
            for (var i = 0; i < varList.length; i++) {
              const onemap = varList[i];
              if (Array.isArray(onemap) && onemap.length >= 2) {
                const type = onemap && onemap.length > 2 ? onemap[2] : false;
                const oasv2 = this.asv(onemap[0], onemap[1], type);
                this.query.push(oasv2);
              }
            }
          }
        } else if (typeof varList[0] === "number" || typeof varList[0] === "string") {
          if (varList[2] && typeof varList[2] === "string") {
            var oasv = this.asv(varList[0], varList[1], varList[2]);
          } else if (varList[1] && varList[1] instanceof Var2) {
            var oasv = this.asv(varList[0], varList[1]);
          } else if (varList[1] && typeof varList[1] === "string") {
            if (varList[1].substring(0, 4) === "xsd:" || varList[1].substring(0, 4) === "xdd:") {
              var oasv = this.asv(this.query.length, varList[0], varList[1]);
            } else {
              var oasv = this.asv(varList[0], varList[1]);
            }
          } else {
            var oasv = this.asv(this.query.length, varList[0]);
          }
          this.query.push(oasv);
        } else if (typeof varList[0] === "object") {
          this.query.push(varList[0].json ? varList[0].json() : varList[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.remote = function(remoteObj, formatObj) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "QueryResource";
        this.cursor.source = { "@type": "Source", url: remoteObj };
        this.cursor.format = "csv";
        if (typeof opts !== "undefined")
          this.cursor.options = formatObj;
        return this;
      };
      WOQLQuery2.prototype.post = function(url, formatObj, source = "post") {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "QueryResource";
        this.cursor.source = { "@type": "Source", [source]: url };
        this.cursor.format = "csv";
        this.cursor.options = formatObj;
        if (typeof formatObj !== "undefined")
          this.cursor.options = formatObj;
        return this;
      };
      WOQLQuery2.prototype.delete_triple = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        this.cursor["@type"] = "DeleteTriple";
        return this.updated();
      };
      WOQLQuery2.prototype.add_triple = function(subject, predicate, object) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        this.cursor["@type"] = "AddTriple";
        return this.updated();
      };
      WOQLQuery2.prototype.delete_quad = function(subject, predicate, object, graphRef) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        if (!graphRef) {
          return this.parameterError(
            "Delete Quad takes four parameters, the last should be a graph id"
          );
        }
        this.cursor["@type"] = "DeleteTriple";
        this.cursor.graph = this.cleanGraph(graphRef);
        return this.updated();
      };
      WOQLQuery2.prototype.add_quad = function(subject, predicate, object, graphRef) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        const args = this.triple(subject, predicate, object);
        if (!graphRef)
          return this.parameterError("Add Quad takes four parameters, the last should be a graph id");
        this.cursor["@type"] = "AddTriple";
        this.cursor.graph = this.cleanGraph(graphRef);
        return this.updated();
      };
      WOQLQuery2.prototype.trim = function(inputStr, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Trim";
        this.cursor.untrimmed = this.cleanDataValue(inputStr);
        this.cursor.trimmed = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.eval = function(arithExp, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Eval";
        this.cursor.expression = arithExp.json ? arithExp.json() : arithExp;
        this.cursor.result = this.cleanArithmeticValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.evaluate = function(arithExp, resultVarName) {
        return this.eval(arithExp, resultVarName);
      };
      WOQLQuery2.prototype.plus = function(...args) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Plus";
        this.cursor.left = this.arop(args.shift());
        if (args.length > 1) {
          this.cursor.right = this.jobj(new WOQLQuery2().plus(...args.map(this.arop)));
        } else {
          this.cursor.right = this.arop(args[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.minus = function(...args) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Minus";
        this.cursor.left = this.arop(args.shift());
        if (args.length > 1) {
          this.cursor.right = this.jobj(new WOQLQuery2().minus(...args.map(this.arop)));
        } else {
          this.cursor.right = this.arop(args[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.times = function(...args) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Times";
        this.cursor.left = this.arop(args.shift());
        if (args.length > 1) {
          this.cursor.right = this.jobj(new WOQLQuery2().times(...args.map(this.arop)));
        } else {
          this.cursor.right = this.arop(args[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.divide = function(...args) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Divide";
        this.cursor.left = this.arop(args.shift());
        if (args.length > 1) {
          this.cursor.right = this.jobj(new WOQLQuery2().divide(...args.map(this.arop)));
        } else {
          this.cursor.right = this.arop(args[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.div = function(...args) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Div";
        this.cursor.left = this.arop(args.shift());
        if (args.length > 1) {
          this.cursor.right = this.jobj(new WOQLQuery2().div(...args.map(this.arop)));
        } else {
          this.cursor.right = this.arop(args[0]);
        }
        return this;
      };
      WOQLQuery2.prototype.exp = function(varNum, expNum) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Exp";
        this.cursor.left = this.arop(varNum);
        this.cursor.right = this.arop(expNum);
        return this;
      };
      WOQLQuery2.prototype.floor = function(varNum) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Floor";
        this.cursor.argument = this.arop(varNum);
        return this;
      };
      WOQLQuery2.prototype.isa = function(instanceIRI, classId) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "IsA";
        this.cursor.element = this.cleanNodeValue(instanceIRI);
        this.cursor.type = this.cleanNodeValue(classId);
        return this;
      };
      WOQLQuery2.prototype.like = function(stringA, stringB, distance) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Like";
        this.cursor.left = this.cleanDataValue(stringA, "xsd:string");
        this.cursor.right = this.cleanDataValue(stringB, "xsd:string");
        if (distance) {
          this.cursor.similarity = this.cleanDataValue(distance, "xsd:decimal");
        }
        return this;
      };
      WOQLQuery2.prototype.less = function(varNum01, varNum02) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Less";
        this.cursor.left = this.cleanDataValue(varNum01);
        this.cursor.right = this.cleanDataValue(varNum02);
        return this;
      };
      WOQLQuery2.prototype.greater = function(varNum01, varNum02) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Greater";
        this.cursor.left = this.cleanDataValue(varNum01);
        this.cursor.right = this.cleanDataValue(varNum02);
        return this;
      };
      WOQLQuery2.prototype.opt = function(subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Optional";
        this.addSubQuery(subquery);
        return this;
      };
      WOQLQuery2.prototype.optional = WOQLQuery2.prototype.opt;
      WOQLQuery2.prototype.unique = function(prefix, inputVarList, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "HashKey";
        this.cursor.base = this.cleanDataValue(prefix, "xsd:string");
        this.cursor.key_list = this.cleanDataValue(inputVarList);
        this.cursor.uri = this.cleanNodeValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.idgen = function(prefix, inputVarList, outputVar) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "LexicalKey";
        this.cursor.base = this.cleanDataValue(prefix, "xsd:string");
        this.cursor.key_list = this.dataValueList(inputVarList);
        this.cursor.uri = this.cleanNodeValue(outputVar);
        return this;
      };
      WOQLQuery2.prototype.idgenerator = WOQLQuery2.prototype.idgen;
      WOQLQuery2.prototype.upper = function(inputVarName, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Upper";
        this.cursor.mixed = this.cleanDataValue(inputVarName);
        this.cursor.upper = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.lower = function(inputVarName, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Lower";
        this.cursor.mixed = this.cleanDataValue(inputVarName);
        this.cursor.lower = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.pad = function(inputVarName, pad, len, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Pad";
        this.cursor.string = this.cleanDataValue(inputVarName);
        this.cursor.char = this.cleanDataValue(pad);
        this.cursor.times = this.cleanDataValue(len, "xsd:integer");
        this.cursor.result = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.split = function(inputVarName, separator, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Split";
        this.cursor.string = this.cleanDataValue(inputVarName);
        this.cursor.pattern = this.cleanDataValue(separator);
        this.cursor.list = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.member = function(element, list) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Member";
        this.cursor.member = this.cleanObject(element);
        this.cursor.list = this.valueList(list);
        return this;
      };
      WOQLQuery2.prototype.concat = function(varList, resultVarName) {
        if (typeof varList === "string") {
          const slist = varList.split(/(v:)/);
          const nlist = [];
          if (slist[0])
            nlist.push(slist[0]);
          for (let i = 1; i < slist.length; i += 2) {
            if (slist[i]) {
              if (slist[i] === "v:") {
                const slist2 = slist[i + 1].split(/([^\w_])/);
                const x = slist2.shift();
                nlist.push(`v:${x}`);
                const rest = slist2.join("");
                if (rest)
                  nlist.push(rest);
              }
            }
          }
          varList = nlist;
        }
        if (Array.isArray(varList)) {
          if (this.cursor["@type"])
            this.wrapCursorWithAnd();
          this.cursor["@type"] = "Concatenate";
          this.cursor.list = this.cleanDataValue(varList, true);
          this.cursor.result = this.cleanDataValue(resultVarName);
        }
        return this;
      };
      WOQLQuery2.prototype.concatenate = WOQLQuery2.prototype.concat;
      WOQLQuery2.prototype.join = function(varList, glue, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Join";
        this.cursor.list = this.cleanDataValue(varList);
        this.cursor.separator = this.cleanDataValue(glue);
        this.cursor.result = this.cleanDataValue(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.sum = function(subquery, total) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Sum";
        this.cursor.list = this.cleanDataValue(subquery);
        this.cursor.result = this.cleanObject(total);
        return this;
      };
      WOQLQuery2.prototype.start = function(start, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Start";
        this.cursor.start = start;
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.limit = function(limit, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Limit";
        this.cursor.limit = limit;
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.re = function(pattern, inputVarName, resultVarList) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Regexp";
        this.cursor.pattern = this.cleanDataValue(pattern);
        this.cursor.string = this.cleanDataValue(inputVarName);
        this.cursor.result = this.cleanDataValue(resultVarList);
        return this;
      };
      WOQLQuery2.prototype.regexp = WOQLQuery2.prototype.re;
      WOQLQuery2.prototype.length = function(inputVarList, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Length";
        this.cursor.list = this.cleanDataValue(inputVarList);
        if (typeof resultVarName === "number") {
          this.cursor.length = this.cleanObject(resultVarName, "xsd:nonNegativeInteger");
        } else if (typeof resultVarName === "string") {
          this.cursor.length = this.varj(resultVarName);
        }
        return this;
      };
      WOQLQuery2.prototype.not = function(subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Not";
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.once = function(subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Once";
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.immediately = function(query) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Immediately";
        return this.addSubQuery(query);
      };
      WOQLQuery2.prototype.count = function(countVarName, subquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Count";
        this.cursor.count = this.cleanObject(countVarName);
        return this.addSubQuery(subquery);
      };
      WOQLQuery2.prototype.typecast = function(varName, varType, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Typecast";
        this.cursor.value = this.cleanObject(varName);
        this.cursor.type = this.cleanNodeValue(varType);
        this.cursor.result = this.cleanObject(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.cast = WOQLQuery2.prototype.typecast;
      WOQLQuery2.prototype.order_by = function(...orderedVarlist) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "OrderBy";
        this.cursor.ordering = [];
        if (!orderedVarlist || orderedVarlist.length === 0) {
          return this.parameterError(
            "Order by must be passed at least one variables to order the query"
          );
        }
        const embedquery = typeof orderedVarlist[orderedVarlist.length - 1] === "object" && orderedVarlist[orderedVarlist.length - 1].json ? orderedVarlist.pop() : false;
        for (let i = 0; i < orderedVarlist.length; i++) {
          let obj;
          if ((typeof orderedVarlist[i] === "string" || orderedVarlist[i] instanceof Var2) && orderedVarlist[i] !== "") {
            obj = {
              "@type": "OrderTemplate",
              variable: this.rawVar(orderedVarlist[i]),
              order: "asc"
            };
          } else if (orderedVarlist[i].length === 2 && orderedVarlist[i][1] === "asc") {
            obj = {
              "@type": "OrderTemplate",
              variable: this.rawVar(orderedVarlist[i][0]),
              order: "asc"
            };
          } else if (orderedVarlist[i].length === 2 && orderedVarlist[i][1] === "desc") {
            obj = {
              "@type": "OrderTemplate",
              variable: this.rawVar(orderedVarlist[i][0]),
              order: "desc"
            };
          }
          if (obj)
            this.cursor.ordering.push(obj);
        }
        return this.addSubQuery(embedquery);
      };
      WOQLQuery2.prototype.group_by = function(gvarlist, groupedvar, output, groupquery) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "GroupBy";
        this.cursor.group_by = [];
        if (typeof gvarlist === "string" || gvarlist instanceof Var2)
          gvarlist = [gvarlist];
        this.cursor.group_by = this.rawVarList(gvarlist);
        if (typeof groupedvar === "string" || groupedvar instanceof Var2)
          groupedvar = [groupedvar];
        this.cursor.template = this.rawVarList(groupedvar);
        this.cursor.grouped = this.varj(output);
        return this.addSubQuery(groupquery);
      };
      WOQLQuery2.prototype.true = function() {
        this.cursor["@type"] = "True";
        return this;
      };
      WOQLQuery2.prototype.path = function(subject, pattern, object, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Path";
        this.cursor.subject = this.cleanSubject(subject);
        if (typeof pattern === "string")
          pattern = this.compilePathPattern(pattern);
        this.cursor.pattern = pattern;
        this.cursor.object = this.cleanObject(object);
        if (typeof resultVarName !== "undefined") {
          this.cursor.path = this.varj(resultVarName);
        }
        return this;
      };
      WOQLQuery2.prototype.dot = function(document, field, value) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Dot";
        this.cursor.document = this.expandValueVariable(document);
        this.cursor.field = this.cleanDataValue(field, "xsd:string");
        this.cursor.value = this.expandValueVariable(value);
        return this;
      };
      WOQLQuery2.prototype.size = function(resourceId, resultVarName) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "Size";
        this.cursor.resource = this.cleanGraph(resourceId);
        this.cursor.size = this.varj(resultVarName);
        return this;
      };
      WOQLQuery2.prototype.triple_count = function(resourceId, TripleCount) {
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "TripleCount";
        this.cursor.resource = this.cleanGraph(resourceId);
        this.cursor.count = this.varj(TripleCount);
        return this;
      };
      WOQLQuery2.prototype.type_of = function(elementId, elementType) {
        if (!elementId || !elementType)
          return this.parameterError("type_of takes two parameters, both values");
        if (this.cursor["@type"])
          this.wrapCursorWithAnd();
        this.cursor["@type"] = "TypeOf";
        this.cursor.value = this.cleanObject(elementId);
        this.cursor.type = this.cleanSubject(elementType);
        return this;
      };
      module2.exports = WOQLQuery2;
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlBuilder.js
  var require_woqlBuilder = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlBuilder.js"(exports2, module2) {
      var WOQLQueryExt = require_woqlQuery();
      var typedef = require_typedef();
      var WOQLQuery2 = class extends WOQLQueryExt {
        // eslint-disable-next-line no-useless-constructor
        constructor(query) {
          super(query);
        }
      };
      WOQLQuery2.prototype.star = function(Graph, Subj, Pred, Obj) {
        Subj = Subj || "v:Subject";
        Pred = Pred || "v:Predicate";
        Obj = Obj || "v:Object";
        Graph = Graph || false;
        if (Graph) {
          return this.quad(Subj, Pred, Obj, Graph);
        }
        return this.triple(Subj, Pred, Obj);
      };
      WOQLQuery2.prototype.all = function(Subj, Pred, Obj, Graph) {
        return this.star(Graph, Subj, Pred, Obj);
      };
      WOQLQuery2.prototype.string = function(s) {
        return { "@type": "xsd:string", "@value": String(s) };
      };
      WOQLQuery2.prototype.boolean = function(tf) {
        tf = tf || false;
        return this.literal(tf, "boolean");
      };
      WOQLQuery2.prototype.literal = function(s, t) {
        t = t.indexOf(":") === -1 ? `xsd:${t}` : t;
        return { "@type": t, "@value": s };
      };
      WOQLQuery2.prototype.iri = function(s) {
        return {
          "@type": "NodeValue",
          node: s
        };
      };
      WOQLQuery2.prototype.update_triple = function(subject, predicate, newObjValue, oldObjValue) {
        const tmp_name = oldObjValue || `v:AnyObject__${this.counter += 1}`;
        return this.and(
          new WOQLQuery2().opt(
            new WOQLQuery2().triple(subject, predicate, tmp_name).delete_triple(subject, predicate, tmp_name).not().triple(subject, predicate, newObjValue)
          ),
          new WOQLQuery2().add_triple(subject, predicate, newObjValue)
        );
      };
      WOQLQuery2.prototype.update_quad = function(subject, predicate, newObject, graph) {
        const tmp_name = `v:AnyObject__${this.counter += 1}`;
        return this.and(
          new WOQLQuery2().opt(
            new WOQLQuery2().quad(subject, predicate, tmp_name, graph).delete_quad(subject, predicate, tmp_name, graph).not().quad(subject, predicate, newObject, graph)
          ),
          new WOQLQuery2().add_quad(subject, predicate, newObject, graph)
        );
      };
      WOQLQuery2.prototype.nuke = function(graphRef) {
        if (graphRef) {
          return this.quad("v:A", "v:B", "v:C", graphRef).delete_quad("v:A", "v:B", "v:C", graphRef);
        }
        return this.triple("v:A", "v:B", "v:C").delete_triple("v:A", "v:B", "v:C");
      };
      WOQLQuery2.prototype.node = function(node, type) {
        type = type || false;
        if (type === "add_quad")
          type = "AddTriple";
        else if (type === "delete_quad")
          type = "DeleteTriple";
        else if (type === "add_triple")
          type = "AddTriple";
        else if (type === "delete_triple")
          type = "DeleteTriple";
        else if (type === "quad")
          type = "Triple";
        else if (type === "triple")
          type = "Triple";
        if (type && type.indexOf(":") === -1)
          type = type;
        const ctxt = { subject: node };
        if (type)
          ctxt.action = type;
        this._set_context(ctxt);
        return this;
      };
      WOQLQuery2.prototype.graph = function(graphRef) {
        return this._set_context({
          graph: graphRef
        });
      };
      WOQLQuery2.prototype._set_context = function(ctxt) {
        if (!this.triple_builder_context)
          this.triple_builder_context = {};
        for (const k in ctxt) {
          this.triple_builder_context[k] = ctxt[k];
        }
        return this;
      };
      WOQLQuery2.prototype.insert = function(id, type, refGraph) {
        refGraph = refGraph || (this.triple_builder_context ? this.triple_builder_context.graph : false);
        if (refGraph) {
          return this.add_quad(id, "rdf:type", `@schema:${type}`, refGraph);
        }
        return this.add_triple(id, "rdf:type", `@schema:${type}`);
      };
      module2.exports = WOQLQuery2;
    }
  });

  // node_modules/@terminusdb/terminusdb-client/lib/query/woqlLibrary.js
  var require_woqlLibrary = __commonJS({
    "node_modules/@terminusdb/terminusdb-client/lib/query/woqlLibrary.js"(exports2, module2) {
      var WOQLQuery2 = require_woqlBuilder();
      var WOQLLibrary2 = class {
        default_schema_resource = "schema/main";
        default_commit_resource = "_commits";
        default_meta_resource = "_meta";
        masterdb_resource = "_system";
        empty = "";
      };
      WOQLLibrary2.prototype.branches = function() {
        const woql = new WOQLQuery2().using("_commits").triple("v:Branch", "rdf:type", "@schema:Branch").triple("v:Branch", "@schema:name", "v:Name").opt().triple("v:Branch", "@schema:head", "v:Head").triple("v:Head", "@schema:identifier", "v:commit_identifier").triple("v:Head", "@schema:timestamp", "v:Timestamp");
        return woql;
      };
      WOQLLibrary2.prototype.commits = function(branch = "main", limit = 0, start = 0, timestamp = 0) {
        const woql = new WOQLQuery2().using("_commits");
        if (limit)
          woql.limit(limit);
        if (start)
          woql.start(start);
        woql.select("v:Parent ID", "v:Commit ID", "v:Time", "v:Author", "v:Branch ID", "v:Message");
        const andArr = [new WOQLQuery2().triple("v:Branch", "name", new WOQLQuery2().string(branch)).triple("v:Branch", "head", "v:Active Commit ID").path("v:Active Commit ID", "parent*", "v:Parent", "v:Path").triple("v:Parent", "timestamp", "v:Time")];
        if (timestamp) {
          andArr.push(new WOQLQuery2().less("v:Time", timestamp));
        }
        andArr.push(new WOQLQuery2().triple("v:Parent", "identifier", "v:Commit ID").triple("v:Parent", "author", "v:Author").triple("v:Parent", "message", "v:Message").opt().triple("v:Parent", "parent", "v:Parent ID"));
        return woql.and(...andArr);
      };
      WOQLLibrary2.prototype.previousCommits = function(commit_id, limit = 10) {
        return new WOQLQuery2().using("_commits").limit(limit).select("v:Parent ID", "v:Message", "v:Commit ID", "v:Time", "v:Author").and(
          new WOQLQuery2().and(
            new WOQLQuery2().triple("v:Active Commit ID", "@schema:identifier", new WOQLQuery2().string(commit_id)),
            new WOQLQuery2().path("v:Active Commit ID", "@schema:parent+", "v:Parent", "v:Path"),
            new WOQLQuery2().triple("v:Parent", "@schema:identifier", "v:Commit ID"),
            new WOQLQuery2().triple("v:Parent", "@schema:timestamp", "v:Time"),
            new WOQLQuery2().triple("v:Parent", "@schema:author", "v:Author"),
            new WOQLQuery2().triple("v:Parent", "@schema:message", "v:Message"),
            new WOQLQuery2().triple("v:Parent", "@schema:parent", "v:Parent ID"),
            new WOQLQuery2().opt().triple("v:Parent", "parent", "v:Parent ID")
          )
        );
      };
      WOQLLibrary2.prototype.first_commit = function() {
        const noparent = new WOQLQuery2().using("_commits").select("v:Any Commit IRI").and(
          new WOQLQuery2().triple("v:Branch", "name", new WOQLQuery2().string("main")).triple("v:Branch", "head", "v:Active Commit ID").path("v:Active Commit ID", "parent*", "v:Any Commit IRI", "v:Path"),
          new WOQLQuery2().triple(
            "v:Any Commit IRI",
            "@schema:identifier",
            "v:Commit ID"
          ),
          new WOQLQuery2().triple(
            "v:Any Commit IRI",
            "@schema:author",
            "v:Author"
          ),
          new WOQLQuery2().triple(
            "v:Any Commit IRI",
            "@schema:message",
            "v:Message"
          ),
          new WOQLQuery2().not().triple(
            "v:Any Commit IRI",
            "@schema:parent",
            "v:Parent IRI"
          )
        );
        return noparent;
      };
      module2.exports = WOQLLibrary2;
    }
  });

  // woql-only.js
  var require_woql_only = __commonJS({
    "woql-only.js"(exports, module) {
      var WOQLQuery = require_woqlBuilder();
      var WOQLLibrary = require_woqlLibrary();
      var { Vars, Var, Doc, VarsUnique, VarUnique, SetVarsUniqueCounter } = require_woqlDoc();
      var WOQL = {};
      WOQL.using = function(refPath, subquery) {
        return new WOQLQuery().using(refPath, subquery);
      };
      WOQL.comment = function(comment, subquery) {
        return new WOQLQuery().comment(comment, subquery);
      };
      WOQL.select = function(...varNames) {
        return new WOQLQuery().select(...varNames);
      };
      WOQL.distinct = function(...varNames) {
        return new WOQLQuery().distinct(...varNames);
      };
      WOQL.and = function(...subqueries) {
        return new WOQLQuery().and(...subqueries);
      };
      WOQL.or = function(...subqueries) {
        return new WOQLQuery().or(...subqueries);
      };
      WOQL.from = function(graphRef, query) {
        return new WOQLQuery().from(graphRef, query);
      };
      WOQL.into = function(graphRef, subquery) {
        return new WOQLQuery().into(graphRef, subquery);
      };
      WOQL.triple = function(subject, predicate, object) {
        return new WOQLQuery().triple(subject, predicate, object);
      };
      WOQL.added_triple = function(subject, predicate, object) {
        return new WOQLQuery().added_triple(subject, predicate, object);
      };
      WOQL.removed_triple = function(subject, predicate, object) {
        return new WOQLQuery().removed_triple(subject, predicate, object);
      };
      WOQL.quad = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().quad(subject, predicate, object, graphRef);
      };
      WOQL.added_quad = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().added_quad(subject, predicate, object, graphRef);
      };
      WOQL.removed_quad = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().removed_quad(subject, predicate, object, graphRef);
      };
      WOQL.sub = function(classA, classB) {
        return new WOQLQuery().sub(classA, classB);
      };
      WOQL.subsumption = function(classA, classB) {
        return new WOQLQuery().sub(classA, classB);
      };
      WOQL.eq = function(varName, varValue) {
        return new WOQLQuery().eq(varName, varValue);
      };
      WOQL.equals = function(varName, varValue) {
        return new WOQLQuery().eq(varName, varValue);
      };
      WOQL.substr = function(string, before, length, after, substring) {
        return new WOQLQuery().substr(string, before, length, after, substring);
      };
      WOQL.substring = function(string, before, length, after, substring) {
        return new WOQLQuery().substr(string, before, length, after, substring);
      };
      WOQL.get = function(asvars, queryResource) {
        return new WOQLQuery().get(asvars, queryResource);
      };
      WOQL.put = function(varsToExp, query, fileResource) {
        return new WOQLQuery().put(varsToExp, query, fileResource);
      };
      WOQL.as = function(source, target, type) {
        return new WOQLQuery().as(source, target, type);
      };
      WOQL.remote = function(remoteObj, formatObj) {
        return new WOQLQuery().remote(remoteObj, formatObj);
      };
      WOQL.post = function(url, formatObj, source) {
        return new WOQLQuery().post(url, formatObj, source);
      };
      WOQL.delete_triple = function(subject, predicate, object) {
        return new WOQLQuery().delete_triple(subject, predicate, object);
      };
      WOQL.delete_quad = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().delete_quad(subject, predicate, object, graphRef);
      };
      WOQL.add_triple = function(subject, predicate, object) {
        return new WOQLQuery().add_triple(subject, predicate, object);
      };
      WOQL.add_quad = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().add_quad(subject, predicate, object, graphRef);
      };
      WOQL.trim = function(inputStr, resultVarName) {
        return new WOQLQuery().trim(inputStr, resultVarName);
      };
      WOQL.evaluate = function(arithExp, resultVarName) {
        return new WOQLQuery().eval(arithExp, resultVarName);
      };
      WOQL.eval = function(arithExp, resultVarName) {
        return new WOQLQuery().eval(arithExp, resultVarName);
      };
      WOQL.plus = function(...args) {
        return new WOQLQuery().plus(...args);
      };
      WOQL.minus = function(...args) {
        return new WOQLQuery().minus(...args);
      };
      WOQL.times = function(...args) {
        return new WOQLQuery().times(...args);
      };
      WOQL.divide = function(...args) {
        return new WOQLQuery().divide(...args);
      };
      WOQL.div = function(...args) {
        return new WOQLQuery().div(...args);
      };
      WOQL.exp = function(varNum, expNum) {
        return new WOQLQuery().exp(varNum, expNum);
      };
      WOQL.floor = function(varNum) {
        return new WOQLQuery().floor(varNum);
      };
      WOQL.isa = function(instanceIRI, classId) {
        return new WOQLQuery().isa(instanceIRI, classId);
      };
      WOQL.like = function(stringA, stringB, distance) {
        return new WOQLQuery().like(stringA, stringB, distance);
      };
      WOQL.less = function(varNum01, varNum02) {
        return new WOQLQuery().less(varNum01, varNum02);
      };
      WOQL.greater = function(varNum01, varNum02) {
        return new WOQLQuery().greater(varNum01, varNum02);
      };
      WOQL.opt = function(subquery) {
        return new WOQLQuery().opt(subquery);
      };
      WOQL.optional = function(subquery) {
        return new WOQLQuery().opt(subquery);
      };
      WOQL.unique = function(prefix, inputVarList, resultVarName) {
        return new WOQLQuery().unique(prefix, inputVarList, resultVarName);
      };
      WOQL.idgen = function(prefix, inputVarList, resultVarName) {
        return new WOQLQuery().idgen(prefix, inputVarList, resultVarName);
      };
      WOQL.idgenerator = function(prefix, inputVarList, resultVarName) {
        return new WOQLQuery().idgen(prefix, inputVarList, resultVarName);
      };
      WOQL.upper = function(inputVarName, resultVarName) {
        return new WOQLQuery().upper(inputVarName, resultVarName);
      };
      WOQL.lower = function(inputVarName, resultVarName) {
        return new WOQLQuery().lower(inputVarName, resultVarName);
      };
      WOQL.pad = function(inputVarName, pad, len, resultVarName) {
        return new WOQLQuery().pad(inputVarName, pad, len, resultVarName);
      };
      WOQL.split = function(inputVarName, separator, resultVarName) {
        return new WOQLQuery().split(inputVarName, separator, resultVarName);
      };
      WOQL.member = function(element, list) {
        return new WOQLQuery().member(element, list);
      };
      WOQL.concat = function(varList, resultVarName) {
        return new WOQLQuery().concat(varList, resultVarName);
      };
      WOQL.join = function(varList, glue, resultVarName) {
        return new WOQLQuery().join(varList, glue, resultVarName);
      };
      WOQL.sum = function(subquery, total) {
        return new WOQLQuery().sum(subquery, total);
      };
      WOQL.start = function(start, subquery) {
        return new WOQLQuery().start(start, subquery);
      };
      WOQL.limit = function(limit, subquery) {
        return new WOQLQuery().limit(limit, subquery);
      };
      WOQL.re = function(pattern, inputVarName, resultVarList) {
        return new WOQLQuery().re(pattern, inputVarName, resultVarList);
      };
      WOQL.regexp = function(pattern, inputVarName, resultVarList) {
        return new WOQLQuery().re(pattern, inputVarName, resultVarList);
      };
      WOQL.length = function(inputVarList, resultVarName) {
        return new WOQLQuery().length(inputVarList, resultVarName);
      };
      WOQL.not = function(subquery) {
        return new WOQLQuery().not(subquery);
      };
      WOQL.once = function(subquery) {
        return new WOQLQuery().once(subquery);
      };
      WOQL.immediately = function(subquery) {
        return new WOQLQuery().immediately(subquery);
      };
      WOQL.count = function(countVarName, subquery) {
        return new WOQLQuery().count(countVarName, subquery);
      };
      WOQL.typecast = function(varName, varType, resultVarName) {
        return new WOQLQuery().typecast(varName, varType, resultVarName);
      };
      WOQL.cast = function(varName, varType, resultVarName) {
        return new WOQLQuery().typecast(varName, varType, resultVarName);
      };
      WOQL.order_by = function(...varNames) {
        return new WOQLQuery().order_by(...varNames);
      };
      WOQL.group_by = function(varList, patternVars, resultVarName, subquery) {
        return new WOQLQuery().group_by(varList, patternVars, resultVarName, subquery);
      };
      WOQL.true = function() {
        return new WOQLQuery().true();
      };
      WOQL.path = function(subject, pattern, object, resultVarName) {
        return new WOQLQuery().path(subject, pattern, object, resultVarName);
      };
      WOQL.size = function(resourceId, resultVarName) {
        return new WOQLQuery().size(resourceId, resultVarName);
      };
      WOQL.triple_count = function(resourceId, tripleCount) {
        return new WOQLQuery().triple_count(resourceId, tripleCount);
      };
      WOQL.type_of = function(elementId, elementType) {
        return new WOQLQuery().type_of(elementId, elementType);
      };
      WOQL.star = function(graph, subject, predicate, object) {
        return new WOQLQuery().star(graph, subject, predicate, object);
      };
      WOQL.all = function(subject, predicate, object, graphRef) {
        return new WOQLQuery().all(subject, predicate, object, graphRef);
      };
      WOQL.node = function(nodeid, chainType) {
        return new WOQLQuery().node(nodeid, chainType);
      };
      WOQL.insert = function(classId, classType, graphRef) {
        return new WOQLQuery().insert(classId, classType, graphRef);
      };
      WOQL.graph = function(graphRef) {
        return new WOQLQuery().graph(graphRef);
      };
      WOQL.nuke = function(graphRef) {
        return new WOQLQuery().nuke(graphRef);
      };
      WOQL.query = function() {
        return new WOQLQuery();
      };
      WOQL.json = function(JSON_LD) {
        return new WOQLQuery().json(JSON_LD);
      };
      WOQL.lib = function() {
        return new WOQLLibrary();
      };
      WOQL.string = function(val) {
        return new WOQLQuery().string(val);
      };
      WOQL.literal = function(val, type) {
        return new WOQLQuery().literal(val, type);
      };
      WOQL.date = function(date) {
        return new WOQLQuery().literal(date, "xsd:date");
      };
      WOQL.datetime = function(datetime) {
        return new WOQLQuery().literal(datetime, "xsd:dateTime");
      };
      WOQL.boolean = function(bool) {
        return new WOQLQuery().boolean(bool);
      };
      WOQL.iri = function(val) {
        return new WOQLQuery().iri(val);
      };
      WOQL.vars = function(...varNames) {
        return varNames.map((item) => new Var(item));
      };
      WOQL.vars_unique = function(...varNames) {
        return varNames.map((item) => new VarUnique(item));
      };
      WOQL.vars_unique_reset_start = function(start) {
        SetVarsUniqueCounter(start ?? 0);
      };
      WOQL.doc = function(object) {
        return new Doc(object);
      };
      WOQL.Vars = function(...varNames) {
        return new Vars(...varNames);
      };
      WOQL.VarsUnique = function(...varNames) {
        return new VarsUnique(...varNames);
      };
      WOQL.read_document = function(IRI, output) {
        return new WOQLQuery().read_document(IRI, output);
      };
      WOQL.insert_document = function(docjson, IRI) {
        return new WOQLQuery().insert_document(docjson, IRI);
      };
      WOQL.update_document = function(docjson, IRI) {
        return new WOQLQuery().update_document(docjson, IRI);
      };
      WOQL.delete_document = function(IRI) {
        return new WOQLQuery().delete_document(IRI);
      };
      WOQL.update_triple = function(subject, predicate, newObjValue, oldObjValue) {
        return new WOQLQuery().update_triple(subject, predicate, newObjValue, oldObjValue);
      };
      WOQL.update_quad = function(subject, predicate, newObject, graphRef) {
        return new WOQLQuery().update_quad(subject, predicate, newObject, graphRef);
      };
      WOQL.value = function(subject, predicate, objValue) {
        return new WOQLQuery().value(subject, predicate, objValue);
      };
      WOQL.link = function(subject, predicate, object) {
        return new WOQLQuery().link(subject, predicate, object);
      };
      WOQL.dot = function(document, field, value) {
        return new WOQLQuery().dot(document, field, value);
      };
      WOQL.emerge = function(auto_eval) {
        const unemerged = ["emerge", "true", "eval"];
        function _emerge_str(k) {
          const str2 = `function ${k}(...args){
            return WOQL.${k}(...args)
        }`;
          return str2;
        }
        const funcs = [_emerge_str("Vars")];
        for (const k in this) {
          if (typeof this[k] === "function") {
            if (unemerged.indexOf(k) === -1) {
              funcs.push(_emerge_str(k));
            }
          }
        }
        const str = funcs.join(";\n");
        if (auto_eval)
          eval(str);
        return str;
      };
      module.exports = WOQL;
    }
  });

  // parse-woql-quickjs.js
  var require_parse_woql_quickjs = __commonJS({
    "parse-woql-quickjs.js"(exports, module) {
      var WOQL = require_woql_only();
      globalThis.parseWoql = function(queryString) {
        if (!queryString || typeof queryString !== "string") {
          throw new Error("Query must be a non-empty string");
        }
        const trimmed = queryString.trim();
        if (!trimmed) {
          throw new Error("Query must be a non-empty string");
        }
        const prelude = WOQL.emerge();
        const woqlQuery = eval(prelude + "\n" + trimmed);
        if (!woqlQuery) {
          throw new Error("Query evaluation returned null/undefined");
        }
        const jsonLD = woqlQuery.json();
        return JSON.stringify(jsonLD);
      };
    }
  });
  require_parse_woql_quickjs();
})();
/*! Bundled license information:

@terminusdb/terminusdb-client/lib/utils.js:
  (**
   * @file Terminus Client Utility Functions
   * @license Apache Version 2
   * Object for bunding up common Terminus Utility Functions
   *)

@terminusdb/terminusdb-client/lib/query/woqlLibrary.js:
  (**
   * @license Apache Version 2
   * @module WOQLLibrary
   * @constructor WOQLLibrary
   * @description Library Functions to manage the commits graph
   * @example
   *  const woqlLib = WOQLLibrary()
   *  woqlLib.branches()
   *
   *  //or you can call this functions using WOQL Class
   *  WOQL.lib().branches()
   * *)
*/
