#!/usr/bin/env node
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

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
    function Vars2(...args2) {
      const varObj = {};
      for (let i = 0, j = args2.length; i < j; i += 1) {
        const argumentName = args2[i];
        varObj[argumentName] = new Var2(argumentName);
      }
      return varObj;
    }
    function VarsUnique2(...args2) {
      const varObj = {};
      for (let i = 0, j = args2.length; i < j; i += 1) {
        const argumentName = args2[i];
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

// node_modules/delayed-stream/lib/delayed_stream.js
var require_delayed_stream = __commonJS({
  "node_modules/delayed-stream/lib/delayed_stream.js"(exports2, module2) {
    var Stream = require("stream").Stream;
    var util = require("util");
    module2.exports = DelayedStream;
    function DelayedStream() {
      this.source = null;
      this.dataSize = 0;
      this.maxDataSize = 1024 * 1024;
      this.pauseStream = true;
      this._maxDataSizeExceeded = false;
      this._released = false;
      this._bufferedEvents = [];
    }
    util.inherits(DelayedStream, Stream);
    DelayedStream.create = function(source, options) {
      var delayedStream = new this();
      options = options || {};
      for (var option in options) {
        delayedStream[option] = options[option];
      }
      delayedStream.source = source;
      var realEmit = source.emit;
      source.emit = function() {
        delayedStream._handleEmit(arguments);
        return realEmit.apply(source, arguments);
      };
      source.on("error", function() {
      });
      if (delayedStream.pauseStream) {
        source.pause();
      }
      return delayedStream;
    };
    Object.defineProperty(DelayedStream.prototype, "readable", {
      configurable: true,
      enumerable: true,
      get: function() {
        return this.source.readable;
      }
    });
    DelayedStream.prototype.setEncoding = function() {
      return this.source.setEncoding.apply(this.source, arguments);
    };
    DelayedStream.prototype.resume = function() {
      if (!this._released) {
        this.release();
      }
      this.source.resume();
    };
    DelayedStream.prototype.pause = function() {
      this.source.pause();
    };
    DelayedStream.prototype.release = function() {
      this._released = true;
      this._bufferedEvents.forEach(function(args2) {
        this.emit.apply(this, args2);
      }.bind(this));
      this._bufferedEvents = [];
    };
    DelayedStream.prototype.pipe = function() {
      var r = Stream.prototype.pipe.apply(this, arguments);
      this.resume();
      return r;
    };
    DelayedStream.prototype._handleEmit = function(args2) {
      if (this._released) {
        this.emit.apply(this, args2);
        return;
      }
      if (args2[0] === "data") {
        this.dataSize += args2[1].length;
        this._checkIfMaxDataSizeExceeded();
      }
      this._bufferedEvents.push(args2);
    };
    DelayedStream.prototype._checkIfMaxDataSizeExceeded = function() {
      if (this._maxDataSizeExceeded) {
        return;
      }
      if (this.dataSize <= this.maxDataSize) {
        return;
      }
      this._maxDataSizeExceeded = true;
      var message = "DelayedStream#maxDataSize of " + this.maxDataSize + " bytes exceeded.";
      this.emit("error", new Error(message));
    };
  }
});

// node_modules/combined-stream/lib/combined_stream.js
var require_combined_stream = __commonJS({
  "node_modules/combined-stream/lib/combined_stream.js"(exports2, module2) {
    var util = require("util");
    var Stream = require("stream").Stream;
    var DelayedStream = require_delayed_stream();
    module2.exports = CombinedStream;
    function CombinedStream() {
      this.writable = false;
      this.readable = true;
      this.dataSize = 0;
      this.maxDataSize = 2 * 1024 * 1024;
      this.pauseStreams = true;
      this._released = false;
      this._streams = [];
      this._currentStream = null;
      this._insideLoop = false;
      this._pendingNext = false;
    }
    util.inherits(CombinedStream, Stream);
    CombinedStream.create = function(options) {
      var combinedStream = new this();
      options = options || {};
      for (var option in options) {
        combinedStream[option] = options[option];
      }
      return combinedStream;
    };
    CombinedStream.isStreamLike = function(stream) {
      return typeof stream !== "function" && typeof stream !== "string" && typeof stream !== "boolean" && typeof stream !== "number" && !Buffer.isBuffer(stream);
    };
    CombinedStream.prototype.append = function(stream) {
      var isStreamLike = CombinedStream.isStreamLike(stream);
      if (isStreamLike) {
        if (!(stream instanceof DelayedStream)) {
          var newStream = DelayedStream.create(stream, {
            maxDataSize: Infinity,
            pauseStream: this.pauseStreams
          });
          stream.on("data", this._checkDataSize.bind(this));
          stream = newStream;
        }
        this._handleErrors(stream);
        if (this.pauseStreams) {
          stream.pause();
        }
      }
      this._streams.push(stream);
      return this;
    };
    CombinedStream.prototype.pipe = function(dest, options) {
      Stream.prototype.pipe.call(this, dest, options);
      this.resume();
      return dest;
    };
    CombinedStream.prototype._getNext = function() {
      this._currentStream = null;
      if (this._insideLoop) {
        this._pendingNext = true;
        return;
      }
      this._insideLoop = true;
      try {
        do {
          this._pendingNext = false;
          this._realGetNext();
        } while (this._pendingNext);
      } finally {
        this._insideLoop = false;
      }
    };
    CombinedStream.prototype._realGetNext = function() {
      var stream = this._streams.shift();
      if (typeof stream == "undefined") {
        this.end();
        return;
      }
      if (typeof stream !== "function") {
        this._pipeNext(stream);
        return;
      }
      var getStream = stream;
      getStream(function(stream2) {
        var isStreamLike = CombinedStream.isStreamLike(stream2);
        if (isStreamLike) {
          stream2.on("data", this._checkDataSize.bind(this));
          this._handleErrors(stream2);
        }
        this._pipeNext(stream2);
      }.bind(this));
    };
    CombinedStream.prototype._pipeNext = function(stream) {
      this._currentStream = stream;
      var isStreamLike = CombinedStream.isStreamLike(stream);
      if (isStreamLike) {
        stream.on("end", this._getNext.bind(this));
        stream.pipe(this, { end: false });
        return;
      }
      var value = stream;
      this.write(value);
      this._getNext();
    };
    CombinedStream.prototype._handleErrors = function(stream) {
      var self2 = this;
      stream.on("error", function(err) {
        self2._emitError(err);
      });
    };
    CombinedStream.prototype.write = function(data) {
      this.emit("data", data);
    };
    CombinedStream.prototype.pause = function() {
      if (!this.pauseStreams) {
        return;
      }
      if (this.pauseStreams && this._currentStream && typeof this._currentStream.pause == "function")
        this._currentStream.pause();
      this.emit("pause");
    };
    CombinedStream.prototype.resume = function() {
      if (!this._released) {
        this._released = true;
        this.writable = true;
        this._getNext();
      }
      if (this.pauseStreams && this._currentStream && typeof this._currentStream.resume == "function")
        this._currentStream.resume();
      this.emit("resume");
    };
    CombinedStream.prototype.end = function() {
      this._reset();
      this.emit("end");
    };
    CombinedStream.prototype.destroy = function() {
      this._reset();
      this.emit("close");
    };
    CombinedStream.prototype._reset = function() {
      this.writable = false;
      this._streams = [];
      this._currentStream = null;
    };
    CombinedStream.prototype._checkDataSize = function() {
      this._updateDataSize();
      if (this.dataSize <= this.maxDataSize) {
        return;
      }
      var message = "DelayedStream#maxDataSize of " + this.maxDataSize + " bytes exceeded.";
      this._emitError(new Error(message));
    };
    CombinedStream.prototype._updateDataSize = function() {
      this.dataSize = 0;
      var self2 = this;
      this._streams.forEach(function(stream) {
        if (!stream.dataSize) {
          return;
        }
        self2.dataSize += stream.dataSize;
      });
      if (this._currentStream && this._currentStream.dataSize) {
        this.dataSize += this._currentStream.dataSize;
      }
    };
    CombinedStream.prototype._emitError = function(err) {
      this._reset();
      this.emit("error", err);
    };
  }
});

// node_modules/mime-db/db.json
var require_db = __commonJS({
  "node_modules/mime-db/db.json"(exports2, module2) {
    module2.exports = {
      "application/1d-interleaved-parityfec": {
        source: "iana"
      },
      "application/3gpdash-qoe-report+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/3gpp-ims+xml": {
        source: "iana",
        compressible: true
      },
      "application/3gpphal+json": {
        source: "iana",
        compressible: true
      },
      "application/3gpphalforms+json": {
        source: "iana",
        compressible: true
      },
      "application/a2l": {
        source: "iana"
      },
      "application/ace+cbor": {
        source: "iana"
      },
      "application/activemessage": {
        source: "iana"
      },
      "application/activity+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-costmap+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-costmapfilter+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-directory+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-endpointcost+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-endpointcostparams+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-endpointprop+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-endpointpropparams+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-error+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-networkmap+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-networkmapfilter+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-updatestreamcontrol+json": {
        source: "iana",
        compressible: true
      },
      "application/alto-updatestreamparams+json": {
        source: "iana",
        compressible: true
      },
      "application/aml": {
        source: "iana"
      },
      "application/andrew-inset": {
        source: "iana",
        extensions: ["ez"]
      },
      "application/applefile": {
        source: "iana"
      },
      "application/applixware": {
        source: "apache",
        extensions: ["aw"]
      },
      "application/at+jwt": {
        source: "iana"
      },
      "application/atf": {
        source: "iana"
      },
      "application/atfx": {
        source: "iana"
      },
      "application/atom+xml": {
        source: "iana",
        compressible: true,
        extensions: ["atom"]
      },
      "application/atomcat+xml": {
        source: "iana",
        compressible: true,
        extensions: ["atomcat"]
      },
      "application/atomdeleted+xml": {
        source: "iana",
        compressible: true,
        extensions: ["atomdeleted"]
      },
      "application/atomicmail": {
        source: "iana"
      },
      "application/atomsvc+xml": {
        source: "iana",
        compressible: true,
        extensions: ["atomsvc"]
      },
      "application/atsc-dwd+xml": {
        source: "iana",
        compressible: true,
        extensions: ["dwd"]
      },
      "application/atsc-dynamic-event-message": {
        source: "iana"
      },
      "application/atsc-held+xml": {
        source: "iana",
        compressible: true,
        extensions: ["held"]
      },
      "application/atsc-rdt+json": {
        source: "iana",
        compressible: true
      },
      "application/atsc-rsat+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rsat"]
      },
      "application/atxml": {
        source: "iana"
      },
      "application/auth-policy+xml": {
        source: "iana",
        compressible: true
      },
      "application/bacnet-xdd+zip": {
        source: "iana",
        compressible: false
      },
      "application/batch-smtp": {
        source: "iana"
      },
      "application/bdoc": {
        compressible: false,
        extensions: ["bdoc"]
      },
      "application/beep+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/calendar+json": {
        source: "iana",
        compressible: true
      },
      "application/calendar+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xcs"]
      },
      "application/call-completion": {
        source: "iana"
      },
      "application/cals-1840": {
        source: "iana"
      },
      "application/captive+json": {
        source: "iana",
        compressible: true
      },
      "application/cbor": {
        source: "iana"
      },
      "application/cbor-seq": {
        source: "iana"
      },
      "application/cccex": {
        source: "iana"
      },
      "application/ccmp+xml": {
        source: "iana",
        compressible: true
      },
      "application/ccxml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["ccxml"]
      },
      "application/cdfx+xml": {
        source: "iana",
        compressible: true,
        extensions: ["cdfx"]
      },
      "application/cdmi-capability": {
        source: "iana",
        extensions: ["cdmia"]
      },
      "application/cdmi-container": {
        source: "iana",
        extensions: ["cdmic"]
      },
      "application/cdmi-domain": {
        source: "iana",
        extensions: ["cdmid"]
      },
      "application/cdmi-object": {
        source: "iana",
        extensions: ["cdmio"]
      },
      "application/cdmi-queue": {
        source: "iana",
        extensions: ["cdmiq"]
      },
      "application/cdni": {
        source: "iana"
      },
      "application/cea": {
        source: "iana"
      },
      "application/cea-2018+xml": {
        source: "iana",
        compressible: true
      },
      "application/cellml+xml": {
        source: "iana",
        compressible: true
      },
      "application/cfw": {
        source: "iana"
      },
      "application/city+json": {
        source: "iana",
        compressible: true
      },
      "application/clr": {
        source: "iana"
      },
      "application/clue+xml": {
        source: "iana",
        compressible: true
      },
      "application/clue_info+xml": {
        source: "iana",
        compressible: true
      },
      "application/cms": {
        source: "iana"
      },
      "application/cnrp+xml": {
        source: "iana",
        compressible: true
      },
      "application/coap-group+json": {
        source: "iana",
        compressible: true
      },
      "application/coap-payload": {
        source: "iana"
      },
      "application/commonground": {
        source: "iana"
      },
      "application/conference-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/cose": {
        source: "iana"
      },
      "application/cose-key": {
        source: "iana"
      },
      "application/cose-key-set": {
        source: "iana"
      },
      "application/cpl+xml": {
        source: "iana",
        compressible: true,
        extensions: ["cpl"]
      },
      "application/csrattrs": {
        source: "iana"
      },
      "application/csta+xml": {
        source: "iana",
        compressible: true
      },
      "application/cstadata+xml": {
        source: "iana",
        compressible: true
      },
      "application/csvm+json": {
        source: "iana",
        compressible: true
      },
      "application/cu-seeme": {
        source: "apache",
        extensions: ["cu"]
      },
      "application/cwt": {
        source: "iana"
      },
      "application/cybercash": {
        source: "iana"
      },
      "application/dart": {
        compressible: true
      },
      "application/dash+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mpd"]
      },
      "application/dash-patch+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mpp"]
      },
      "application/dashdelta": {
        source: "iana"
      },
      "application/davmount+xml": {
        source: "iana",
        compressible: true,
        extensions: ["davmount"]
      },
      "application/dca-rft": {
        source: "iana"
      },
      "application/dcd": {
        source: "iana"
      },
      "application/dec-dx": {
        source: "iana"
      },
      "application/dialog-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/dicom": {
        source: "iana"
      },
      "application/dicom+json": {
        source: "iana",
        compressible: true
      },
      "application/dicom+xml": {
        source: "iana",
        compressible: true
      },
      "application/dii": {
        source: "iana"
      },
      "application/dit": {
        source: "iana"
      },
      "application/dns": {
        source: "iana"
      },
      "application/dns+json": {
        source: "iana",
        compressible: true
      },
      "application/dns-message": {
        source: "iana"
      },
      "application/docbook+xml": {
        source: "apache",
        compressible: true,
        extensions: ["dbk"]
      },
      "application/dots+cbor": {
        source: "iana"
      },
      "application/dskpp+xml": {
        source: "iana",
        compressible: true
      },
      "application/dssc+der": {
        source: "iana",
        extensions: ["dssc"]
      },
      "application/dssc+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xdssc"]
      },
      "application/dvcs": {
        source: "iana"
      },
      "application/ecmascript": {
        source: "iana",
        compressible: true,
        extensions: ["es", "ecma"]
      },
      "application/edi-consent": {
        source: "iana"
      },
      "application/edi-x12": {
        source: "iana",
        compressible: false
      },
      "application/edifact": {
        source: "iana",
        compressible: false
      },
      "application/efi": {
        source: "iana"
      },
      "application/elm+json": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/elm+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.cap+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/emergencycalldata.comment+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.control+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.deviceinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.ecall.msd": {
        source: "iana"
      },
      "application/emergencycalldata.providerinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.serviceinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.subscriberinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/emergencycalldata.veds+xml": {
        source: "iana",
        compressible: true
      },
      "application/emma+xml": {
        source: "iana",
        compressible: true,
        extensions: ["emma"]
      },
      "application/emotionml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["emotionml"]
      },
      "application/encaprtp": {
        source: "iana"
      },
      "application/epp+xml": {
        source: "iana",
        compressible: true
      },
      "application/epub+zip": {
        source: "iana",
        compressible: false,
        extensions: ["epub"]
      },
      "application/eshop": {
        source: "iana"
      },
      "application/exi": {
        source: "iana",
        extensions: ["exi"]
      },
      "application/expect-ct-report+json": {
        source: "iana",
        compressible: true
      },
      "application/express": {
        source: "iana",
        extensions: ["exp"]
      },
      "application/fastinfoset": {
        source: "iana"
      },
      "application/fastsoap": {
        source: "iana"
      },
      "application/fdt+xml": {
        source: "iana",
        compressible: true,
        extensions: ["fdt"]
      },
      "application/fhir+json": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/fhir+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/fido.trusted-apps+json": {
        compressible: true
      },
      "application/fits": {
        source: "iana"
      },
      "application/flexfec": {
        source: "iana"
      },
      "application/font-sfnt": {
        source: "iana"
      },
      "application/font-tdpfr": {
        source: "iana",
        extensions: ["pfr"]
      },
      "application/font-woff": {
        source: "iana",
        compressible: false
      },
      "application/framework-attributes+xml": {
        source: "iana",
        compressible: true
      },
      "application/geo+json": {
        source: "iana",
        compressible: true,
        extensions: ["geojson"]
      },
      "application/geo+json-seq": {
        source: "iana"
      },
      "application/geopackage+sqlite3": {
        source: "iana"
      },
      "application/geoxacml+xml": {
        source: "iana",
        compressible: true
      },
      "application/gltf-buffer": {
        source: "iana"
      },
      "application/gml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["gml"]
      },
      "application/gpx+xml": {
        source: "apache",
        compressible: true,
        extensions: ["gpx"]
      },
      "application/gxf": {
        source: "apache",
        extensions: ["gxf"]
      },
      "application/gzip": {
        source: "iana",
        compressible: false,
        extensions: ["gz"]
      },
      "application/h224": {
        source: "iana"
      },
      "application/held+xml": {
        source: "iana",
        compressible: true
      },
      "application/hjson": {
        extensions: ["hjson"]
      },
      "application/http": {
        source: "iana"
      },
      "application/hyperstudio": {
        source: "iana",
        extensions: ["stk"]
      },
      "application/ibe-key-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/ibe-pkg-reply+xml": {
        source: "iana",
        compressible: true
      },
      "application/ibe-pp-data": {
        source: "iana"
      },
      "application/iges": {
        source: "iana"
      },
      "application/im-iscomposing+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/index": {
        source: "iana"
      },
      "application/index.cmd": {
        source: "iana"
      },
      "application/index.obj": {
        source: "iana"
      },
      "application/index.response": {
        source: "iana"
      },
      "application/index.vnd": {
        source: "iana"
      },
      "application/inkml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["ink", "inkml"]
      },
      "application/iotp": {
        source: "iana"
      },
      "application/ipfix": {
        source: "iana",
        extensions: ["ipfix"]
      },
      "application/ipp": {
        source: "iana"
      },
      "application/isup": {
        source: "iana"
      },
      "application/its+xml": {
        source: "iana",
        compressible: true,
        extensions: ["its"]
      },
      "application/java-archive": {
        source: "apache",
        compressible: false,
        extensions: ["jar", "war", "ear"]
      },
      "application/java-serialized-object": {
        source: "apache",
        compressible: false,
        extensions: ["ser"]
      },
      "application/java-vm": {
        source: "apache",
        compressible: false,
        extensions: ["class"]
      },
      "application/javascript": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["js", "mjs"]
      },
      "application/jf2feed+json": {
        source: "iana",
        compressible: true
      },
      "application/jose": {
        source: "iana"
      },
      "application/jose+json": {
        source: "iana",
        compressible: true
      },
      "application/jrd+json": {
        source: "iana",
        compressible: true
      },
      "application/jscalendar+json": {
        source: "iana",
        compressible: true
      },
      "application/json": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["json", "map"]
      },
      "application/json-patch+json": {
        source: "iana",
        compressible: true
      },
      "application/json-seq": {
        source: "iana"
      },
      "application/json5": {
        extensions: ["json5"]
      },
      "application/jsonml+json": {
        source: "apache",
        compressible: true,
        extensions: ["jsonml"]
      },
      "application/jwk+json": {
        source: "iana",
        compressible: true
      },
      "application/jwk-set+json": {
        source: "iana",
        compressible: true
      },
      "application/jwt": {
        source: "iana"
      },
      "application/kpml-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/kpml-response+xml": {
        source: "iana",
        compressible: true
      },
      "application/ld+json": {
        source: "iana",
        compressible: true,
        extensions: ["jsonld"]
      },
      "application/lgr+xml": {
        source: "iana",
        compressible: true,
        extensions: ["lgr"]
      },
      "application/link-format": {
        source: "iana"
      },
      "application/load-control+xml": {
        source: "iana",
        compressible: true
      },
      "application/lost+xml": {
        source: "iana",
        compressible: true,
        extensions: ["lostxml"]
      },
      "application/lostsync+xml": {
        source: "iana",
        compressible: true
      },
      "application/lpf+zip": {
        source: "iana",
        compressible: false
      },
      "application/lxf": {
        source: "iana"
      },
      "application/mac-binhex40": {
        source: "iana",
        extensions: ["hqx"]
      },
      "application/mac-compactpro": {
        source: "apache",
        extensions: ["cpt"]
      },
      "application/macwriteii": {
        source: "iana"
      },
      "application/mads+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mads"]
      },
      "application/manifest+json": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["webmanifest"]
      },
      "application/marc": {
        source: "iana",
        extensions: ["mrc"]
      },
      "application/marcxml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mrcx"]
      },
      "application/mathematica": {
        source: "iana",
        extensions: ["ma", "nb", "mb"]
      },
      "application/mathml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mathml"]
      },
      "application/mathml-content+xml": {
        source: "iana",
        compressible: true
      },
      "application/mathml-presentation+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-associated-procedure-description+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-deregister+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-envelope+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-msk+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-msk-response+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-protection-description+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-reception-report+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-register+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-register-response+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-schedule+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbms-user-service-description+xml": {
        source: "iana",
        compressible: true
      },
      "application/mbox": {
        source: "iana",
        extensions: ["mbox"]
      },
      "application/media-policy-dataset+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mpf"]
      },
      "application/media_control+xml": {
        source: "iana",
        compressible: true
      },
      "application/mediaservercontrol+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mscml"]
      },
      "application/merge-patch+json": {
        source: "iana",
        compressible: true
      },
      "application/metalink+xml": {
        source: "apache",
        compressible: true,
        extensions: ["metalink"]
      },
      "application/metalink4+xml": {
        source: "iana",
        compressible: true,
        extensions: ["meta4"]
      },
      "application/mets+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mets"]
      },
      "application/mf4": {
        source: "iana"
      },
      "application/mikey": {
        source: "iana"
      },
      "application/mipc": {
        source: "iana"
      },
      "application/missing-blocks+cbor-seq": {
        source: "iana"
      },
      "application/mmt-aei+xml": {
        source: "iana",
        compressible: true,
        extensions: ["maei"]
      },
      "application/mmt-usd+xml": {
        source: "iana",
        compressible: true,
        extensions: ["musd"]
      },
      "application/mods+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mods"]
      },
      "application/moss-keys": {
        source: "iana"
      },
      "application/moss-signature": {
        source: "iana"
      },
      "application/mosskey-data": {
        source: "iana"
      },
      "application/mosskey-request": {
        source: "iana"
      },
      "application/mp21": {
        source: "iana",
        extensions: ["m21", "mp21"]
      },
      "application/mp4": {
        source: "iana",
        extensions: ["mp4s", "m4p"]
      },
      "application/mpeg4-generic": {
        source: "iana"
      },
      "application/mpeg4-iod": {
        source: "iana"
      },
      "application/mpeg4-iod-xmt": {
        source: "iana"
      },
      "application/mrb-consumer+xml": {
        source: "iana",
        compressible: true
      },
      "application/mrb-publish+xml": {
        source: "iana",
        compressible: true
      },
      "application/msc-ivr+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/msc-mixer+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/msword": {
        source: "iana",
        compressible: false,
        extensions: ["doc", "dot"]
      },
      "application/mud+json": {
        source: "iana",
        compressible: true
      },
      "application/multipart-core": {
        source: "iana"
      },
      "application/mxf": {
        source: "iana",
        extensions: ["mxf"]
      },
      "application/n-quads": {
        source: "iana",
        extensions: ["nq"]
      },
      "application/n-triples": {
        source: "iana",
        extensions: ["nt"]
      },
      "application/nasdata": {
        source: "iana"
      },
      "application/news-checkgroups": {
        source: "iana",
        charset: "US-ASCII"
      },
      "application/news-groupinfo": {
        source: "iana",
        charset: "US-ASCII"
      },
      "application/news-transmission": {
        source: "iana"
      },
      "application/nlsml+xml": {
        source: "iana",
        compressible: true
      },
      "application/node": {
        source: "iana",
        extensions: ["cjs"]
      },
      "application/nss": {
        source: "iana"
      },
      "application/oauth-authz-req+jwt": {
        source: "iana"
      },
      "application/oblivious-dns-message": {
        source: "iana"
      },
      "application/ocsp-request": {
        source: "iana"
      },
      "application/ocsp-response": {
        source: "iana"
      },
      "application/octet-stream": {
        source: "iana",
        compressible: false,
        extensions: ["bin", "dms", "lrf", "mar", "so", "dist", "distz", "pkg", "bpk", "dump", "elc", "deploy", "exe", "dll", "deb", "dmg", "iso", "img", "msi", "msp", "msm", "buffer"]
      },
      "application/oda": {
        source: "iana",
        extensions: ["oda"]
      },
      "application/odm+xml": {
        source: "iana",
        compressible: true
      },
      "application/odx": {
        source: "iana"
      },
      "application/oebps-package+xml": {
        source: "iana",
        compressible: true,
        extensions: ["opf"]
      },
      "application/ogg": {
        source: "iana",
        compressible: false,
        extensions: ["ogx"]
      },
      "application/omdoc+xml": {
        source: "apache",
        compressible: true,
        extensions: ["omdoc"]
      },
      "application/onenote": {
        source: "apache",
        extensions: ["onetoc", "onetoc2", "onetmp", "onepkg"]
      },
      "application/opc-nodeset+xml": {
        source: "iana",
        compressible: true
      },
      "application/oscore": {
        source: "iana"
      },
      "application/oxps": {
        source: "iana",
        extensions: ["oxps"]
      },
      "application/p21": {
        source: "iana"
      },
      "application/p21+zip": {
        source: "iana",
        compressible: false
      },
      "application/p2p-overlay+xml": {
        source: "iana",
        compressible: true,
        extensions: ["relo"]
      },
      "application/parityfec": {
        source: "iana"
      },
      "application/passport": {
        source: "iana"
      },
      "application/patch-ops-error+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xer"]
      },
      "application/pdf": {
        source: "iana",
        compressible: false,
        extensions: ["pdf"]
      },
      "application/pdx": {
        source: "iana"
      },
      "application/pem-certificate-chain": {
        source: "iana"
      },
      "application/pgp-encrypted": {
        source: "iana",
        compressible: false,
        extensions: ["pgp"]
      },
      "application/pgp-keys": {
        source: "iana",
        extensions: ["asc"]
      },
      "application/pgp-signature": {
        source: "iana",
        extensions: ["asc", "sig"]
      },
      "application/pics-rules": {
        source: "apache",
        extensions: ["prf"]
      },
      "application/pidf+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/pidf-diff+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/pkcs10": {
        source: "iana",
        extensions: ["p10"]
      },
      "application/pkcs12": {
        source: "iana"
      },
      "application/pkcs7-mime": {
        source: "iana",
        extensions: ["p7m", "p7c"]
      },
      "application/pkcs7-signature": {
        source: "iana",
        extensions: ["p7s"]
      },
      "application/pkcs8": {
        source: "iana",
        extensions: ["p8"]
      },
      "application/pkcs8-encrypted": {
        source: "iana"
      },
      "application/pkix-attr-cert": {
        source: "iana",
        extensions: ["ac"]
      },
      "application/pkix-cert": {
        source: "iana",
        extensions: ["cer"]
      },
      "application/pkix-crl": {
        source: "iana",
        extensions: ["crl"]
      },
      "application/pkix-pkipath": {
        source: "iana",
        extensions: ["pkipath"]
      },
      "application/pkixcmp": {
        source: "iana",
        extensions: ["pki"]
      },
      "application/pls+xml": {
        source: "iana",
        compressible: true,
        extensions: ["pls"]
      },
      "application/poc-settings+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/postscript": {
        source: "iana",
        compressible: true,
        extensions: ["ai", "eps", "ps"]
      },
      "application/ppsp-tracker+json": {
        source: "iana",
        compressible: true
      },
      "application/problem+json": {
        source: "iana",
        compressible: true
      },
      "application/problem+xml": {
        source: "iana",
        compressible: true
      },
      "application/provenance+xml": {
        source: "iana",
        compressible: true,
        extensions: ["provx"]
      },
      "application/prs.alvestrand.titrax-sheet": {
        source: "iana"
      },
      "application/prs.cww": {
        source: "iana",
        extensions: ["cww"]
      },
      "application/prs.cyn": {
        source: "iana",
        charset: "7-BIT"
      },
      "application/prs.hpub+zip": {
        source: "iana",
        compressible: false
      },
      "application/prs.nprend": {
        source: "iana"
      },
      "application/prs.plucker": {
        source: "iana"
      },
      "application/prs.rdf-xml-crypt": {
        source: "iana"
      },
      "application/prs.xsf+xml": {
        source: "iana",
        compressible: true
      },
      "application/pskc+xml": {
        source: "iana",
        compressible: true,
        extensions: ["pskcxml"]
      },
      "application/pvd+json": {
        source: "iana",
        compressible: true
      },
      "application/qsig": {
        source: "iana"
      },
      "application/raml+yaml": {
        compressible: true,
        extensions: ["raml"]
      },
      "application/raptorfec": {
        source: "iana"
      },
      "application/rdap+json": {
        source: "iana",
        compressible: true
      },
      "application/rdf+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rdf", "owl"]
      },
      "application/reginfo+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rif"]
      },
      "application/relax-ng-compact-syntax": {
        source: "iana",
        extensions: ["rnc"]
      },
      "application/remote-printing": {
        source: "iana"
      },
      "application/reputon+json": {
        source: "iana",
        compressible: true
      },
      "application/resource-lists+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rl"]
      },
      "application/resource-lists-diff+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rld"]
      },
      "application/rfc+xml": {
        source: "iana",
        compressible: true
      },
      "application/riscos": {
        source: "iana"
      },
      "application/rlmi+xml": {
        source: "iana",
        compressible: true
      },
      "application/rls-services+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rs"]
      },
      "application/route-apd+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rapd"]
      },
      "application/route-s-tsid+xml": {
        source: "iana",
        compressible: true,
        extensions: ["sls"]
      },
      "application/route-usd+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rusd"]
      },
      "application/rpki-ghostbusters": {
        source: "iana",
        extensions: ["gbr"]
      },
      "application/rpki-manifest": {
        source: "iana",
        extensions: ["mft"]
      },
      "application/rpki-publication": {
        source: "iana"
      },
      "application/rpki-roa": {
        source: "iana",
        extensions: ["roa"]
      },
      "application/rpki-updown": {
        source: "iana"
      },
      "application/rsd+xml": {
        source: "apache",
        compressible: true,
        extensions: ["rsd"]
      },
      "application/rss+xml": {
        source: "apache",
        compressible: true,
        extensions: ["rss"]
      },
      "application/rtf": {
        source: "iana",
        compressible: true,
        extensions: ["rtf"]
      },
      "application/rtploopback": {
        source: "iana"
      },
      "application/rtx": {
        source: "iana"
      },
      "application/samlassertion+xml": {
        source: "iana",
        compressible: true
      },
      "application/samlmetadata+xml": {
        source: "iana",
        compressible: true
      },
      "application/sarif+json": {
        source: "iana",
        compressible: true
      },
      "application/sarif-external-properties+json": {
        source: "iana",
        compressible: true
      },
      "application/sbe": {
        source: "iana"
      },
      "application/sbml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["sbml"]
      },
      "application/scaip+xml": {
        source: "iana",
        compressible: true
      },
      "application/scim+json": {
        source: "iana",
        compressible: true
      },
      "application/scvp-cv-request": {
        source: "iana",
        extensions: ["scq"]
      },
      "application/scvp-cv-response": {
        source: "iana",
        extensions: ["scs"]
      },
      "application/scvp-vp-request": {
        source: "iana",
        extensions: ["spq"]
      },
      "application/scvp-vp-response": {
        source: "iana",
        extensions: ["spp"]
      },
      "application/sdp": {
        source: "iana",
        extensions: ["sdp"]
      },
      "application/secevent+jwt": {
        source: "iana"
      },
      "application/senml+cbor": {
        source: "iana"
      },
      "application/senml+json": {
        source: "iana",
        compressible: true
      },
      "application/senml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["senmlx"]
      },
      "application/senml-etch+cbor": {
        source: "iana"
      },
      "application/senml-etch+json": {
        source: "iana",
        compressible: true
      },
      "application/senml-exi": {
        source: "iana"
      },
      "application/sensml+cbor": {
        source: "iana"
      },
      "application/sensml+json": {
        source: "iana",
        compressible: true
      },
      "application/sensml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["sensmlx"]
      },
      "application/sensml-exi": {
        source: "iana"
      },
      "application/sep+xml": {
        source: "iana",
        compressible: true
      },
      "application/sep-exi": {
        source: "iana"
      },
      "application/session-info": {
        source: "iana"
      },
      "application/set-payment": {
        source: "iana"
      },
      "application/set-payment-initiation": {
        source: "iana",
        extensions: ["setpay"]
      },
      "application/set-registration": {
        source: "iana"
      },
      "application/set-registration-initiation": {
        source: "iana",
        extensions: ["setreg"]
      },
      "application/sgml": {
        source: "iana"
      },
      "application/sgml-open-catalog": {
        source: "iana"
      },
      "application/shf+xml": {
        source: "iana",
        compressible: true,
        extensions: ["shf"]
      },
      "application/sieve": {
        source: "iana",
        extensions: ["siv", "sieve"]
      },
      "application/simple-filter+xml": {
        source: "iana",
        compressible: true
      },
      "application/simple-message-summary": {
        source: "iana"
      },
      "application/simplesymbolcontainer": {
        source: "iana"
      },
      "application/sipc": {
        source: "iana"
      },
      "application/slate": {
        source: "iana"
      },
      "application/smil": {
        source: "iana"
      },
      "application/smil+xml": {
        source: "iana",
        compressible: true,
        extensions: ["smi", "smil"]
      },
      "application/smpte336m": {
        source: "iana"
      },
      "application/soap+fastinfoset": {
        source: "iana"
      },
      "application/soap+xml": {
        source: "iana",
        compressible: true
      },
      "application/sparql-query": {
        source: "iana",
        extensions: ["rq"]
      },
      "application/sparql-results+xml": {
        source: "iana",
        compressible: true,
        extensions: ["srx"]
      },
      "application/spdx+json": {
        source: "iana",
        compressible: true
      },
      "application/spirits-event+xml": {
        source: "iana",
        compressible: true
      },
      "application/sql": {
        source: "iana"
      },
      "application/srgs": {
        source: "iana",
        extensions: ["gram"]
      },
      "application/srgs+xml": {
        source: "iana",
        compressible: true,
        extensions: ["grxml"]
      },
      "application/sru+xml": {
        source: "iana",
        compressible: true,
        extensions: ["sru"]
      },
      "application/ssdl+xml": {
        source: "apache",
        compressible: true,
        extensions: ["ssdl"]
      },
      "application/ssml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["ssml"]
      },
      "application/stix+json": {
        source: "iana",
        compressible: true
      },
      "application/swid+xml": {
        source: "iana",
        compressible: true,
        extensions: ["swidtag"]
      },
      "application/tamp-apex-update": {
        source: "iana"
      },
      "application/tamp-apex-update-confirm": {
        source: "iana"
      },
      "application/tamp-community-update": {
        source: "iana"
      },
      "application/tamp-community-update-confirm": {
        source: "iana"
      },
      "application/tamp-error": {
        source: "iana"
      },
      "application/tamp-sequence-adjust": {
        source: "iana"
      },
      "application/tamp-sequence-adjust-confirm": {
        source: "iana"
      },
      "application/tamp-status-query": {
        source: "iana"
      },
      "application/tamp-status-response": {
        source: "iana"
      },
      "application/tamp-update": {
        source: "iana"
      },
      "application/tamp-update-confirm": {
        source: "iana"
      },
      "application/tar": {
        compressible: true
      },
      "application/taxii+json": {
        source: "iana",
        compressible: true
      },
      "application/td+json": {
        source: "iana",
        compressible: true
      },
      "application/tei+xml": {
        source: "iana",
        compressible: true,
        extensions: ["tei", "teicorpus"]
      },
      "application/tetra_isi": {
        source: "iana"
      },
      "application/thraud+xml": {
        source: "iana",
        compressible: true,
        extensions: ["tfi"]
      },
      "application/timestamp-query": {
        source: "iana"
      },
      "application/timestamp-reply": {
        source: "iana"
      },
      "application/timestamped-data": {
        source: "iana",
        extensions: ["tsd"]
      },
      "application/tlsrpt+gzip": {
        source: "iana"
      },
      "application/tlsrpt+json": {
        source: "iana",
        compressible: true
      },
      "application/tnauthlist": {
        source: "iana"
      },
      "application/token-introspection+jwt": {
        source: "iana"
      },
      "application/toml": {
        compressible: true,
        extensions: ["toml"]
      },
      "application/trickle-ice-sdpfrag": {
        source: "iana"
      },
      "application/trig": {
        source: "iana",
        extensions: ["trig"]
      },
      "application/ttml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["ttml"]
      },
      "application/tve-trigger": {
        source: "iana"
      },
      "application/tzif": {
        source: "iana"
      },
      "application/tzif-leap": {
        source: "iana"
      },
      "application/ubjson": {
        compressible: false,
        extensions: ["ubj"]
      },
      "application/ulpfec": {
        source: "iana"
      },
      "application/urc-grpsheet+xml": {
        source: "iana",
        compressible: true
      },
      "application/urc-ressheet+xml": {
        source: "iana",
        compressible: true,
        extensions: ["rsheet"]
      },
      "application/urc-targetdesc+xml": {
        source: "iana",
        compressible: true,
        extensions: ["td"]
      },
      "application/urc-uisocketdesc+xml": {
        source: "iana",
        compressible: true
      },
      "application/vcard+json": {
        source: "iana",
        compressible: true
      },
      "application/vcard+xml": {
        source: "iana",
        compressible: true
      },
      "application/vemmi": {
        source: "iana"
      },
      "application/vividence.scriptfile": {
        source: "apache"
      },
      "application/vnd.1000minds.decision-model+xml": {
        source: "iana",
        compressible: true,
        extensions: ["1km"]
      },
      "application/vnd.3gpp-prose+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp-prose-pc3ch+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp-v2x-local-service-information": {
        source: "iana"
      },
      "application/vnd.3gpp.5gnas": {
        source: "iana"
      },
      "application/vnd.3gpp.access-transfer-events+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.bsf+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.gmop+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.gtpc": {
        source: "iana"
      },
      "application/vnd.3gpp.interworking-data": {
        source: "iana"
      },
      "application/vnd.3gpp.lpp": {
        source: "iana"
      },
      "application/vnd.3gpp.mc-signalling-ear": {
        source: "iana"
      },
      "application/vnd.3gpp.mcdata-affiliation-command+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcdata-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcdata-payload": {
        source: "iana"
      },
      "application/vnd.3gpp.mcdata-service-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcdata-signalling": {
        source: "iana"
      },
      "application/vnd.3gpp.mcdata-ue-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcdata-user-profile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-affiliation-command+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-floor-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-location-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-mbms-usage-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-service-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-signed+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-ue-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-ue-init-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcptt-user-profile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-affiliation-command+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-affiliation-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-location-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-mbms-usage-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-service-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-transmission-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-ue-config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mcvideo-user-profile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.mid-call+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.ngap": {
        source: "iana"
      },
      "application/vnd.3gpp.pfcp": {
        source: "iana"
      },
      "application/vnd.3gpp.pic-bw-large": {
        source: "iana",
        extensions: ["plb"]
      },
      "application/vnd.3gpp.pic-bw-small": {
        source: "iana",
        extensions: ["psb"]
      },
      "application/vnd.3gpp.pic-bw-var": {
        source: "iana",
        extensions: ["pvb"]
      },
      "application/vnd.3gpp.s1ap": {
        source: "iana"
      },
      "application/vnd.3gpp.sms": {
        source: "iana"
      },
      "application/vnd.3gpp.sms+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.srvcc-ext+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.srvcc-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.state-and-event-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp.ussd+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp2.bcmcsinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.3gpp2.sms": {
        source: "iana"
      },
      "application/vnd.3gpp2.tcap": {
        source: "iana",
        extensions: ["tcap"]
      },
      "application/vnd.3lightssoftware.imagescal": {
        source: "iana"
      },
      "application/vnd.3m.post-it-notes": {
        source: "iana",
        extensions: ["pwn"]
      },
      "application/vnd.accpac.simply.aso": {
        source: "iana",
        extensions: ["aso"]
      },
      "application/vnd.accpac.simply.imp": {
        source: "iana",
        extensions: ["imp"]
      },
      "application/vnd.acucobol": {
        source: "iana",
        extensions: ["acu"]
      },
      "application/vnd.acucorp": {
        source: "iana",
        extensions: ["atc", "acutc"]
      },
      "application/vnd.adobe.air-application-installer-package+zip": {
        source: "apache",
        compressible: false,
        extensions: ["air"]
      },
      "application/vnd.adobe.flash.movie": {
        source: "iana"
      },
      "application/vnd.adobe.formscentral.fcdt": {
        source: "iana",
        extensions: ["fcdt"]
      },
      "application/vnd.adobe.fxp": {
        source: "iana",
        extensions: ["fxp", "fxpl"]
      },
      "application/vnd.adobe.partial-upload": {
        source: "iana"
      },
      "application/vnd.adobe.xdp+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xdp"]
      },
      "application/vnd.adobe.xfdf": {
        source: "iana",
        extensions: ["xfdf"]
      },
      "application/vnd.aether.imp": {
        source: "iana"
      },
      "application/vnd.afpc.afplinedata": {
        source: "iana"
      },
      "application/vnd.afpc.afplinedata-pagedef": {
        source: "iana"
      },
      "application/vnd.afpc.cmoca-cmresource": {
        source: "iana"
      },
      "application/vnd.afpc.foca-charset": {
        source: "iana"
      },
      "application/vnd.afpc.foca-codedfont": {
        source: "iana"
      },
      "application/vnd.afpc.foca-codepage": {
        source: "iana"
      },
      "application/vnd.afpc.modca": {
        source: "iana"
      },
      "application/vnd.afpc.modca-cmtable": {
        source: "iana"
      },
      "application/vnd.afpc.modca-formdef": {
        source: "iana"
      },
      "application/vnd.afpc.modca-mediummap": {
        source: "iana"
      },
      "application/vnd.afpc.modca-objectcontainer": {
        source: "iana"
      },
      "application/vnd.afpc.modca-overlay": {
        source: "iana"
      },
      "application/vnd.afpc.modca-pagesegment": {
        source: "iana"
      },
      "application/vnd.age": {
        source: "iana",
        extensions: ["age"]
      },
      "application/vnd.ah-barcode": {
        source: "iana"
      },
      "application/vnd.ahead.space": {
        source: "iana",
        extensions: ["ahead"]
      },
      "application/vnd.airzip.filesecure.azf": {
        source: "iana",
        extensions: ["azf"]
      },
      "application/vnd.airzip.filesecure.azs": {
        source: "iana",
        extensions: ["azs"]
      },
      "application/vnd.amadeus+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.amazon.ebook": {
        source: "apache",
        extensions: ["azw"]
      },
      "application/vnd.amazon.mobi8-ebook": {
        source: "iana"
      },
      "application/vnd.americandynamics.acc": {
        source: "iana",
        extensions: ["acc"]
      },
      "application/vnd.amiga.ami": {
        source: "iana",
        extensions: ["ami"]
      },
      "application/vnd.amundsen.maze+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.android.ota": {
        source: "iana"
      },
      "application/vnd.android.package-archive": {
        source: "apache",
        compressible: false,
        extensions: ["apk"]
      },
      "application/vnd.anki": {
        source: "iana"
      },
      "application/vnd.anser-web-certificate-issue-initiation": {
        source: "iana",
        extensions: ["cii"]
      },
      "application/vnd.anser-web-funds-transfer-initiation": {
        source: "apache",
        extensions: ["fti"]
      },
      "application/vnd.antix.game-component": {
        source: "iana",
        extensions: ["atx"]
      },
      "application/vnd.apache.arrow.file": {
        source: "iana"
      },
      "application/vnd.apache.arrow.stream": {
        source: "iana"
      },
      "application/vnd.apache.thrift.binary": {
        source: "iana"
      },
      "application/vnd.apache.thrift.compact": {
        source: "iana"
      },
      "application/vnd.apache.thrift.json": {
        source: "iana"
      },
      "application/vnd.api+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.aplextor.warrp+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.apothekende.reservation+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.apple.installer+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mpkg"]
      },
      "application/vnd.apple.keynote": {
        source: "iana",
        extensions: ["key"]
      },
      "application/vnd.apple.mpegurl": {
        source: "iana",
        extensions: ["m3u8"]
      },
      "application/vnd.apple.numbers": {
        source: "iana",
        extensions: ["numbers"]
      },
      "application/vnd.apple.pages": {
        source: "iana",
        extensions: ["pages"]
      },
      "application/vnd.apple.pkpass": {
        compressible: false,
        extensions: ["pkpass"]
      },
      "application/vnd.arastra.swi": {
        source: "iana"
      },
      "application/vnd.aristanetworks.swi": {
        source: "iana",
        extensions: ["swi"]
      },
      "application/vnd.artisan+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.artsquare": {
        source: "iana"
      },
      "application/vnd.astraea-software.iota": {
        source: "iana",
        extensions: ["iota"]
      },
      "application/vnd.audiograph": {
        source: "iana",
        extensions: ["aep"]
      },
      "application/vnd.autopackage": {
        source: "iana"
      },
      "application/vnd.avalon+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.avistar+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.balsamiq.bmml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["bmml"]
      },
      "application/vnd.balsamiq.bmpr": {
        source: "iana"
      },
      "application/vnd.banana-accounting": {
        source: "iana"
      },
      "application/vnd.bbf.usp.error": {
        source: "iana"
      },
      "application/vnd.bbf.usp.msg": {
        source: "iana"
      },
      "application/vnd.bbf.usp.msg+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.bekitzur-stech+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.bint.med-content": {
        source: "iana"
      },
      "application/vnd.biopax.rdf+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.blink-idb-value-wrapper": {
        source: "iana"
      },
      "application/vnd.blueice.multipass": {
        source: "iana",
        extensions: ["mpm"]
      },
      "application/vnd.bluetooth.ep.oob": {
        source: "iana"
      },
      "application/vnd.bluetooth.le.oob": {
        source: "iana"
      },
      "application/vnd.bmi": {
        source: "iana",
        extensions: ["bmi"]
      },
      "application/vnd.bpf": {
        source: "iana"
      },
      "application/vnd.bpf3": {
        source: "iana"
      },
      "application/vnd.businessobjects": {
        source: "iana",
        extensions: ["rep"]
      },
      "application/vnd.byu.uapi+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cab-jscript": {
        source: "iana"
      },
      "application/vnd.canon-cpdl": {
        source: "iana"
      },
      "application/vnd.canon-lips": {
        source: "iana"
      },
      "application/vnd.capasystems-pg+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cendio.thinlinc.clientconf": {
        source: "iana"
      },
      "application/vnd.century-systems.tcp_stream": {
        source: "iana"
      },
      "application/vnd.chemdraw+xml": {
        source: "iana",
        compressible: true,
        extensions: ["cdxml"]
      },
      "application/vnd.chess-pgn": {
        source: "iana"
      },
      "application/vnd.chipnuts.karaoke-mmd": {
        source: "iana",
        extensions: ["mmd"]
      },
      "application/vnd.ciedi": {
        source: "iana"
      },
      "application/vnd.cinderella": {
        source: "iana",
        extensions: ["cdy"]
      },
      "application/vnd.cirpack.isdn-ext": {
        source: "iana"
      },
      "application/vnd.citationstyles.style+xml": {
        source: "iana",
        compressible: true,
        extensions: ["csl"]
      },
      "application/vnd.claymore": {
        source: "iana",
        extensions: ["cla"]
      },
      "application/vnd.cloanto.rp9": {
        source: "iana",
        extensions: ["rp9"]
      },
      "application/vnd.clonk.c4group": {
        source: "iana",
        extensions: ["c4g", "c4d", "c4f", "c4p", "c4u"]
      },
      "application/vnd.cluetrust.cartomobile-config": {
        source: "iana",
        extensions: ["c11amc"]
      },
      "application/vnd.cluetrust.cartomobile-config-pkg": {
        source: "iana",
        extensions: ["c11amz"]
      },
      "application/vnd.coffeescript": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.document": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.document-template": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.presentation": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.presentation-template": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.spreadsheet": {
        source: "iana"
      },
      "application/vnd.collabio.xodocuments.spreadsheet-template": {
        source: "iana"
      },
      "application/vnd.collection+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.collection.doc+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.collection.next+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.comicbook+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.comicbook-rar": {
        source: "iana"
      },
      "application/vnd.commerce-battelle": {
        source: "iana"
      },
      "application/vnd.commonspace": {
        source: "iana",
        extensions: ["csp"]
      },
      "application/vnd.contact.cmsg": {
        source: "iana",
        extensions: ["cdbcmsg"]
      },
      "application/vnd.coreos.ignition+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cosmocaller": {
        source: "iana",
        extensions: ["cmc"]
      },
      "application/vnd.crick.clicker": {
        source: "iana",
        extensions: ["clkx"]
      },
      "application/vnd.crick.clicker.keyboard": {
        source: "iana",
        extensions: ["clkk"]
      },
      "application/vnd.crick.clicker.palette": {
        source: "iana",
        extensions: ["clkp"]
      },
      "application/vnd.crick.clicker.template": {
        source: "iana",
        extensions: ["clkt"]
      },
      "application/vnd.crick.clicker.wordbank": {
        source: "iana",
        extensions: ["clkw"]
      },
      "application/vnd.criticaltools.wbs+xml": {
        source: "iana",
        compressible: true,
        extensions: ["wbs"]
      },
      "application/vnd.cryptii.pipe+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.crypto-shade-file": {
        source: "iana"
      },
      "application/vnd.cryptomator.encrypted": {
        source: "iana"
      },
      "application/vnd.cryptomator.vault": {
        source: "iana"
      },
      "application/vnd.ctc-posml": {
        source: "iana",
        extensions: ["pml"]
      },
      "application/vnd.ctct.ws+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cups-pdf": {
        source: "iana"
      },
      "application/vnd.cups-postscript": {
        source: "iana"
      },
      "application/vnd.cups-ppd": {
        source: "iana",
        extensions: ["ppd"]
      },
      "application/vnd.cups-raster": {
        source: "iana"
      },
      "application/vnd.cups-raw": {
        source: "iana"
      },
      "application/vnd.curl": {
        source: "iana"
      },
      "application/vnd.curl.car": {
        source: "apache",
        extensions: ["car"]
      },
      "application/vnd.curl.pcurl": {
        source: "apache",
        extensions: ["pcurl"]
      },
      "application/vnd.cyan.dean.root+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cybank": {
        source: "iana"
      },
      "application/vnd.cyclonedx+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.cyclonedx+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.d2l.coursepackage1p0+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.d3m-dataset": {
        source: "iana"
      },
      "application/vnd.d3m-problem": {
        source: "iana"
      },
      "application/vnd.dart": {
        source: "iana",
        compressible: true,
        extensions: ["dart"]
      },
      "application/vnd.data-vision.rdz": {
        source: "iana",
        extensions: ["rdz"]
      },
      "application/vnd.datapackage+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dataresource+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dbf": {
        source: "iana",
        extensions: ["dbf"]
      },
      "application/vnd.debian.binary-package": {
        source: "iana"
      },
      "application/vnd.dece.data": {
        source: "iana",
        extensions: ["uvf", "uvvf", "uvd", "uvvd"]
      },
      "application/vnd.dece.ttml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["uvt", "uvvt"]
      },
      "application/vnd.dece.unspecified": {
        source: "iana",
        extensions: ["uvx", "uvvx"]
      },
      "application/vnd.dece.zip": {
        source: "iana",
        extensions: ["uvz", "uvvz"]
      },
      "application/vnd.denovo.fcselayout-link": {
        source: "iana",
        extensions: ["fe_launch"]
      },
      "application/vnd.desmume.movie": {
        source: "iana"
      },
      "application/vnd.dir-bi.plate-dl-nosuffix": {
        source: "iana"
      },
      "application/vnd.dm.delegation+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dna": {
        source: "iana",
        extensions: ["dna"]
      },
      "application/vnd.document+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dolby.mlp": {
        source: "apache",
        extensions: ["mlp"]
      },
      "application/vnd.dolby.mobile.1": {
        source: "iana"
      },
      "application/vnd.dolby.mobile.2": {
        source: "iana"
      },
      "application/vnd.doremir.scorecloud-binary-document": {
        source: "iana"
      },
      "application/vnd.dpgraph": {
        source: "iana",
        extensions: ["dpg"]
      },
      "application/vnd.dreamfactory": {
        source: "iana",
        extensions: ["dfac"]
      },
      "application/vnd.drive+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ds-keypoint": {
        source: "apache",
        extensions: ["kpxx"]
      },
      "application/vnd.dtg.local": {
        source: "iana"
      },
      "application/vnd.dtg.local.flash": {
        source: "iana"
      },
      "application/vnd.dtg.local.html": {
        source: "iana"
      },
      "application/vnd.dvb.ait": {
        source: "iana",
        extensions: ["ait"]
      },
      "application/vnd.dvb.dvbisl+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.dvbj": {
        source: "iana"
      },
      "application/vnd.dvb.esgcontainer": {
        source: "iana"
      },
      "application/vnd.dvb.ipdcdftnotifaccess": {
        source: "iana"
      },
      "application/vnd.dvb.ipdcesgaccess": {
        source: "iana"
      },
      "application/vnd.dvb.ipdcesgaccess2": {
        source: "iana"
      },
      "application/vnd.dvb.ipdcesgpdd": {
        source: "iana"
      },
      "application/vnd.dvb.ipdcroaming": {
        source: "iana"
      },
      "application/vnd.dvb.iptv.alfec-base": {
        source: "iana"
      },
      "application/vnd.dvb.iptv.alfec-enhancement": {
        source: "iana"
      },
      "application/vnd.dvb.notif-aggregate-root+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-container+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-generic+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-ia-msglist+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-ia-registration-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-ia-registration-response+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.notif-init+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.dvb.pfr": {
        source: "iana"
      },
      "application/vnd.dvb.service": {
        source: "iana",
        extensions: ["svc"]
      },
      "application/vnd.dxr": {
        source: "iana"
      },
      "application/vnd.dynageo": {
        source: "iana",
        extensions: ["geo"]
      },
      "application/vnd.dzr": {
        source: "iana"
      },
      "application/vnd.easykaraoke.cdgdownload": {
        source: "iana"
      },
      "application/vnd.ecdis-update": {
        source: "iana"
      },
      "application/vnd.ecip.rlp": {
        source: "iana"
      },
      "application/vnd.eclipse.ditto+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ecowin.chart": {
        source: "iana",
        extensions: ["mag"]
      },
      "application/vnd.ecowin.filerequest": {
        source: "iana"
      },
      "application/vnd.ecowin.fileupdate": {
        source: "iana"
      },
      "application/vnd.ecowin.series": {
        source: "iana"
      },
      "application/vnd.ecowin.seriesrequest": {
        source: "iana"
      },
      "application/vnd.ecowin.seriesupdate": {
        source: "iana"
      },
      "application/vnd.efi.img": {
        source: "iana"
      },
      "application/vnd.efi.iso": {
        source: "iana"
      },
      "application/vnd.emclient.accessrequest+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.enliven": {
        source: "iana",
        extensions: ["nml"]
      },
      "application/vnd.enphase.envoy": {
        source: "iana"
      },
      "application/vnd.eprints.data+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.epson.esf": {
        source: "iana",
        extensions: ["esf"]
      },
      "application/vnd.epson.msf": {
        source: "iana",
        extensions: ["msf"]
      },
      "application/vnd.epson.quickanime": {
        source: "iana",
        extensions: ["qam"]
      },
      "application/vnd.epson.salt": {
        source: "iana",
        extensions: ["slt"]
      },
      "application/vnd.epson.ssf": {
        source: "iana",
        extensions: ["ssf"]
      },
      "application/vnd.ericsson.quickcall": {
        source: "iana"
      },
      "application/vnd.espass-espass+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.eszigno3+xml": {
        source: "iana",
        compressible: true,
        extensions: ["es3", "et3"]
      },
      "application/vnd.etsi.aoc+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.asic-e+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.etsi.asic-s+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.etsi.cug+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvcommand+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvdiscovery+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvprofile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvsad-bc+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvsad-cod+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvsad-npvr+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvservice+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvsync+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.iptvueprofile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.mcid+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.mheg5": {
        source: "iana"
      },
      "application/vnd.etsi.overload-control-policy-dataset+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.pstn+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.sci+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.simservs+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.timestamp-token": {
        source: "iana"
      },
      "application/vnd.etsi.tsl+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.etsi.tsl.der": {
        source: "iana"
      },
      "application/vnd.eu.kasparian.car+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.eudora.data": {
        source: "iana"
      },
      "application/vnd.evolv.ecig.profile": {
        source: "iana"
      },
      "application/vnd.evolv.ecig.settings": {
        source: "iana"
      },
      "application/vnd.evolv.ecig.theme": {
        source: "iana"
      },
      "application/vnd.exstream-empower+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.exstream-package": {
        source: "iana"
      },
      "application/vnd.ezpix-album": {
        source: "iana",
        extensions: ["ez2"]
      },
      "application/vnd.ezpix-package": {
        source: "iana",
        extensions: ["ez3"]
      },
      "application/vnd.f-secure.mobile": {
        source: "iana"
      },
      "application/vnd.familysearch.gedcom+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.fastcopy-disk-image": {
        source: "iana"
      },
      "application/vnd.fdf": {
        source: "iana",
        extensions: ["fdf"]
      },
      "application/vnd.fdsn.mseed": {
        source: "iana",
        extensions: ["mseed"]
      },
      "application/vnd.fdsn.seed": {
        source: "iana",
        extensions: ["seed", "dataless"]
      },
      "application/vnd.ffsns": {
        source: "iana"
      },
      "application/vnd.ficlab.flb+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.filmit.zfc": {
        source: "iana"
      },
      "application/vnd.fints": {
        source: "iana"
      },
      "application/vnd.firemonkeys.cloudcell": {
        source: "iana"
      },
      "application/vnd.flographit": {
        source: "iana",
        extensions: ["gph"]
      },
      "application/vnd.fluxtime.clip": {
        source: "iana",
        extensions: ["ftc"]
      },
      "application/vnd.font-fontforge-sfd": {
        source: "iana"
      },
      "application/vnd.framemaker": {
        source: "iana",
        extensions: ["fm", "frame", "maker", "book"]
      },
      "application/vnd.frogans.fnc": {
        source: "iana",
        extensions: ["fnc"]
      },
      "application/vnd.frogans.ltf": {
        source: "iana",
        extensions: ["ltf"]
      },
      "application/vnd.fsc.weblaunch": {
        source: "iana",
        extensions: ["fsc"]
      },
      "application/vnd.fujifilm.fb.docuworks": {
        source: "iana"
      },
      "application/vnd.fujifilm.fb.docuworks.binder": {
        source: "iana"
      },
      "application/vnd.fujifilm.fb.docuworks.container": {
        source: "iana"
      },
      "application/vnd.fujifilm.fb.jfi+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.fujitsu.oasys": {
        source: "iana",
        extensions: ["oas"]
      },
      "application/vnd.fujitsu.oasys2": {
        source: "iana",
        extensions: ["oa2"]
      },
      "application/vnd.fujitsu.oasys3": {
        source: "iana",
        extensions: ["oa3"]
      },
      "application/vnd.fujitsu.oasysgp": {
        source: "iana",
        extensions: ["fg5"]
      },
      "application/vnd.fujitsu.oasysprs": {
        source: "iana",
        extensions: ["bh2"]
      },
      "application/vnd.fujixerox.art-ex": {
        source: "iana"
      },
      "application/vnd.fujixerox.art4": {
        source: "iana"
      },
      "application/vnd.fujixerox.ddd": {
        source: "iana",
        extensions: ["ddd"]
      },
      "application/vnd.fujixerox.docuworks": {
        source: "iana",
        extensions: ["xdw"]
      },
      "application/vnd.fujixerox.docuworks.binder": {
        source: "iana",
        extensions: ["xbd"]
      },
      "application/vnd.fujixerox.docuworks.container": {
        source: "iana"
      },
      "application/vnd.fujixerox.hbpl": {
        source: "iana"
      },
      "application/vnd.fut-misnet": {
        source: "iana"
      },
      "application/vnd.futoin+cbor": {
        source: "iana"
      },
      "application/vnd.futoin+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.fuzzysheet": {
        source: "iana",
        extensions: ["fzs"]
      },
      "application/vnd.genomatix.tuxedo": {
        source: "iana",
        extensions: ["txd"]
      },
      "application/vnd.gentics.grd+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.geo+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.geocube+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.geogebra.file": {
        source: "iana",
        extensions: ["ggb"]
      },
      "application/vnd.geogebra.slides": {
        source: "iana"
      },
      "application/vnd.geogebra.tool": {
        source: "iana",
        extensions: ["ggt"]
      },
      "application/vnd.geometry-explorer": {
        source: "iana",
        extensions: ["gex", "gre"]
      },
      "application/vnd.geonext": {
        source: "iana",
        extensions: ["gxt"]
      },
      "application/vnd.geoplan": {
        source: "iana",
        extensions: ["g2w"]
      },
      "application/vnd.geospace": {
        source: "iana",
        extensions: ["g3w"]
      },
      "application/vnd.gerber": {
        source: "iana"
      },
      "application/vnd.globalplatform.card-content-mgt": {
        source: "iana"
      },
      "application/vnd.globalplatform.card-content-mgt-response": {
        source: "iana"
      },
      "application/vnd.gmx": {
        source: "iana",
        extensions: ["gmx"]
      },
      "application/vnd.google-apps.document": {
        compressible: false,
        extensions: ["gdoc"]
      },
      "application/vnd.google-apps.presentation": {
        compressible: false,
        extensions: ["gslides"]
      },
      "application/vnd.google-apps.spreadsheet": {
        compressible: false,
        extensions: ["gsheet"]
      },
      "application/vnd.google-earth.kml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["kml"]
      },
      "application/vnd.google-earth.kmz": {
        source: "iana",
        compressible: false,
        extensions: ["kmz"]
      },
      "application/vnd.gov.sk.e-form+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.gov.sk.e-form+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.gov.sk.xmldatacontainer+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.grafeq": {
        source: "iana",
        extensions: ["gqf", "gqs"]
      },
      "application/vnd.gridmp": {
        source: "iana"
      },
      "application/vnd.groove-account": {
        source: "iana",
        extensions: ["gac"]
      },
      "application/vnd.groove-help": {
        source: "iana",
        extensions: ["ghf"]
      },
      "application/vnd.groove-identity-message": {
        source: "iana",
        extensions: ["gim"]
      },
      "application/vnd.groove-injector": {
        source: "iana",
        extensions: ["grv"]
      },
      "application/vnd.groove-tool-message": {
        source: "iana",
        extensions: ["gtm"]
      },
      "application/vnd.groove-tool-template": {
        source: "iana",
        extensions: ["tpl"]
      },
      "application/vnd.groove-vcard": {
        source: "iana",
        extensions: ["vcg"]
      },
      "application/vnd.hal+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hal+xml": {
        source: "iana",
        compressible: true,
        extensions: ["hal"]
      },
      "application/vnd.handheld-entertainment+xml": {
        source: "iana",
        compressible: true,
        extensions: ["zmm"]
      },
      "application/vnd.hbci": {
        source: "iana",
        extensions: ["hbci"]
      },
      "application/vnd.hc+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hcl-bireports": {
        source: "iana"
      },
      "application/vnd.hdt": {
        source: "iana"
      },
      "application/vnd.heroku+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hhe.lesson-player": {
        source: "iana",
        extensions: ["les"]
      },
      "application/vnd.hl7cda+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.hl7v2+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.hp-hpgl": {
        source: "iana",
        extensions: ["hpgl"]
      },
      "application/vnd.hp-hpid": {
        source: "iana",
        extensions: ["hpid"]
      },
      "application/vnd.hp-hps": {
        source: "iana",
        extensions: ["hps"]
      },
      "application/vnd.hp-jlyt": {
        source: "iana",
        extensions: ["jlt"]
      },
      "application/vnd.hp-pcl": {
        source: "iana",
        extensions: ["pcl"]
      },
      "application/vnd.hp-pclxl": {
        source: "iana",
        extensions: ["pclxl"]
      },
      "application/vnd.httphone": {
        source: "iana"
      },
      "application/vnd.hydrostatix.sof-data": {
        source: "iana",
        extensions: ["sfd-hdstx"]
      },
      "application/vnd.hyper+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hyper-item+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hyperdrive+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.hzn-3d-crossword": {
        source: "iana"
      },
      "application/vnd.ibm.afplinedata": {
        source: "iana"
      },
      "application/vnd.ibm.electronic-media": {
        source: "iana"
      },
      "application/vnd.ibm.minipay": {
        source: "iana",
        extensions: ["mpy"]
      },
      "application/vnd.ibm.modcap": {
        source: "iana",
        extensions: ["afp", "listafp", "list3820"]
      },
      "application/vnd.ibm.rights-management": {
        source: "iana",
        extensions: ["irm"]
      },
      "application/vnd.ibm.secure-container": {
        source: "iana",
        extensions: ["sc"]
      },
      "application/vnd.iccprofile": {
        source: "iana",
        extensions: ["icc", "icm"]
      },
      "application/vnd.ieee.1905": {
        source: "iana"
      },
      "application/vnd.igloader": {
        source: "iana",
        extensions: ["igl"]
      },
      "application/vnd.imagemeter.folder+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.imagemeter.image+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.immervision-ivp": {
        source: "iana",
        extensions: ["ivp"]
      },
      "application/vnd.immervision-ivu": {
        source: "iana",
        extensions: ["ivu"]
      },
      "application/vnd.ims.imsccv1p1": {
        source: "iana"
      },
      "application/vnd.ims.imsccv1p2": {
        source: "iana"
      },
      "application/vnd.ims.imsccv1p3": {
        source: "iana"
      },
      "application/vnd.ims.lis.v2.result+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ims.lti.v2.toolconsumerprofile+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ims.lti.v2.toolproxy+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ims.lti.v2.toolproxy.id+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ims.lti.v2.toolsettings+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ims.lti.v2.toolsettings.simple+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.informedcontrol.rms+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.informix-visionary": {
        source: "iana"
      },
      "application/vnd.infotech.project": {
        source: "iana"
      },
      "application/vnd.infotech.project+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.innopath.wamp.notification": {
        source: "iana"
      },
      "application/vnd.insors.igm": {
        source: "iana",
        extensions: ["igm"]
      },
      "application/vnd.intercon.formnet": {
        source: "iana",
        extensions: ["xpw", "xpx"]
      },
      "application/vnd.intergeo": {
        source: "iana",
        extensions: ["i2g"]
      },
      "application/vnd.intertrust.digibox": {
        source: "iana"
      },
      "application/vnd.intertrust.nncp": {
        source: "iana"
      },
      "application/vnd.intu.qbo": {
        source: "iana",
        extensions: ["qbo"]
      },
      "application/vnd.intu.qfx": {
        source: "iana",
        extensions: ["qfx"]
      },
      "application/vnd.iptc.g2.catalogitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.conceptitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.knowledgeitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.newsitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.newsmessage+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.packageitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.iptc.g2.planningitem+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ipunplugged.rcprofile": {
        source: "iana",
        extensions: ["rcprofile"]
      },
      "application/vnd.irepository.package+xml": {
        source: "iana",
        compressible: true,
        extensions: ["irp"]
      },
      "application/vnd.is-xpr": {
        source: "iana",
        extensions: ["xpr"]
      },
      "application/vnd.isac.fcs": {
        source: "iana",
        extensions: ["fcs"]
      },
      "application/vnd.iso11783-10+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.jam": {
        source: "iana",
        extensions: ["jam"]
      },
      "application/vnd.japannet-directory-service": {
        source: "iana"
      },
      "application/vnd.japannet-jpnstore-wakeup": {
        source: "iana"
      },
      "application/vnd.japannet-payment-wakeup": {
        source: "iana"
      },
      "application/vnd.japannet-registration": {
        source: "iana"
      },
      "application/vnd.japannet-registration-wakeup": {
        source: "iana"
      },
      "application/vnd.japannet-setstore-wakeup": {
        source: "iana"
      },
      "application/vnd.japannet-verification": {
        source: "iana"
      },
      "application/vnd.japannet-verification-wakeup": {
        source: "iana"
      },
      "application/vnd.jcp.javame.midlet-rms": {
        source: "iana",
        extensions: ["rms"]
      },
      "application/vnd.jisp": {
        source: "iana",
        extensions: ["jisp"]
      },
      "application/vnd.joost.joda-archive": {
        source: "iana",
        extensions: ["joda"]
      },
      "application/vnd.jsk.isdn-ngn": {
        source: "iana"
      },
      "application/vnd.kahootz": {
        source: "iana",
        extensions: ["ktz", "ktr"]
      },
      "application/vnd.kde.karbon": {
        source: "iana",
        extensions: ["karbon"]
      },
      "application/vnd.kde.kchart": {
        source: "iana",
        extensions: ["chrt"]
      },
      "application/vnd.kde.kformula": {
        source: "iana",
        extensions: ["kfo"]
      },
      "application/vnd.kde.kivio": {
        source: "iana",
        extensions: ["flw"]
      },
      "application/vnd.kde.kontour": {
        source: "iana",
        extensions: ["kon"]
      },
      "application/vnd.kde.kpresenter": {
        source: "iana",
        extensions: ["kpr", "kpt"]
      },
      "application/vnd.kde.kspread": {
        source: "iana",
        extensions: ["ksp"]
      },
      "application/vnd.kde.kword": {
        source: "iana",
        extensions: ["kwd", "kwt"]
      },
      "application/vnd.kenameaapp": {
        source: "iana",
        extensions: ["htke"]
      },
      "application/vnd.kidspiration": {
        source: "iana",
        extensions: ["kia"]
      },
      "application/vnd.kinar": {
        source: "iana",
        extensions: ["kne", "knp"]
      },
      "application/vnd.koan": {
        source: "iana",
        extensions: ["skp", "skd", "skt", "skm"]
      },
      "application/vnd.kodak-descriptor": {
        source: "iana",
        extensions: ["sse"]
      },
      "application/vnd.las": {
        source: "iana"
      },
      "application/vnd.las.las+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.las.las+xml": {
        source: "iana",
        compressible: true,
        extensions: ["lasxml"]
      },
      "application/vnd.laszip": {
        source: "iana"
      },
      "application/vnd.leap+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.liberty-request+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.llamagraphics.life-balance.desktop": {
        source: "iana",
        extensions: ["lbd"]
      },
      "application/vnd.llamagraphics.life-balance.exchange+xml": {
        source: "iana",
        compressible: true,
        extensions: ["lbe"]
      },
      "application/vnd.logipipe.circuit+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.loom": {
        source: "iana"
      },
      "application/vnd.lotus-1-2-3": {
        source: "iana",
        extensions: ["123"]
      },
      "application/vnd.lotus-approach": {
        source: "iana",
        extensions: ["apr"]
      },
      "application/vnd.lotus-freelance": {
        source: "iana",
        extensions: ["pre"]
      },
      "application/vnd.lotus-notes": {
        source: "iana",
        extensions: ["nsf"]
      },
      "application/vnd.lotus-organizer": {
        source: "iana",
        extensions: ["org"]
      },
      "application/vnd.lotus-screencam": {
        source: "iana",
        extensions: ["scm"]
      },
      "application/vnd.lotus-wordpro": {
        source: "iana",
        extensions: ["lwp"]
      },
      "application/vnd.macports.portpkg": {
        source: "iana",
        extensions: ["portpkg"]
      },
      "application/vnd.mapbox-vector-tile": {
        source: "iana",
        extensions: ["mvt"]
      },
      "application/vnd.marlin.drm.actiontoken+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.marlin.drm.conftoken+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.marlin.drm.license+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.marlin.drm.mdcf": {
        source: "iana"
      },
      "application/vnd.mason+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.maxar.archive.3tz+zip": {
        source: "iana",
        compressible: false
      },
      "application/vnd.maxmind.maxmind-db": {
        source: "iana"
      },
      "application/vnd.mcd": {
        source: "iana",
        extensions: ["mcd"]
      },
      "application/vnd.medcalcdata": {
        source: "iana",
        extensions: ["mc1"]
      },
      "application/vnd.mediastation.cdkey": {
        source: "iana",
        extensions: ["cdkey"]
      },
      "application/vnd.meridian-slingshot": {
        source: "iana"
      },
      "application/vnd.mfer": {
        source: "iana",
        extensions: ["mwf"]
      },
      "application/vnd.mfmp": {
        source: "iana",
        extensions: ["mfm"]
      },
      "application/vnd.micro+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.micrografx.flo": {
        source: "iana",
        extensions: ["flo"]
      },
      "application/vnd.micrografx.igx": {
        source: "iana",
        extensions: ["igx"]
      },
      "application/vnd.microsoft.portable-executable": {
        source: "iana"
      },
      "application/vnd.microsoft.windows.thumbnail-cache": {
        source: "iana"
      },
      "application/vnd.miele+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.mif": {
        source: "iana",
        extensions: ["mif"]
      },
      "application/vnd.minisoft-hp3000-save": {
        source: "iana"
      },
      "application/vnd.mitsubishi.misty-guard.trustweb": {
        source: "iana"
      },
      "application/vnd.mobius.daf": {
        source: "iana",
        extensions: ["daf"]
      },
      "application/vnd.mobius.dis": {
        source: "iana",
        extensions: ["dis"]
      },
      "application/vnd.mobius.mbk": {
        source: "iana",
        extensions: ["mbk"]
      },
      "application/vnd.mobius.mqy": {
        source: "iana",
        extensions: ["mqy"]
      },
      "application/vnd.mobius.msl": {
        source: "iana",
        extensions: ["msl"]
      },
      "application/vnd.mobius.plc": {
        source: "iana",
        extensions: ["plc"]
      },
      "application/vnd.mobius.txf": {
        source: "iana",
        extensions: ["txf"]
      },
      "application/vnd.mophun.application": {
        source: "iana",
        extensions: ["mpn"]
      },
      "application/vnd.mophun.certificate": {
        source: "iana",
        extensions: ["mpc"]
      },
      "application/vnd.motorola.flexsuite": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.adsi": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.fis": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.gotap": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.kmr": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.ttc": {
        source: "iana"
      },
      "application/vnd.motorola.flexsuite.wem": {
        source: "iana"
      },
      "application/vnd.motorola.iprm": {
        source: "iana"
      },
      "application/vnd.mozilla.xul+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xul"]
      },
      "application/vnd.ms-3mfdocument": {
        source: "iana"
      },
      "application/vnd.ms-artgalry": {
        source: "iana",
        extensions: ["cil"]
      },
      "application/vnd.ms-asf": {
        source: "iana"
      },
      "application/vnd.ms-cab-compressed": {
        source: "iana",
        extensions: ["cab"]
      },
      "application/vnd.ms-color.iccprofile": {
        source: "apache"
      },
      "application/vnd.ms-excel": {
        source: "iana",
        compressible: false,
        extensions: ["xls", "xlm", "xla", "xlc", "xlt", "xlw"]
      },
      "application/vnd.ms-excel.addin.macroenabled.12": {
        source: "iana",
        extensions: ["xlam"]
      },
      "application/vnd.ms-excel.sheet.binary.macroenabled.12": {
        source: "iana",
        extensions: ["xlsb"]
      },
      "application/vnd.ms-excel.sheet.macroenabled.12": {
        source: "iana",
        extensions: ["xlsm"]
      },
      "application/vnd.ms-excel.template.macroenabled.12": {
        source: "iana",
        extensions: ["xltm"]
      },
      "application/vnd.ms-fontobject": {
        source: "iana",
        compressible: true,
        extensions: ["eot"]
      },
      "application/vnd.ms-htmlhelp": {
        source: "iana",
        extensions: ["chm"]
      },
      "application/vnd.ms-ims": {
        source: "iana",
        extensions: ["ims"]
      },
      "application/vnd.ms-lrm": {
        source: "iana",
        extensions: ["lrm"]
      },
      "application/vnd.ms-office.activex+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ms-officetheme": {
        source: "iana",
        extensions: ["thmx"]
      },
      "application/vnd.ms-opentype": {
        source: "apache",
        compressible: true
      },
      "application/vnd.ms-outlook": {
        compressible: false,
        extensions: ["msg"]
      },
      "application/vnd.ms-package.obfuscated-opentype": {
        source: "apache"
      },
      "application/vnd.ms-pki.seccat": {
        source: "apache",
        extensions: ["cat"]
      },
      "application/vnd.ms-pki.stl": {
        source: "apache",
        extensions: ["stl"]
      },
      "application/vnd.ms-playready.initiator+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ms-powerpoint": {
        source: "iana",
        compressible: false,
        extensions: ["ppt", "pps", "pot"]
      },
      "application/vnd.ms-powerpoint.addin.macroenabled.12": {
        source: "iana",
        extensions: ["ppam"]
      },
      "application/vnd.ms-powerpoint.presentation.macroenabled.12": {
        source: "iana",
        extensions: ["pptm"]
      },
      "application/vnd.ms-powerpoint.slide.macroenabled.12": {
        source: "iana",
        extensions: ["sldm"]
      },
      "application/vnd.ms-powerpoint.slideshow.macroenabled.12": {
        source: "iana",
        extensions: ["ppsm"]
      },
      "application/vnd.ms-powerpoint.template.macroenabled.12": {
        source: "iana",
        extensions: ["potm"]
      },
      "application/vnd.ms-printdevicecapabilities+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ms-printing.printticket+xml": {
        source: "apache",
        compressible: true
      },
      "application/vnd.ms-printschematicket+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ms-project": {
        source: "iana",
        extensions: ["mpp", "mpt"]
      },
      "application/vnd.ms-tnef": {
        source: "iana"
      },
      "application/vnd.ms-windows.devicepairing": {
        source: "iana"
      },
      "application/vnd.ms-windows.nwprinting.oob": {
        source: "iana"
      },
      "application/vnd.ms-windows.printerpairing": {
        source: "iana"
      },
      "application/vnd.ms-windows.wsd.oob": {
        source: "iana"
      },
      "application/vnd.ms-wmdrm.lic-chlg-req": {
        source: "iana"
      },
      "application/vnd.ms-wmdrm.lic-resp": {
        source: "iana"
      },
      "application/vnd.ms-wmdrm.meter-chlg-req": {
        source: "iana"
      },
      "application/vnd.ms-wmdrm.meter-resp": {
        source: "iana"
      },
      "application/vnd.ms-word.document.macroenabled.12": {
        source: "iana",
        extensions: ["docm"]
      },
      "application/vnd.ms-word.template.macroenabled.12": {
        source: "iana",
        extensions: ["dotm"]
      },
      "application/vnd.ms-works": {
        source: "iana",
        extensions: ["wps", "wks", "wcm", "wdb"]
      },
      "application/vnd.ms-wpl": {
        source: "iana",
        extensions: ["wpl"]
      },
      "application/vnd.ms-xpsdocument": {
        source: "iana",
        compressible: false,
        extensions: ["xps"]
      },
      "application/vnd.msa-disk-image": {
        source: "iana"
      },
      "application/vnd.mseq": {
        source: "iana",
        extensions: ["mseq"]
      },
      "application/vnd.msign": {
        source: "iana"
      },
      "application/vnd.multiad.creator": {
        source: "iana"
      },
      "application/vnd.multiad.creator.cif": {
        source: "iana"
      },
      "application/vnd.music-niff": {
        source: "iana"
      },
      "application/vnd.musician": {
        source: "iana",
        extensions: ["mus"]
      },
      "application/vnd.muvee.style": {
        source: "iana",
        extensions: ["msty"]
      },
      "application/vnd.mynfc": {
        source: "iana",
        extensions: ["taglet"]
      },
      "application/vnd.nacamar.ybrid+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.ncd.control": {
        source: "iana"
      },
      "application/vnd.ncd.reference": {
        source: "iana"
      },
      "application/vnd.nearst.inv+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nebumind.line": {
        source: "iana"
      },
      "application/vnd.nervana": {
        source: "iana"
      },
      "application/vnd.netfpx": {
        source: "iana"
      },
      "application/vnd.neurolanguage.nlu": {
        source: "iana",
        extensions: ["nlu"]
      },
      "application/vnd.nimn": {
        source: "iana"
      },
      "application/vnd.nintendo.nitro.rom": {
        source: "iana"
      },
      "application/vnd.nintendo.snes.rom": {
        source: "iana"
      },
      "application/vnd.nitf": {
        source: "iana",
        extensions: ["ntf", "nitf"]
      },
      "application/vnd.noblenet-directory": {
        source: "iana",
        extensions: ["nnd"]
      },
      "application/vnd.noblenet-sealer": {
        source: "iana",
        extensions: ["nns"]
      },
      "application/vnd.noblenet-web": {
        source: "iana",
        extensions: ["nnw"]
      },
      "application/vnd.nokia.catalogs": {
        source: "iana"
      },
      "application/vnd.nokia.conml+wbxml": {
        source: "iana"
      },
      "application/vnd.nokia.conml+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nokia.iptv.config+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nokia.isds-radio-presets": {
        source: "iana"
      },
      "application/vnd.nokia.landmark+wbxml": {
        source: "iana"
      },
      "application/vnd.nokia.landmark+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nokia.landmarkcollection+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nokia.n-gage.ac+xml": {
        source: "iana",
        compressible: true,
        extensions: ["ac"]
      },
      "application/vnd.nokia.n-gage.data": {
        source: "iana",
        extensions: ["ngdat"]
      },
      "application/vnd.nokia.n-gage.symbian.install": {
        source: "iana",
        extensions: ["n-gage"]
      },
      "application/vnd.nokia.ncd": {
        source: "iana"
      },
      "application/vnd.nokia.pcd+wbxml": {
        source: "iana"
      },
      "application/vnd.nokia.pcd+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.nokia.radio-preset": {
        source: "iana",
        extensions: ["rpst"]
      },
      "application/vnd.nokia.radio-presets": {
        source: "iana",
        extensions: ["rpss"]
      },
      "application/vnd.novadigm.edm": {
        source: "iana",
        extensions: ["edm"]
      },
      "application/vnd.novadigm.edx": {
        source: "iana",
        extensions: ["edx"]
      },
      "application/vnd.novadigm.ext": {
        source: "iana",
        extensions: ["ext"]
      },
      "application/vnd.ntt-local.content-share": {
        source: "iana"
      },
      "application/vnd.ntt-local.file-transfer": {
        source: "iana"
      },
      "application/vnd.ntt-local.ogw_remote-access": {
        source: "iana"
      },
      "application/vnd.ntt-local.sip-ta_remote": {
        source: "iana"
      },
      "application/vnd.ntt-local.sip-ta_tcp_stream": {
        source: "iana"
      },
      "application/vnd.oasis.opendocument.chart": {
        source: "iana",
        extensions: ["odc"]
      },
      "application/vnd.oasis.opendocument.chart-template": {
        source: "iana",
        extensions: ["otc"]
      },
      "application/vnd.oasis.opendocument.database": {
        source: "iana",
        extensions: ["odb"]
      },
      "application/vnd.oasis.opendocument.formula": {
        source: "iana",
        extensions: ["odf"]
      },
      "application/vnd.oasis.opendocument.formula-template": {
        source: "iana",
        extensions: ["odft"]
      },
      "application/vnd.oasis.opendocument.graphics": {
        source: "iana",
        compressible: false,
        extensions: ["odg"]
      },
      "application/vnd.oasis.opendocument.graphics-template": {
        source: "iana",
        extensions: ["otg"]
      },
      "application/vnd.oasis.opendocument.image": {
        source: "iana",
        extensions: ["odi"]
      },
      "application/vnd.oasis.opendocument.image-template": {
        source: "iana",
        extensions: ["oti"]
      },
      "application/vnd.oasis.opendocument.presentation": {
        source: "iana",
        compressible: false,
        extensions: ["odp"]
      },
      "application/vnd.oasis.opendocument.presentation-template": {
        source: "iana",
        extensions: ["otp"]
      },
      "application/vnd.oasis.opendocument.spreadsheet": {
        source: "iana",
        compressible: false,
        extensions: ["ods"]
      },
      "application/vnd.oasis.opendocument.spreadsheet-template": {
        source: "iana",
        extensions: ["ots"]
      },
      "application/vnd.oasis.opendocument.text": {
        source: "iana",
        compressible: false,
        extensions: ["odt"]
      },
      "application/vnd.oasis.opendocument.text-master": {
        source: "iana",
        extensions: ["odm"]
      },
      "application/vnd.oasis.opendocument.text-template": {
        source: "iana",
        extensions: ["ott"]
      },
      "application/vnd.oasis.opendocument.text-web": {
        source: "iana",
        extensions: ["oth"]
      },
      "application/vnd.obn": {
        source: "iana"
      },
      "application/vnd.ocf+cbor": {
        source: "iana"
      },
      "application/vnd.oci.image.manifest.v1+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oftn.l10n+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.contentaccessdownload+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.contentaccessstreaming+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.cspg-hexbinary": {
        source: "iana"
      },
      "application/vnd.oipf.dae.svg+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.dae.xhtml+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.mippvcontrolmessage+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.pae.gem": {
        source: "iana"
      },
      "application/vnd.oipf.spdiscovery+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.spdlist+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.ueprofile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oipf.userprofile+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.olpc-sugar": {
        source: "iana",
        extensions: ["xo"]
      },
      "application/vnd.oma-scws-config": {
        source: "iana"
      },
      "application/vnd.oma-scws-http-request": {
        source: "iana"
      },
      "application/vnd.oma-scws-http-response": {
        source: "iana"
      },
      "application/vnd.oma.bcast.associated-procedure-parameter+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.drm-trigger+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.imd+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.ltkm": {
        source: "iana"
      },
      "application/vnd.oma.bcast.notification+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.provisioningtrigger": {
        source: "iana"
      },
      "application/vnd.oma.bcast.sgboot": {
        source: "iana"
      },
      "application/vnd.oma.bcast.sgdd+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.sgdu": {
        source: "iana"
      },
      "application/vnd.oma.bcast.simple-symbol-container": {
        source: "iana"
      },
      "application/vnd.oma.bcast.smartcard-trigger+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.sprov+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.bcast.stkm": {
        source: "iana"
      },
      "application/vnd.oma.cab-address-book+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.cab-feature-handler+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.cab-pcc+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.cab-subs-invite+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.cab-user-prefs+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.dcd": {
        source: "iana"
      },
      "application/vnd.oma.dcdc": {
        source: "iana"
      },
      "application/vnd.oma.dd2+xml": {
        source: "iana",
        compressible: true,
        extensions: ["dd2"]
      },
      "application/vnd.oma.drm.risd+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.group-usage-list+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.lwm2m+cbor": {
        source: "iana"
      },
      "application/vnd.oma.lwm2m+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.lwm2m+tlv": {
        source: "iana"
      },
      "application/vnd.oma.pal+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.poc.detailed-progress-report+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.poc.final-report+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.poc.groups+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.poc.invocation-descriptor+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.poc.optimized-progress-report+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.push": {
        source: "iana"
      },
      "application/vnd.oma.scidm.messages+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oma.xcap-directory+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.omads-email+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.omads-file+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.omads-folder+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.omaloc-supl-init": {
        source: "iana"
      },
      "application/vnd.onepager": {
        source: "iana"
      },
      "application/vnd.onepagertamp": {
        source: "iana"
      },
      "application/vnd.onepagertamx": {
        source: "iana"
      },
      "application/vnd.onepagertat": {
        source: "iana"
      },
      "application/vnd.onepagertatp": {
        source: "iana"
      },
      "application/vnd.onepagertatx": {
        source: "iana"
      },
      "application/vnd.openblox.game+xml": {
        source: "iana",
        compressible: true,
        extensions: ["obgx"]
      },
      "application/vnd.openblox.game-binary": {
        source: "iana"
      },
      "application/vnd.openeye.oeb": {
        source: "iana"
      },
      "application/vnd.openofficeorg.extension": {
        source: "apache",
        extensions: ["oxt"]
      },
      "application/vnd.openstreetmap.data+xml": {
        source: "iana",
        compressible: true,
        extensions: ["osm"]
      },
      "application/vnd.opentimestamps.ots": {
        source: "iana"
      },
      "application/vnd.openxmlformats-officedocument.custom-properties+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.customxmlproperties+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawing+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.chart+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.chartshapes+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.diagramcolors+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.diagramdata+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.diagramlayout+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.drawingml.diagramstyle+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.extended-properties+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.commentauthors+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.comments+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.handoutmaster+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.notesmaster+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.notesslide+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.presentation": {
        source: "iana",
        compressible: false,
        extensions: ["pptx"]
      },
      "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.presprops+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slide": {
        source: "iana",
        extensions: ["sldx"]
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slide+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slidelayout+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slidemaster+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slideshow": {
        source: "iana",
        extensions: ["ppsx"]
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.slideupdateinfo+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.tablestyles+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.tags+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.template": {
        source: "iana",
        extensions: ["potx"]
      },
      "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.presentationml.viewprops+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.calcchain+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.dialogsheet+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.externallink+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotcachedefinition+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotcacherecords+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.pivottable+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.querytable+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.revisionheaders+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.revisionlog+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedstrings+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": {
        source: "iana",
        compressible: false,
        extensions: ["xlsx"]
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetmetadata+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.tablesinglecells+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.template": {
        source: "iana",
        extensions: ["xltx"]
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.usernames+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.volatiledependencies+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.theme+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.themeoverride+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.vmldrawing": {
        source: "iana"
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document": {
        source: "iana",
        compressible: false,
        extensions: ["docx"]
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document.glossary+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.fonttable+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.template": {
        source: "iana",
        extensions: ["dotx"]
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-officedocument.wordprocessingml.websettings+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-package.core-properties+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.openxmlformats-package.relationships+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oracle.resource+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.orange.indata": {
        source: "iana"
      },
      "application/vnd.osa.netdeploy": {
        source: "iana"
      },
      "application/vnd.osgeo.mapguide.package": {
        source: "iana",
        extensions: ["mgp"]
      },
      "application/vnd.osgi.bundle": {
        source: "iana"
      },
      "application/vnd.osgi.dp": {
        source: "iana",
        extensions: ["dp"]
      },
      "application/vnd.osgi.subsystem": {
        source: "iana",
        extensions: ["esa"]
      },
      "application/vnd.otps.ct-kip+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.oxli.countgraph": {
        source: "iana"
      },
      "application/vnd.pagerduty+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.palm": {
        source: "iana",
        extensions: ["pdb", "pqa", "oprc"]
      },
      "application/vnd.panoply": {
        source: "iana"
      },
      "application/vnd.paos.xml": {
        source: "iana"
      },
      "application/vnd.patentdive": {
        source: "iana"
      },
      "application/vnd.patientecommsdoc": {
        source: "iana"
      },
      "application/vnd.pawaafile": {
        source: "iana",
        extensions: ["paw"]
      },
      "application/vnd.pcos": {
        source: "iana"
      },
      "application/vnd.pg.format": {
        source: "iana",
        extensions: ["str"]
      },
      "application/vnd.pg.osasli": {
        source: "iana",
        extensions: ["ei6"]
      },
      "application/vnd.piaccess.application-licence": {
        source: "iana"
      },
      "application/vnd.picsel": {
        source: "iana",
        extensions: ["efif"]
      },
      "application/vnd.pmi.widget": {
        source: "iana",
        extensions: ["wg"]
      },
      "application/vnd.poc.group-advertisement+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.pocketlearn": {
        source: "iana",
        extensions: ["plf"]
      },
      "application/vnd.powerbuilder6": {
        source: "iana",
        extensions: ["pbd"]
      },
      "application/vnd.powerbuilder6-s": {
        source: "iana"
      },
      "application/vnd.powerbuilder7": {
        source: "iana"
      },
      "application/vnd.powerbuilder7-s": {
        source: "iana"
      },
      "application/vnd.powerbuilder75": {
        source: "iana"
      },
      "application/vnd.powerbuilder75-s": {
        source: "iana"
      },
      "application/vnd.preminet": {
        source: "iana"
      },
      "application/vnd.previewsystems.box": {
        source: "iana",
        extensions: ["box"]
      },
      "application/vnd.proteus.magazine": {
        source: "iana",
        extensions: ["mgz"]
      },
      "application/vnd.psfs": {
        source: "iana"
      },
      "application/vnd.publishare-delta-tree": {
        source: "iana",
        extensions: ["qps"]
      },
      "application/vnd.pvi.ptid1": {
        source: "iana",
        extensions: ["ptid"]
      },
      "application/vnd.pwg-multiplexed": {
        source: "iana"
      },
      "application/vnd.pwg-xhtml-print+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.qualcomm.brew-app-res": {
        source: "iana"
      },
      "application/vnd.quarantainenet": {
        source: "iana"
      },
      "application/vnd.quark.quarkxpress": {
        source: "iana",
        extensions: ["qxd", "qxt", "qwd", "qwt", "qxl", "qxb"]
      },
      "application/vnd.quobject-quoxdocument": {
        source: "iana"
      },
      "application/vnd.radisys.moml+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-audit+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-audit-conf+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-audit-conn+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-audit-dialog+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-audit-stream+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-conf+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-base+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-fax-detect+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-fax-sendrecv+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-group+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-speech+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.radisys.msml-dialog-transform+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.rainstor.data": {
        source: "iana"
      },
      "application/vnd.rapid": {
        source: "iana"
      },
      "application/vnd.rar": {
        source: "iana",
        extensions: ["rar"]
      },
      "application/vnd.realvnc.bed": {
        source: "iana",
        extensions: ["bed"]
      },
      "application/vnd.recordare.musicxml": {
        source: "iana",
        extensions: ["mxl"]
      },
      "application/vnd.recordare.musicxml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["musicxml"]
      },
      "application/vnd.renlearn.rlprint": {
        source: "iana"
      },
      "application/vnd.resilient.logic": {
        source: "iana"
      },
      "application/vnd.restful+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.rig.cryptonote": {
        source: "iana",
        extensions: ["cryptonote"]
      },
      "application/vnd.rim.cod": {
        source: "apache",
        extensions: ["cod"]
      },
      "application/vnd.rn-realmedia": {
        source: "apache",
        extensions: ["rm"]
      },
      "application/vnd.rn-realmedia-vbr": {
        source: "apache",
        extensions: ["rmvb"]
      },
      "application/vnd.route66.link66+xml": {
        source: "iana",
        compressible: true,
        extensions: ["link66"]
      },
      "application/vnd.rs-274x": {
        source: "iana"
      },
      "application/vnd.ruckus.download": {
        source: "iana"
      },
      "application/vnd.s3sms": {
        source: "iana"
      },
      "application/vnd.sailingtracker.track": {
        source: "iana",
        extensions: ["st"]
      },
      "application/vnd.sar": {
        source: "iana"
      },
      "application/vnd.sbm.cid": {
        source: "iana"
      },
      "application/vnd.sbm.mid2": {
        source: "iana"
      },
      "application/vnd.scribus": {
        source: "iana"
      },
      "application/vnd.sealed.3df": {
        source: "iana"
      },
      "application/vnd.sealed.csf": {
        source: "iana"
      },
      "application/vnd.sealed.doc": {
        source: "iana"
      },
      "application/vnd.sealed.eml": {
        source: "iana"
      },
      "application/vnd.sealed.mht": {
        source: "iana"
      },
      "application/vnd.sealed.net": {
        source: "iana"
      },
      "application/vnd.sealed.ppt": {
        source: "iana"
      },
      "application/vnd.sealed.tiff": {
        source: "iana"
      },
      "application/vnd.sealed.xls": {
        source: "iana"
      },
      "application/vnd.sealedmedia.softseal.html": {
        source: "iana"
      },
      "application/vnd.sealedmedia.softseal.pdf": {
        source: "iana"
      },
      "application/vnd.seemail": {
        source: "iana",
        extensions: ["see"]
      },
      "application/vnd.seis+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.sema": {
        source: "iana",
        extensions: ["sema"]
      },
      "application/vnd.semd": {
        source: "iana",
        extensions: ["semd"]
      },
      "application/vnd.semf": {
        source: "iana",
        extensions: ["semf"]
      },
      "application/vnd.shade-save-file": {
        source: "iana"
      },
      "application/vnd.shana.informed.formdata": {
        source: "iana",
        extensions: ["ifm"]
      },
      "application/vnd.shana.informed.formtemplate": {
        source: "iana",
        extensions: ["itp"]
      },
      "application/vnd.shana.informed.interchange": {
        source: "iana",
        extensions: ["iif"]
      },
      "application/vnd.shana.informed.package": {
        source: "iana",
        extensions: ["ipk"]
      },
      "application/vnd.shootproof+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.shopkick+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.shp": {
        source: "iana"
      },
      "application/vnd.shx": {
        source: "iana"
      },
      "application/vnd.sigrok.session": {
        source: "iana"
      },
      "application/vnd.simtech-mindmapper": {
        source: "iana",
        extensions: ["twd", "twds"]
      },
      "application/vnd.siren+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.smaf": {
        source: "iana",
        extensions: ["mmf"]
      },
      "application/vnd.smart.notebook": {
        source: "iana"
      },
      "application/vnd.smart.teacher": {
        source: "iana",
        extensions: ["teacher"]
      },
      "application/vnd.snesdev-page-table": {
        source: "iana"
      },
      "application/vnd.software602.filler.form+xml": {
        source: "iana",
        compressible: true,
        extensions: ["fo"]
      },
      "application/vnd.software602.filler.form-xml-zip": {
        source: "iana"
      },
      "application/vnd.solent.sdkm+xml": {
        source: "iana",
        compressible: true,
        extensions: ["sdkm", "sdkd"]
      },
      "application/vnd.spotfire.dxp": {
        source: "iana",
        extensions: ["dxp"]
      },
      "application/vnd.spotfire.sfs": {
        source: "iana",
        extensions: ["sfs"]
      },
      "application/vnd.sqlite3": {
        source: "iana"
      },
      "application/vnd.sss-cod": {
        source: "iana"
      },
      "application/vnd.sss-dtf": {
        source: "iana"
      },
      "application/vnd.sss-ntf": {
        source: "iana"
      },
      "application/vnd.stardivision.calc": {
        source: "apache",
        extensions: ["sdc"]
      },
      "application/vnd.stardivision.draw": {
        source: "apache",
        extensions: ["sda"]
      },
      "application/vnd.stardivision.impress": {
        source: "apache",
        extensions: ["sdd"]
      },
      "application/vnd.stardivision.math": {
        source: "apache",
        extensions: ["smf"]
      },
      "application/vnd.stardivision.writer": {
        source: "apache",
        extensions: ["sdw", "vor"]
      },
      "application/vnd.stardivision.writer-global": {
        source: "apache",
        extensions: ["sgl"]
      },
      "application/vnd.stepmania.package": {
        source: "iana",
        extensions: ["smzip"]
      },
      "application/vnd.stepmania.stepchart": {
        source: "iana",
        extensions: ["sm"]
      },
      "application/vnd.street-stream": {
        source: "iana"
      },
      "application/vnd.sun.wadl+xml": {
        source: "iana",
        compressible: true,
        extensions: ["wadl"]
      },
      "application/vnd.sun.xml.calc": {
        source: "apache",
        extensions: ["sxc"]
      },
      "application/vnd.sun.xml.calc.template": {
        source: "apache",
        extensions: ["stc"]
      },
      "application/vnd.sun.xml.draw": {
        source: "apache",
        extensions: ["sxd"]
      },
      "application/vnd.sun.xml.draw.template": {
        source: "apache",
        extensions: ["std"]
      },
      "application/vnd.sun.xml.impress": {
        source: "apache",
        extensions: ["sxi"]
      },
      "application/vnd.sun.xml.impress.template": {
        source: "apache",
        extensions: ["sti"]
      },
      "application/vnd.sun.xml.math": {
        source: "apache",
        extensions: ["sxm"]
      },
      "application/vnd.sun.xml.writer": {
        source: "apache",
        extensions: ["sxw"]
      },
      "application/vnd.sun.xml.writer.global": {
        source: "apache",
        extensions: ["sxg"]
      },
      "application/vnd.sun.xml.writer.template": {
        source: "apache",
        extensions: ["stw"]
      },
      "application/vnd.sus-calendar": {
        source: "iana",
        extensions: ["sus", "susp"]
      },
      "application/vnd.svd": {
        source: "iana",
        extensions: ["svd"]
      },
      "application/vnd.swiftview-ics": {
        source: "iana"
      },
      "application/vnd.sycle+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.syft+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.symbian.install": {
        source: "apache",
        extensions: ["sis", "sisx"]
      },
      "application/vnd.syncml+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["xsm"]
      },
      "application/vnd.syncml.dm+wbxml": {
        source: "iana",
        charset: "UTF-8",
        extensions: ["bdm"]
      },
      "application/vnd.syncml.dm+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["xdm"]
      },
      "application/vnd.syncml.dm.notification": {
        source: "iana"
      },
      "application/vnd.syncml.dmddf+wbxml": {
        source: "iana"
      },
      "application/vnd.syncml.dmddf+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["ddf"]
      },
      "application/vnd.syncml.dmtnds+wbxml": {
        source: "iana"
      },
      "application/vnd.syncml.dmtnds+xml": {
        source: "iana",
        charset: "UTF-8",
        compressible: true
      },
      "application/vnd.syncml.ds.notification": {
        source: "iana"
      },
      "application/vnd.tableschema+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.tao.intent-module-archive": {
        source: "iana",
        extensions: ["tao"]
      },
      "application/vnd.tcpdump.pcap": {
        source: "iana",
        extensions: ["pcap", "cap", "dmp"]
      },
      "application/vnd.think-cell.ppttc+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.tmd.mediaflex.api+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.tml": {
        source: "iana"
      },
      "application/vnd.tmobile-livetv": {
        source: "iana",
        extensions: ["tmo"]
      },
      "application/vnd.tri.onesource": {
        source: "iana"
      },
      "application/vnd.trid.tpt": {
        source: "iana",
        extensions: ["tpt"]
      },
      "application/vnd.triscape.mxs": {
        source: "iana",
        extensions: ["mxs"]
      },
      "application/vnd.trueapp": {
        source: "iana",
        extensions: ["tra"]
      },
      "application/vnd.truedoc": {
        source: "iana"
      },
      "application/vnd.ubisoft.webplayer": {
        source: "iana"
      },
      "application/vnd.ufdl": {
        source: "iana",
        extensions: ["ufd", "ufdl"]
      },
      "application/vnd.uiq.theme": {
        source: "iana",
        extensions: ["utz"]
      },
      "application/vnd.umajin": {
        source: "iana",
        extensions: ["umj"]
      },
      "application/vnd.unity": {
        source: "iana",
        extensions: ["unityweb"]
      },
      "application/vnd.uoml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["uoml"]
      },
      "application/vnd.uplanet.alert": {
        source: "iana"
      },
      "application/vnd.uplanet.alert-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.bearer-choice": {
        source: "iana"
      },
      "application/vnd.uplanet.bearer-choice-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.cacheop": {
        source: "iana"
      },
      "application/vnd.uplanet.cacheop-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.channel": {
        source: "iana"
      },
      "application/vnd.uplanet.channel-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.list": {
        source: "iana"
      },
      "application/vnd.uplanet.list-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.listcmd": {
        source: "iana"
      },
      "application/vnd.uplanet.listcmd-wbxml": {
        source: "iana"
      },
      "application/vnd.uplanet.signal": {
        source: "iana"
      },
      "application/vnd.uri-map": {
        source: "iana"
      },
      "application/vnd.valve.source.material": {
        source: "iana"
      },
      "application/vnd.vcx": {
        source: "iana",
        extensions: ["vcx"]
      },
      "application/vnd.vd-study": {
        source: "iana"
      },
      "application/vnd.vectorworks": {
        source: "iana"
      },
      "application/vnd.vel+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.verimatrix.vcas": {
        source: "iana"
      },
      "application/vnd.veritone.aion+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.veryant.thin": {
        source: "iana"
      },
      "application/vnd.ves.encrypted": {
        source: "iana"
      },
      "application/vnd.vidsoft.vidconference": {
        source: "iana"
      },
      "application/vnd.visio": {
        source: "iana",
        extensions: ["vsd", "vst", "vss", "vsw"]
      },
      "application/vnd.visionary": {
        source: "iana",
        extensions: ["vis"]
      },
      "application/vnd.vividence.scriptfile": {
        source: "iana"
      },
      "application/vnd.vsf": {
        source: "iana",
        extensions: ["vsf"]
      },
      "application/vnd.wap.sic": {
        source: "iana"
      },
      "application/vnd.wap.slc": {
        source: "iana"
      },
      "application/vnd.wap.wbxml": {
        source: "iana",
        charset: "UTF-8",
        extensions: ["wbxml"]
      },
      "application/vnd.wap.wmlc": {
        source: "iana",
        extensions: ["wmlc"]
      },
      "application/vnd.wap.wmlscriptc": {
        source: "iana",
        extensions: ["wmlsc"]
      },
      "application/vnd.webturbo": {
        source: "iana",
        extensions: ["wtb"]
      },
      "application/vnd.wfa.dpp": {
        source: "iana"
      },
      "application/vnd.wfa.p2p": {
        source: "iana"
      },
      "application/vnd.wfa.wsc": {
        source: "iana"
      },
      "application/vnd.windows.devicepairing": {
        source: "iana"
      },
      "application/vnd.wmc": {
        source: "iana"
      },
      "application/vnd.wmf.bootstrap": {
        source: "iana"
      },
      "application/vnd.wolfram.mathematica": {
        source: "iana"
      },
      "application/vnd.wolfram.mathematica.package": {
        source: "iana"
      },
      "application/vnd.wolfram.player": {
        source: "iana",
        extensions: ["nbp"]
      },
      "application/vnd.wordperfect": {
        source: "iana",
        extensions: ["wpd"]
      },
      "application/vnd.wqd": {
        source: "iana",
        extensions: ["wqd"]
      },
      "application/vnd.wrq-hp3000-labelled": {
        source: "iana"
      },
      "application/vnd.wt.stf": {
        source: "iana",
        extensions: ["stf"]
      },
      "application/vnd.wv.csp+wbxml": {
        source: "iana"
      },
      "application/vnd.wv.csp+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.wv.ssp+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.xacml+json": {
        source: "iana",
        compressible: true
      },
      "application/vnd.xara": {
        source: "iana",
        extensions: ["xar"]
      },
      "application/vnd.xfdl": {
        source: "iana",
        extensions: ["xfdl"]
      },
      "application/vnd.xfdl.webform": {
        source: "iana"
      },
      "application/vnd.xmi+xml": {
        source: "iana",
        compressible: true
      },
      "application/vnd.xmpie.cpkg": {
        source: "iana"
      },
      "application/vnd.xmpie.dpkg": {
        source: "iana"
      },
      "application/vnd.xmpie.plan": {
        source: "iana"
      },
      "application/vnd.xmpie.ppkg": {
        source: "iana"
      },
      "application/vnd.xmpie.xlim": {
        source: "iana"
      },
      "application/vnd.yamaha.hv-dic": {
        source: "iana",
        extensions: ["hvd"]
      },
      "application/vnd.yamaha.hv-script": {
        source: "iana",
        extensions: ["hvs"]
      },
      "application/vnd.yamaha.hv-voice": {
        source: "iana",
        extensions: ["hvp"]
      },
      "application/vnd.yamaha.openscoreformat": {
        source: "iana",
        extensions: ["osf"]
      },
      "application/vnd.yamaha.openscoreformat.osfpvg+xml": {
        source: "iana",
        compressible: true,
        extensions: ["osfpvg"]
      },
      "application/vnd.yamaha.remote-setup": {
        source: "iana"
      },
      "application/vnd.yamaha.smaf-audio": {
        source: "iana",
        extensions: ["saf"]
      },
      "application/vnd.yamaha.smaf-phrase": {
        source: "iana",
        extensions: ["spf"]
      },
      "application/vnd.yamaha.through-ngn": {
        source: "iana"
      },
      "application/vnd.yamaha.tunnel-udpencap": {
        source: "iana"
      },
      "application/vnd.yaoweme": {
        source: "iana"
      },
      "application/vnd.yellowriver-custom-menu": {
        source: "iana",
        extensions: ["cmp"]
      },
      "application/vnd.youtube.yt": {
        source: "iana"
      },
      "application/vnd.zul": {
        source: "iana",
        extensions: ["zir", "zirz"]
      },
      "application/vnd.zzazz.deck+xml": {
        source: "iana",
        compressible: true,
        extensions: ["zaz"]
      },
      "application/voicexml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["vxml"]
      },
      "application/voucher-cms+json": {
        source: "iana",
        compressible: true
      },
      "application/vq-rtcpxr": {
        source: "iana"
      },
      "application/wasm": {
        source: "iana",
        compressible: true,
        extensions: ["wasm"]
      },
      "application/watcherinfo+xml": {
        source: "iana",
        compressible: true,
        extensions: ["wif"]
      },
      "application/webpush-options+json": {
        source: "iana",
        compressible: true
      },
      "application/whoispp-query": {
        source: "iana"
      },
      "application/whoispp-response": {
        source: "iana"
      },
      "application/widget": {
        source: "iana",
        extensions: ["wgt"]
      },
      "application/winhlp": {
        source: "apache",
        extensions: ["hlp"]
      },
      "application/wita": {
        source: "iana"
      },
      "application/wordperfect5.1": {
        source: "iana"
      },
      "application/wsdl+xml": {
        source: "iana",
        compressible: true,
        extensions: ["wsdl"]
      },
      "application/wspolicy+xml": {
        source: "iana",
        compressible: true,
        extensions: ["wspolicy"]
      },
      "application/x-7z-compressed": {
        source: "apache",
        compressible: false,
        extensions: ["7z"]
      },
      "application/x-abiword": {
        source: "apache",
        extensions: ["abw"]
      },
      "application/x-ace-compressed": {
        source: "apache",
        extensions: ["ace"]
      },
      "application/x-amf": {
        source: "apache"
      },
      "application/x-apple-diskimage": {
        source: "apache",
        extensions: ["dmg"]
      },
      "application/x-arj": {
        compressible: false,
        extensions: ["arj"]
      },
      "application/x-authorware-bin": {
        source: "apache",
        extensions: ["aab", "x32", "u32", "vox"]
      },
      "application/x-authorware-map": {
        source: "apache",
        extensions: ["aam"]
      },
      "application/x-authorware-seg": {
        source: "apache",
        extensions: ["aas"]
      },
      "application/x-bcpio": {
        source: "apache",
        extensions: ["bcpio"]
      },
      "application/x-bdoc": {
        compressible: false,
        extensions: ["bdoc"]
      },
      "application/x-bittorrent": {
        source: "apache",
        extensions: ["torrent"]
      },
      "application/x-blorb": {
        source: "apache",
        extensions: ["blb", "blorb"]
      },
      "application/x-bzip": {
        source: "apache",
        compressible: false,
        extensions: ["bz"]
      },
      "application/x-bzip2": {
        source: "apache",
        compressible: false,
        extensions: ["bz2", "boz"]
      },
      "application/x-cbr": {
        source: "apache",
        extensions: ["cbr", "cba", "cbt", "cbz", "cb7"]
      },
      "application/x-cdlink": {
        source: "apache",
        extensions: ["vcd"]
      },
      "application/x-cfs-compressed": {
        source: "apache",
        extensions: ["cfs"]
      },
      "application/x-chat": {
        source: "apache",
        extensions: ["chat"]
      },
      "application/x-chess-pgn": {
        source: "apache",
        extensions: ["pgn"]
      },
      "application/x-chrome-extension": {
        extensions: ["crx"]
      },
      "application/x-cocoa": {
        source: "nginx",
        extensions: ["cco"]
      },
      "application/x-compress": {
        source: "apache"
      },
      "application/x-conference": {
        source: "apache",
        extensions: ["nsc"]
      },
      "application/x-cpio": {
        source: "apache",
        extensions: ["cpio"]
      },
      "application/x-csh": {
        source: "apache",
        extensions: ["csh"]
      },
      "application/x-deb": {
        compressible: false
      },
      "application/x-debian-package": {
        source: "apache",
        extensions: ["deb", "udeb"]
      },
      "application/x-dgc-compressed": {
        source: "apache",
        extensions: ["dgc"]
      },
      "application/x-director": {
        source: "apache",
        extensions: ["dir", "dcr", "dxr", "cst", "cct", "cxt", "w3d", "fgd", "swa"]
      },
      "application/x-doom": {
        source: "apache",
        extensions: ["wad"]
      },
      "application/x-dtbncx+xml": {
        source: "apache",
        compressible: true,
        extensions: ["ncx"]
      },
      "application/x-dtbook+xml": {
        source: "apache",
        compressible: true,
        extensions: ["dtb"]
      },
      "application/x-dtbresource+xml": {
        source: "apache",
        compressible: true,
        extensions: ["res"]
      },
      "application/x-dvi": {
        source: "apache",
        compressible: false,
        extensions: ["dvi"]
      },
      "application/x-envoy": {
        source: "apache",
        extensions: ["evy"]
      },
      "application/x-eva": {
        source: "apache",
        extensions: ["eva"]
      },
      "application/x-font-bdf": {
        source: "apache",
        extensions: ["bdf"]
      },
      "application/x-font-dos": {
        source: "apache"
      },
      "application/x-font-framemaker": {
        source: "apache"
      },
      "application/x-font-ghostscript": {
        source: "apache",
        extensions: ["gsf"]
      },
      "application/x-font-libgrx": {
        source: "apache"
      },
      "application/x-font-linux-psf": {
        source: "apache",
        extensions: ["psf"]
      },
      "application/x-font-pcf": {
        source: "apache",
        extensions: ["pcf"]
      },
      "application/x-font-snf": {
        source: "apache",
        extensions: ["snf"]
      },
      "application/x-font-speedo": {
        source: "apache"
      },
      "application/x-font-sunos-news": {
        source: "apache"
      },
      "application/x-font-type1": {
        source: "apache",
        extensions: ["pfa", "pfb", "pfm", "afm"]
      },
      "application/x-font-vfont": {
        source: "apache"
      },
      "application/x-freearc": {
        source: "apache",
        extensions: ["arc"]
      },
      "application/x-futuresplash": {
        source: "apache",
        extensions: ["spl"]
      },
      "application/x-gca-compressed": {
        source: "apache",
        extensions: ["gca"]
      },
      "application/x-glulx": {
        source: "apache",
        extensions: ["ulx"]
      },
      "application/x-gnumeric": {
        source: "apache",
        extensions: ["gnumeric"]
      },
      "application/x-gramps-xml": {
        source: "apache",
        extensions: ["gramps"]
      },
      "application/x-gtar": {
        source: "apache",
        extensions: ["gtar"]
      },
      "application/x-gzip": {
        source: "apache"
      },
      "application/x-hdf": {
        source: "apache",
        extensions: ["hdf"]
      },
      "application/x-httpd-php": {
        compressible: true,
        extensions: ["php"]
      },
      "application/x-install-instructions": {
        source: "apache",
        extensions: ["install"]
      },
      "application/x-iso9660-image": {
        source: "apache",
        extensions: ["iso"]
      },
      "application/x-iwork-keynote-sffkey": {
        extensions: ["key"]
      },
      "application/x-iwork-numbers-sffnumbers": {
        extensions: ["numbers"]
      },
      "application/x-iwork-pages-sffpages": {
        extensions: ["pages"]
      },
      "application/x-java-archive-diff": {
        source: "nginx",
        extensions: ["jardiff"]
      },
      "application/x-java-jnlp-file": {
        source: "apache",
        compressible: false,
        extensions: ["jnlp"]
      },
      "application/x-javascript": {
        compressible: true
      },
      "application/x-keepass2": {
        extensions: ["kdbx"]
      },
      "application/x-latex": {
        source: "apache",
        compressible: false,
        extensions: ["latex"]
      },
      "application/x-lua-bytecode": {
        extensions: ["luac"]
      },
      "application/x-lzh-compressed": {
        source: "apache",
        extensions: ["lzh", "lha"]
      },
      "application/x-makeself": {
        source: "nginx",
        extensions: ["run"]
      },
      "application/x-mie": {
        source: "apache",
        extensions: ["mie"]
      },
      "application/x-mobipocket-ebook": {
        source: "apache",
        extensions: ["prc", "mobi"]
      },
      "application/x-mpegurl": {
        compressible: false
      },
      "application/x-ms-application": {
        source: "apache",
        extensions: ["application"]
      },
      "application/x-ms-shortcut": {
        source: "apache",
        extensions: ["lnk"]
      },
      "application/x-ms-wmd": {
        source: "apache",
        extensions: ["wmd"]
      },
      "application/x-ms-wmz": {
        source: "apache",
        extensions: ["wmz"]
      },
      "application/x-ms-xbap": {
        source: "apache",
        extensions: ["xbap"]
      },
      "application/x-msaccess": {
        source: "apache",
        extensions: ["mdb"]
      },
      "application/x-msbinder": {
        source: "apache",
        extensions: ["obd"]
      },
      "application/x-mscardfile": {
        source: "apache",
        extensions: ["crd"]
      },
      "application/x-msclip": {
        source: "apache",
        extensions: ["clp"]
      },
      "application/x-msdos-program": {
        extensions: ["exe"]
      },
      "application/x-msdownload": {
        source: "apache",
        extensions: ["exe", "dll", "com", "bat", "msi"]
      },
      "application/x-msmediaview": {
        source: "apache",
        extensions: ["mvb", "m13", "m14"]
      },
      "application/x-msmetafile": {
        source: "apache",
        extensions: ["wmf", "wmz", "emf", "emz"]
      },
      "application/x-msmoney": {
        source: "apache",
        extensions: ["mny"]
      },
      "application/x-mspublisher": {
        source: "apache",
        extensions: ["pub"]
      },
      "application/x-msschedule": {
        source: "apache",
        extensions: ["scd"]
      },
      "application/x-msterminal": {
        source: "apache",
        extensions: ["trm"]
      },
      "application/x-mswrite": {
        source: "apache",
        extensions: ["wri"]
      },
      "application/x-netcdf": {
        source: "apache",
        extensions: ["nc", "cdf"]
      },
      "application/x-ns-proxy-autoconfig": {
        compressible: true,
        extensions: ["pac"]
      },
      "application/x-nzb": {
        source: "apache",
        extensions: ["nzb"]
      },
      "application/x-perl": {
        source: "nginx",
        extensions: ["pl", "pm"]
      },
      "application/x-pilot": {
        source: "nginx",
        extensions: ["prc", "pdb"]
      },
      "application/x-pkcs12": {
        source: "apache",
        compressible: false,
        extensions: ["p12", "pfx"]
      },
      "application/x-pkcs7-certificates": {
        source: "apache",
        extensions: ["p7b", "spc"]
      },
      "application/x-pkcs7-certreqresp": {
        source: "apache",
        extensions: ["p7r"]
      },
      "application/x-pki-message": {
        source: "iana"
      },
      "application/x-rar-compressed": {
        source: "apache",
        compressible: false,
        extensions: ["rar"]
      },
      "application/x-redhat-package-manager": {
        source: "nginx",
        extensions: ["rpm"]
      },
      "application/x-research-info-systems": {
        source: "apache",
        extensions: ["ris"]
      },
      "application/x-sea": {
        source: "nginx",
        extensions: ["sea"]
      },
      "application/x-sh": {
        source: "apache",
        compressible: true,
        extensions: ["sh"]
      },
      "application/x-shar": {
        source: "apache",
        extensions: ["shar"]
      },
      "application/x-shockwave-flash": {
        source: "apache",
        compressible: false,
        extensions: ["swf"]
      },
      "application/x-silverlight-app": {
        source: "apache",
        extensions: ["xap"]
      },
      "application/x-sql": {
        source: "apache",
        extensions: ["sql"]
      },
      "application/x-stuffit": {
        source: "apache",
        compressible: false,
        extensions: ["sit"]
      },
      "application/x-stuffitx": {
        source: "apache",
        extensions: ["sitx"]
      },
      "application/x-subrip": {
        source: "apache",
        extensions: ["srt"]
      },
      "application/x-sv4cpio": {
        source: "apache",
        extensions: ["sv4cpio"]
      },
      "application/x-sv4crc": {
        source: "apache",
        extensions: ["sv4crc"]
      },
      "application/x-t3vm-image": {
        source: "apache",
        extensions: ["t3"]
      },
      "application/x-tads": {
        source: "apache",
        extensions: ["gam"]
      },
      "application/x-tar": {
        source: "apache",
        compressible: true,
        extensions: ["tar"]
      },
      "application/x-tcl": {
        source: "apache",
        extensions: ["tcl", "tk"]
      },
      "application/x-tex": {
        source: "apache",
        extensions: ["tex"]
      },
      "application/x-tex-tfm": {
        source: "apache",
        extensions: ["tfm"]
      },
      "application/x-texinfo": {
        source: "apache",
        extensions: ["texinfo", "texi"]
      },
      "application/x-tgif": {
        source: "apache",
        extensions: ["obj"]
      },
      "application/x-ustar": {
        source: "apache",
        extensions: ["ustar"]
      },
      "application/x-virtualbox-hdd": {
        compressible: true,
        extensions: ["hdd"]
      },
      "application/x-virtualbox-ova": {
        compressible: true,
        extensions: ["ova"]
      },
      "application/x-virtualbox-ovf": {
        compressible: true,
        extensions: ["ovf"]
      },
      "application/x-virtualbox-vbox": {
        compressible: true,
        extensions: ["vbox"]
      },
      "application/x-virtualbox-vbox-extpack": {
        compressible: false,
        extensions: ["vbox-extpack"]
      },
      "application/x-virtualbox-vdi": {
        compressible: true,
        extensions: ["vdi"]
      },
      "application/x-virtualbox-vhd": {
        compressible: true,
        extensions: ["vhd"]
      },
      "application/x-virtualbox-vmdk": {
        compressible: true,
        extensions: ["vmdk"]
      },
      "application/x-wais-source": {
        source: "apache",
        extensions: ["src"]
      },
      "application/x-web-app-manifest+json": {
        compressible: true,
        extensions: ["webapp"]
      },
      "application/x-www-form-urlencoded": {
        source: "iana",
        compressible: true
      },
      "application/x-x509-ca-cert": {
        source: "iana",
        extensions: ["der", "crt", "pem"]
      },
      "application/x-x509-ca-ra-cert": {
        source: "iana"
      },
      "application/x-x509-next-ca-cert": {
        source: "iana"
      },
      "application/x-xfig": {
        source: "apache",
        extensions: ["fig"]
      },
      "application/x-xliff+xml": {
        source: "apache",
        compressible: true,
        extensions: ["xlf"]
      },
      "application/x-xpinstall": {
        source: "apache",
        compressible: false,
        extensions: ["xpi"]
      },
      "application/x-xz": {
        source: "apache",
        extensions: ["xz"]
      },
      "application/x-zmachine": {
        source: "apache",
        extensions: ["z1", "z2", "z3", "z4", "z5", "z6", "z7", "z8"]
      },
      "application/x400-bp": {
        source: "iana"
      },
      "application/xacml+xml": {
        source: "iana",
        compressible: true
      },
      "application/xaml+xml": {
        source: "apache",
        compressible: true,
        extensions: ["xaml"]
      },
      "application/xcap-att+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xav"]
      },
      "application/xcap-caps+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xca"]
      },
      "application/xcap-diff+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xdf"]
      },
      "application/xcap-el+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xel"]
      },
      "application/xcap-error+xml": {
        source: "iana",
        compressible: true
      },
      "application/xcap-ns+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xns"]
      },
      "application/xcon-conference-info+xml": {
        source: "iana",
        compressible: true
      },
      "application/xcon-conference-info-diff+xml": {
        source: "iana",
        compressible: true
      },
      "application/xenc+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xenc"]
      },
      "application/xhtml+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xhtml", "xht"]
      },
      "application/xhtml-voice+xml": {
        source: "apache",
        compressible: true
      },
      "application/xliff+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xlf"]
      },
      "application/xml": {
        source: "iana",
        compressible: true,
        extensions: ["xml", "xsl", "xsd", "rng"]
      },
      "application/xml-dtd": {
        source: "iana",
        compressible: true,
        extensions: ["dtd"]
      },
      "application/xml-external-parsed-entity": {
        source: "iana"
      },
      "application/xml-patch+xml": {
        source: "iana",
        compressible: true
      },
      "application/xmpp+xml": {
        source: "iana",
        compressible: true
      },
      "application/xop+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xop"]
      },
      "application/xproc+xml": {
        source: "apache",
        compressible: true,
        extensions: ["xpl"]
      },
      "application/xslt+xml": {
        source: "iana",
        compressible: true,
        extensions: ["xsl", "xslt"]
      },
      "application/xspf+xml": {
        source: "apache",
        compressible: true,
        extensions: ["xspf"]
      },
      "application/xv+xml": {
        source: "iana",
        compressible: true,
        extensions: ["mxml", "xhvml", "xvml", "xvm"]
      },
      "application/yang": {
        source: "iana",
        extensions: ["yang"]
      },
      "application/yang-data+json": {
        source: "iana",
        compressible: true
      },
      "application/yang-data+xml": {
        source: "iana",
        compressible: true
      },
      "application/yang-patch+json": {
        source: "iana",
        compressible: true
      },
      "application/yang-patch+xml": {
        source: "iana",
        compressible: true
      },
      "application/yin+xml": {
        source: "iana",
        compressible: true,
        extensions: ["yin"]
      },
      "application/zip": {
        source: "iana",
        compressible: false,
        extensions: ["zip"]
      },
      "application/zlib": {
        source: "iana"
      },
      "application/zstd": {
        source: "iana"
      },
      "audio/1d-interleaved-parityfec": {
        source: "iana"
      },
      "audio/32kadpcm": {
        source: "iana"
      },
      "audio/3gpp": {
        source: "iana",
        compressible: false,
        extensions: ["3gpp"]
      },
      "audio/3gpp2": {
        source: "iana"
      },
      "audio/aac": {
        source: "iana"
      },
      "audio/ac3": {
        source: "iana"
      },
      "audio/adpcm": {
        source: "apache",
        extensions: ["adp"]
      },
      "audio/amr": {
        source: "iana",
        extensions: ["amr"]
      },
      "audio/amr-wb": {
        source: "iana"
      },
      "audio/amr-wb+": {
        source: "iana"
      },
      "audio/aptx": {
        source: "iana"
      },
      "audio/asc": {
        source: "iana"
      },
      "audio/atrac-advanced-lossless": {
        source: "iana"
      },
      "audio/atrac-x": {
        source: "iana"
      },
      "audio/atrac3": {
        source: "iana"
      },
      "audio/basic": {
        source: "iana",
        compressible: false,
        extensions: ["au", "snd"]
      },
      "audio/bv16": {
        source: "iana"
      },
      "audio/bv32": {
        source: "iana"
      },
      "audio/clearmode": {
        source: "iana"
      },
      "audio/cn": {
        source: "iana"
      },
      "audio/dat12": {
        source: "iana"
      },
      "audio/dls": {
        source: "iana"
      },
      "audio/dsr-es201108": {
        source: "iana"
      },
      "audio/dsr-es202050": {
        source: "iana"
      },
      "audio/dsr-es202211": {
        source: "iana"
      },
      "audio/dsr-es202212": {
        source: "iana"
      },
      "audio/dv": {
        source: "iana"
      },
      "audio/dvi4": {
        source: "iana"
      },
      "audio/eac3": {
        source: "iana"
      },
      "audio/encaprtp": {
        source: "iana"
      },
      "audio/evrc": {
        source: "iana"
      },
      "audio/evrc-qcp": {
        source: "iana"
      },
      "audio/evrc0": {
        source: "iana"
      },
      "audio/evrc1": {
        source: "iana"
      },
      "audio/evrcb": {
        source: "iana"
      },
      "audio/evrcb0": {
        source: "iana"
      },
      "audio/evrcb1": {
        source: "iana"
      },
      "audio/evrcnw": {
        source: "iana"
      },
      "audio/evrcnw0": {
        source: "iana"
      },
      "audio/evrcnw1": {
        source: "iana"
      },
      "audio/evrcwb": {
        source: "iana"
      },
      "audio/evrcwb0": {
        source: "iana"
      },
      "audio/evrcwb1": {
        source: "iana"
      },
      "audio/evs": {
        source: "iana"
      },
      "audio/flexfec": {
        source: "iana"
      },
      "audio/fwdred": {
        source: "iana"
      },
      "audio/g711-0": {
        source: "iana"
      },
      "audio/g719": {
        source: "iana"
      },
      "audio/g722": {
        source: "iana"
      },
      "audio/g7221": {
        source: "iana"
      },
      "audio/g723": {
        source: "iana"
      },
      "audio/g726-16": {
        source: "iana"
      },
      "audio/g726-24": {
        source: "iana"
      },
      "audio/g726-32": {
        source: "iana"
      },
      "audio/g726-40": {
        source: "iana"
      },
      "audio/g728": {
        source: "iana"
      },
      "audio/g729": {
        source: "iana"
      },
      "audio/g7291": {
        source: "iana"
      },
      "audio/g729d": {
        source: "iana"
      },
      "audio/g729e": {
        source: "iana"
      },
      "audio/gsm": {
        source: "iana"
      },
      "audio/gsm-efr": {
        source: "iana"
      },
      "audio/gsm-hr-08": {
        source: "iana"
      },
      "audio/ilbc": {
        source: "iana"
      },
      "audio/ip-mr_v2.5": {
        source: "iana"
      },
      "audio/isac": {
        source: "apache"
      },
      "audio/l16": {
        source: "iana"
      },
      "audio/l20": {
        source: "iana"
      },
      "audio/l24": {
        source: "iana",
        compressible: false
      },
      "audio/l8": {
        source: "iana"
      },
      "audio/lpc": {
        source: "iana"
      },
      "audio/melp": {
        source: "iana"
      },
      "audio/melp1200": {
        source: "iana"
      },
      "audio/melp2400": {
        source: "iana"
      },
      "audio/melp600": {
        source: "iana"
      },
      "audio/mhas": {
        source: "iana"
      },
      "audio/midi": {
        source: "apache",
        extensions: ["mid", "midi", "kar", "rmi"]
      },
      "audio/mobile-xmf": {
        source: "iana",
        extensions: ["mxmf"]
      },
      "audio/mp3": {
        compressible: false,
        extensions: ["mp3"]
      },
      "audio/mp4": {
        source: "iana",
        compressible: false,
        extensions: ["m4a", "mp4a"]
      },
      "audio/mp4a-latm": {
        source: "iana"
      },
      "audio/mpa": {
        source: "iana"
      },
      "audio/mpa-robust": {
        source: "iana"
      },
      "audio/mpeg": {
        source: "iana",
        compressible: false,
        extensions: ["mpga", "mp2", "mp2a", "mp3", "m2a", "m3a"]
      },
      "audio/mpeg4-generic": {
        source: "iana"
      },
      "audio/musepack": {
        source: "apache"
      },
      "audio/ogg": {
        source: "iana",
        compressible: false,
        extensions: ["oga", "ogg", "spx", "opus"]
      },
      "audio/opus": {
        source: "iana"
      },
      "audio/parityfec": {
        source: "iana"
      },
      "audio/pcma": {
        source: "iana"
      },
      "audio/pcma-wb": {
        source: "iana"
      },
      "audio/pcmu": {
        source: "iana"
      },
      "audio/pcmu-wb": {
        source: "iana"
      },
      "audio/prs.sid": {
        source: "iana"
      },
      "audio/qcelp": {
        source: "iana"
      },
      "audio/raptorfec": {
        source: "iana"
      },
      "audio/red": {
        source: "iana"
      },
      "audio/rtp-enc-aescm128": {
        source: "iana"
      },
      "audio/rtp-midi": {
        source: "iana"
      },
      "audio/rtploopback": {
        source: "iana"
      },
      "audio/rtx": {
        source: "iana"
      },
      "audio/s3m": {
        source: "apache",
        extensions: ["s3m"]
      },
      "audio/scip": {
        source: "iana"
      },
      "audio/silk": {
        source: "apache",
        extensions: ["sil"]
      },
      "audio/smv": {
        source: "iana"
      },
      "audio/smv-qcp": {
        source: "iana"
      },
      "audio/smv0": {
        source: "iana"
      },
      "audio/sofa": {
        source: "iana"
      },
      "audio/sp-midi": {
        source: "iana"
      },
      "audio/speex": {
        source: "iana"
      },
      "audio/t140c": {
        source: "iana"
      },
      "audio/t38": {
        source: "iana"
      },
      "audio/telephone-event": {
        source: "iana"
      },
      "audio/tetra_acelp": {
        source: "iana"
      },
      "audio/tetra_acelp_bb": {
        source: "iana"
      },
      "audio/tone": {
        source: "iana"
      },
      "audio/tsvcis": {
        source: "iana"
      },
      "audio/uemclip": {
        source: "iana"
      },
      "audio/ulpfec": {
        source: "iana"
      },
      "audio/usac": {
        source: "iana"
      },
      "audio/vdvi": {
        source: "iana"
      },
      "audio/vmr-wb": {
        source: "iana"
      },
      "audio/vnd.3gpp.iufp": {
        source: "iana"
      },
      "audio/vnd.4sb": {
        source: "iana"
      },
      "audio/vnd.audiokoz": {
        source: "iana"
      },
      "audio/vnd.celp": {
        source: "iana"
      },
      "audio/vnd.cisco.nse": {
        source: "iana"
      },
      "audio/vnd.cmles.radio-events": {
        source: "iana"
      },
      "audio/vnd.cns.anp1": {
        source: "iana"
      },
      "audio/vnd.cns.inf1": {
        source: "iana"
      },
      "audio/vnd.dece.audio": {
        source: "iana",
        extensions: ["uva", "uvva"]
      },
      "audio/vnd.digital-winds": {
        source: "iana",
        extensions: ["eol"]
      },
      "audio/vnd.dlna.adts": {
        source: "iana"
      },
      "audio/vnd.dolby.heaac.1": {
        source: "iana"
      },
      "audio/vnd.dolby.heaac.2": {
        source: "iana"
      },
      "audio/vnd.dolby.mlp": {
        source: "iana"
      },
      "audio/vnd.dolby.mps": {
        source: "iana"
      },
      "audio/vnd.dolby.pl2": {
        source: "iana"
      },
      "audio/vnd.dolby.pl2x": {
        source: "iana"
      },
      "audio/vnd.dolby.pl2z": {
        source: "iana"
      },
      "audio/vnd.dolby.pulse.1": {
        source: "iana"
      },
      "audio/vnd.dra": {
        source: "iana",
        extensions: ["dra"]
      },
      "audio/vnd.dts": {
        source: "iana",
        extensions: ["dts"]
      },
      "audio/vnd.dts.hd": {
        source: "iana",
        extensions: ["dtshd"]
      },
      "audio/vnd.dts.uhd": {
        source: "iana"
      },
      "audio/vnd.dvb.file": {
        source: "iana"
      },
      "audio/vnd.everad.plj": {
        source: "iana"
      },
      "audio/vnd.hns.audio": {
        source: "iana"
      },
      "audio/vnd.lucent.voice": {
        source: "iana",
        extensions: ["lvp"]
      },
      "audio/vnd.ms-playready.media.pya": {
        source: "iana",
        extensions: ["pya"]
      },
      "audio/vnd.nokia.mobile-xmf": {
        source: "iana"
      },
      "audio/vnd.nortel.vbk": {
        source: "iana"
      },
      "audio/vnd.nuera.ecelp4800": {
        source: "iana",
        extensions: ["ecelp4800"]
      },
      "audio/vnd.nuera.ecelp7470": {
        source: "iana",
        extensions: ["ecelp7470"]
      },
      "audio/vnd.nuera.ecelp9600": {
        source: "iana",
        extensions: ["ecelp9600"]
      },
      "audio/vnd.octel.sbc": {
        source: "iana"
      },
      "audio/vnd.presonus.multitrack": {
        source: "iana"
      },
      "audio/vnd.qcelp": {
        source: "iana"
      },
      "audio/vnd.rhetorex.32kadpcm": {
        source: "iana"
      },
      "audio/vnd.rip": {
        source: "iana",
        extensions: ["rip"]
      },
      "audio/vnd.rn-realaudio": {
        compressible: false
      },
      "audio/vnd.sealedmedia.softseal.mpeg": {
        source: "iana"
      },
      "audio/vnd.vmx.cvsd": {
        source: "iana"
      },
      "audio/vnd.wave": {
        compressible: false
      },
      "audio/vorbis": {
        source: "iana",
        compressible: false
      },
      "audio/vorbis-config": {
        source: "iana"
      },
      "audio/wav": {
        compressible: false,
        extensions: ["wav"]
      },
      "audio/wave": {
        compressible: false,
        extensions: ["wav"]
      },
      "audio/webm": {
        source: "apache",
        compressible: false,
        extensions: ["weba"]
      },
      "audio/x-aac": {
        source: "apache",
        compressible: false,
        extensions: ["aac"]
      },
      "audio/x-aiff": {
        source: "apache",
        extensions: ["aif", "aiff", "aifc"]
      },
      "audio/x-caf": {
        source: "apache",
        compressible: false,
        extensions: ["caf"]
      },
      "audio/x-flac": {
        source: "apache",
        extensions: ["flac"]
      },
      "audio/x-m4a": {
        source: "nginx",
        extensions: ["m4a"]
      },
      "audio/x-matroska": {
        source: "apache",
        extensions: ["mka"]
      },
      "audio/x-mpegurl": {
        source: "apache",
        extensions: ["m3u"]
      },
      "audio/x-ms-wax": {
        source: "apache",
        extensions: ["wax"]
      },
      "audio/x-ms-wma": {
        source: "apache",
        extensions: ["wma"]
      },
      "audio/x-pn-realaudio": {
        source: "apache",
        extensions: ["ram", "ra"]
      },
      "audio/x-pn-realaudio-plugin": {
        source: "apache",
        extensions: ["rmp"]
      },
      "audio/x-realaudio": {
        source: "nginx",
        extensions: ["ra"]
      },
      "audio/x-tta": {
        source: "apache"
      },
      "audio/x-wav": {
        source: "apache",
        extensions: ["wav"]
      },
      "audio/xm": {
        source: "apache",
        extensions: ["xm"]
      },
      "chemical/x-cdx": {
        source: "apache",
        extensions: ["cdx"]
      },
      "chemical/x-cif": {
        source: "apache",
        extensions: ["cif"]
      },
      "chemical/x-cmdf": {
        source: "apache",
        extensions: ["cmdf"]
      },
      "chemical/x-cml": {
        source: "apache",
        extensions: ["cml"]
      },
      "chemical/x-csml": {
        source: "apache",
        extensions: ["csml"]
      },
      "chemical/x-pdb": {
        source: "apache"
      },
      "chemical/x-xyz": {
        source: "apache",
        extensions: ["xyz"]
      },
      "font/collection": {
        source: "iana",
        extensions: ["ttc"]
      },
      "font/otf": {
        source: "iana",
        compressible: true,
        extensions: ["otf"]
      },
      "font/sfnt": {
        source: "iana"
      },
      "font/ttf": {
        source: "iana",
        compressible: true,
        extensions: ["ttf"]
      },
      "font/woff": {
        source: "iana",
        extensions: ["woff"]
      },
      "font/woff2": {
        source: "iana",
        extensions: ["woff2"]
      },
      "image/aces": {
        source: "iana",
        extensions: ["exr"]
      },
      "image/apng": {
        compressible: false,
        extensions: ["apng"]
      },
      "image/avci": {
        source: "iana",
        extensions: ["avci"]
      },
      "image/avcs": {
        source: "iana",
        extensions: ["avcs"]
      },
      "image/avif": {
        source: "iana",
        compressible: false,
        extensions: ["avif"]
      },
      "image/bmp": {
        source: "iana",
        compressible: true,
        extensions: ["bmp"]
      },
      "image/cgm": {
        source: "iana",
        extensions: ["cgm"]
      },
      "image/dicom-rle": {
        source: "iana",
        extensions: ["drle"]
      },
      "image/emf": {
        source: "iana",
        extensions: ["emf"]
      },
      "image/fits": {
        source: "iana",
        extensions: ["fits"]
      },
      "image/g3fax": {
        source: "iana",
        extensions: ["g3"]
      },
      "image/gif": {
        source: "iana",
        compressible: false,
        extensions: ["gif"]
      },
      "image/heic": {
        source: "iana",
        extensions: ["heic"]
      },
      "image/heic-sequence": {
        source: "iana",
        extensions: ["heics"]
      },
      "image/heif": {
        source: "iana",
        extensions: ["heif"]
      },
      "image/heif-sequence": {
        source: "iana",
        extensions: ["heifs"]
      },
      "image/hej2k": {
        source: "iana",
        extensions: ["hej2"]
      },
      "image/hsj2": {
        source: "iana",
        extensions: ["hsj2"]
      },
      "image/ief": {
        source: "iana",
        extensions: ["ief"]
      },
      "image/jls": {
        source: "iana",
        extensions: ["jls"]
      },
      "image/jp2": {
        source: "iana",
        compressible: false,
        extensions: ["jp2", "jpg2"]
      },
      "image/jpeg": {
        source: "iana",
        compressible: false,
        extensions: ["jpeg", "jpg", "jpe"]
      },
      "image/jph": {
        source: "iana",
        extensions: ["jph"]
      },
      "image/jphc": {
        source: "iana",
        extensions: ["jhc"]
      },
      "image/jpm": {
        source: "iana",
        compressible: false,
        extensions: ["jpm"]
      },
      "image/jpx": {
        source: "iana",
        compressible: false,
        extensions: ["jpx", "jpf"]
      },
      "image/jxr": {
        source: "iana",
        extensions: ["jxr"]
      },
      "image/jxra": {
        source: "iana",
        extensions: ["jxra"]
      },
      "image/jxrs": {
        source: "iana",
        extensions: ["jxrs"]
      },
      "image/jxs": {
        source: "iana",
        extensions: ["jxs"]
      },
      "image/jxsc": {
        source: "iana",
        extensions: ["jxsc"]
      },
      "image/jxsi": {
        source: "iana",
        extensions: ["jxsi"]
      },
      "image/jxss": {
        source: "iana",
        extensions: ["jxss"]
      },
      "image/ktx": {
        source: "iana",
        extensions: ["ktx"]
      },
      "image/ktx2": {
        source: "iana",
        extensions: ["ktx2"]
      },
      "image/naplps": {
        source: "iana"
      },
      "image/pjpeg": {
        compressible: false
      },
      "image/png": {
        source: "iana",
        compressible: false,
        extensions: ["png"]
      },
      "image/prs.btif": {
        source: "iana",
        extensions: ["btif"]
      },
      "image/prs.pti": {
        source: "iana",
        extensions: ["pti"]
      },
      "image/pwg-raster": {
        source: "iana"
      },
      "image/sgi": {
        source: "apache",
        extensions: ["sgi"]
      },
      "image/svg+xml": {
        source: "iana",
        compressible: true,
        extensions: ["svg", "svgz"]
      },
      "image/t38": {
        source: "iana",
        extensions: ["t38"]
      },
      "image/tiff": {
        source: "iana",
        compressible: false,
        extensions: ["tif", "tiff"]
      },
      "image/tiff-fx": {
        source: "iana",
        extensions: ["tfx"]
      },
      "image/vnd.adobe.photoshop": {
        source: "iana",
        compressible: true,
        extensions: ["psd"]
      },
      "image/vnd.airzip.accelerator.azv": {
        source: "iana",
        extensions: ["azv"]
      },
      "image/vnd.cns.inf2": {
        source: "iana"
      },
      "image/vnd.dece.graphic": {
        source: "iana",
        extensions: ["uvi", "uvvi", "uvg", "uvvg"]
      },
      "image/vnd.djvu": {
        source: "iana",
        extensions: ["djvu", "djv"]
      },
      "image/vnd.dvb.subtitle": {
        source: "iana",
        extensions: ["sub"]
      },
      "image/vnd.dwg": {
        source: "iana",
        extensions: ["dwg"]
      },
      "image/vnd.dxf": {
        source: "iana",
        extensions: ["dxf"]
      },
      "image/vnd.fastbidsheet": {
        source: "iana",
        extensions: ["fbs"]
      },
      "image/vnd.fpx": {
        source: "iana",
        extensions: ["fpx"]
      },
      "image/vnd.fst": {
        source: "iana",
        extensions: ["fst"]
      },
      "image/vnd.fujixerox.edmics-mmr": {
        source: "iana",
        extensions: ["mmr"]
      },
      "image/vnd.fujixerox.edmics-rlc": {
        source: "iana",
        extensions: ["rlc"]
      },
      "image/vnd.globalgraphics.pgb": {
        source: "iana"
      },
      "image/vnd.microsoft.icon": {
        source: "iana",
        compressible: true,
        extensions: ["ico"]
      },
      "image/vnd.mix": {
        source: "iana"
      },
      "image/vnd.mozilla.apng": {
        source: "iana"
      },
      "image/vnd.ms-dds": {
        compressible: true,
        extensions: ["dds"]
      },
      "image/vnd.ms-modi": {
        source: "iana",
        extensions: ["mdi"]
      },
      "image/vnd.ms-photo": {
        source: "apache",
        extensions: ["wdp"]
      },
      "image/vnd.net-fpx": {
        source: "iana",
        extensions: ["npx"]
      },
      "image/vnd.pco.b16": {
        source: "iana",
        extensions: ["b16"]
      },
      "image/vnd.radiance": {
        source: "iana"
      },
      "image/vnd.sealed.png": {
        source: "iana"
      },
      "image/vnd.sealedmedia.softseal.gif": {
        source: "iana"
      },
      "image/vnd.sealedmedia.softseal.jpg": {
        source: "iana"
      },
      "image/vnd.svf": {
        source: "iana"
      },
      "image/vnd.tencent.tap": {
        source: "iana",
        extensions: ["tap"]
      },
      "image/vnd.valve.source.texture": {
        source: "iana",
        extensions: ["vtf"]
      },
      "image/vnd.wap.wbmp": {
        source: "iana",
        extensions: ["wbmp"]
      },
      "image/vnd.xiff": {
        source: "iana",
        extensions: ["xif"]
      },
      "image/vnd.zbrush.pcx": {
        source: "iana",
        extensions: ["pcx"]
      },
      "image/webp": {
        source: "apache",
        extensions: ["webp"]
      },
      "image/wmf": {
        source: "iana",
        extensions: ["wmf"]
      },
      "image/x-3ds": {
        source: "apache",
        extensions: ["3ds"]
      },
      "image/x-cmu-raster": {
        source: "apache",
        extensions: ["ras"]
      },
      "image/x-cmx": {
        source: "apache",
        extensions: ["cmx"]
      },
      "image/x-freehand": {
        source: "apache",
        extensions: ["fh", "fhc", "fh4", "fh5", "fh7"]
      },
      "image/x-icon": {
        source: "apache",
        compressible: true,
        extensions: ["ico"]
      },
      "image/x-jng": {
        source: "nginx",
        extensions: ["jng"]
      },
      "image/x-mrsid-image": {
        source: "apache",
        extensions: ["sid"]
      },
      "image/x-ms-bmp": {
        source: "nginx",
        compressible: true,
        extensions: ["bmp"]
      },
      "image/x-pcx": {
        source: "apache",
        extensions: ["pcx"]
      },
      "image/x-pict": {
        source: "apache",
        extensions: ["pic", "pct"]
      },
      "image/x-portable-anymap": {
        source: "apache",
        extensions: ["pnm"]
      },
      "image/x-portable-bitmap": {
        source: "apache",
        extensions: ["pbm"]
      },
      "image/x-portable-graymap": {
        source: "apache",
        extensions: ["pgm"]
      },
      "image/x-portable-pixmap": {
        source: "apache",
        extensions: ["ppm"]
      },
      "image/x-rgb": {
        source: "apache",
        extensions: ["rgb"]
      },
      "image/x-tga": {
        source: "apache",
        extensions: ["tga"]
      },
      "image/x-xbitmap": {
        source: "apache",
        extensions: ["xbm"]
      },
      "image/x-xcf": {
        compressible: false
      },
      "image/x-xpixmap": {
        source: "apache",
        extensions: ["xpm"]
      },
      "image/x-xwindowdump": {
        source: "apache",
        extensions: ["xwd"]
      },
      "message/cpim": {
        source: "iana"
      },
      "message/delivery-status": {
        source: "iana"
      },
      "message/disposition-notification": {
        source: "iana",
        extensions: [
          "disposition-notification"
        ]
      },
      "message/external-body": {
        source: "iana"
      },
      "message/feedback-report": {
        source: "iana"
      },
      "message/global": {
        source: "iana",
        extensions: ["u8msg"]
      },
      "message/global-delivery-status": {
        source: "iana",
        extensions: ["u8dsn"]
      },
      "message/global-disposition-notification": {
        source: "iana",
        extensions: ["u8mdn"]
      },
      "message/global-headers": {
        source: "iana",
        extensions: ["u8hdr"]
      },
      "message/http": {
        source: "iana",
        compressible: false
      },
      "message/imdn+xml": {
        source: "iana",
        compressible: true
      },
      "message/news": {
        source: "iana"
      },
      "message/partial": {
        source: "iana",
        compressible: false
      },
      "message/rfc822": {
        source: "iana",
        compressible: true,
        extensions: ["eml", "mime"]
      },
      "message/s-http": {
        source: "iana"
      },
      "message/sip": {
        source: "iana"
      },
      "message/sipfrag": {
        source: "iana"
      },
      "message/tracking-status": {
        source: "iana"
      },
      "message/vnd.si.simp": {
        source: "iana"
      },
      "message/vnd.wfa.wsc": {
        source: "iana",
        extensions: ["wsc"]
      },
      "model/3mf": {
        source: "iana",
        extensions: ["3mf"]
      },
      "model/e57": {
        source: "iana"
      },
      "model/gltf+json": {
        source: "iana",
        compressible: true,
        extensions: ["gltf"]
      },
      "model/gltf-binary": {
        source: "iana",
        compressible: true,
        extensions: ["glb"]
      },
      "model/iges": {
        source: "iana",
        compressible: false,
        extensions: ["igs", "iges"]
      },
      "model/mesh": {
        source: "iana",
        compressible: false,
        extensions: ["msh", "mesh", "silo"]
      },
      "model/mtl": {
        source: "iana",
        extensions: ["mtl"]
      },
      "model/obj": {
        source: "iana",
        extensions: ["obj"]
      },
      "model/step": {
        source: "iana"
      },
      "model/step+xml": {
        source: "iana",
        compressible: true,
        extensions: ["stpx"]
      },
      "model/step+zip": {
        source: "iana",
        compressible: false,
        extensions: ["stpz"]
      },
      "model/step-xml+zip": {
        source: "iana",
        compressible: false,
        extensions: ["stpxz"]
      },
      "model/stl": {
        source: "iana",
        extensions: ["stl"]
      },
      "model/vnd.collada+xml": {
        source: "iana",
        compressible: true,
        extensions: ["dae"]
      },
      "model/vnd.dwf": {
        source: "iana",
        extensions: ["dwf"]
      },
      "model/vnd.flatland.3dml": {
        source: "iana"
      },
      "model/vnd.gdl": {
        source: "iana",
        extensions: ["gdl"]
      },
      "model/vnd.gs-gdl": {
        source: "apache"
      },
      "model/vnd.gs.gdl": {
        source: "iana"
      },
      "model/vnd.gtw": {
        source: "iana",
        extensions: ["gtw"]
      },
      "model/vnd.moml+xml": {
        source: "iana",
        compressible: true
      },
      "model/vnd.mts": {
        source: "iana",
        extensions: ["mts"]
      },
      "model/vnd.opengex": {
        source: "iana",
        extensions: ["ogex"]
      },
      "model/vnd.parasolid.transmit.binary": {
        source: "iana",
        extensions: ["x_b"]
      },
      "model/vnd.parasolid.transmit.text": {
        source: "iana",
        extensions: ["x_t"]
      },
      "model/vnd.pytha.pyox": {
        source: "iana"
      },
      "model/vnd.rosette.annotated-data-model": {
        source: "iana"
      },
      "model/vnd.sap.vds": {
        source: "iana",
        extensions: ["vds"]
      },
      "model/vnd.usdz+zip": {
        source: "iana",
        compressible: false,
        extensions: ["usdz"]
      },
      "model/vnd.valve.source.compiled-map": {
        source: "iana",
        extensions: ["bsp"]
      },
      "model/vnd.vtu": {
        source: "iana",
        extensions: ["vtu"]
      },
      "model/vrml": {
        source: "iana",
        compressible: false,
        extensions: ["wrl", "vrml"]
      },
      "model/x3d+binary": {
        source: "apache",
        compressible: false,
        extensions: ["x3db", "x3dbz"]
      },
      "model/x3d+fastinfoset": {
        source: "iana",
        extensions: ["x3db"]
      },
      "model/x3d+vrml": {
        source: "apache",
        compressible: false,
        extensions: ["x3dv", "x3dvz"]
      },
      "model/x3d+xml": {
        source: "iana",
        compressible: true,
        extensions: ["x3d", "x3dz"]
      },
      "model/x3d-vrml": {
        source: "iana",
        extensions: ["x3dv"]
      },
      "multipart/alternative": {
        source: "iana",
        compressible: false
      },
      "multipart/appledouble": {
        source: "iana"
      },
      "multipart/byteranges": {
        source: "iana"
      },
      "multipart/digest": {
        source: "iana"
      },
      "multipart/encrypted": {
        source: "iana",
        compressible: false
      },
      "multipart/form-data": {
        source: "iana",
        compressible: false
      },
      "multipart/header-set": {
        source: "iana"
      },
      "multipart/mixed": {
        source: "iana"
      },
      "multipart/multilingual": {
        source: "iana"
      },
      "multipart/parallel": {
        source: "iana"
      },
      "multipart/related": {
        source: "iana",
        compressible: false
      },
      "multipart/report": {
        source: "iana"
      },
      "multipart/signed": {
        source: "iana",
        compressible: false
      },
      "multipart/vnd.bint.med-plus": {
        source: "iana"
      },
      "multipart/voice-message": {
        source: "iana"
      },
      "multipart/x-mixed-replace": {
        source: "iana"
      },
      "text/1d-interleaved-parityfec": {
        source: "iana"
      },
      "text/cache-manifest": {
        source: "iana",
        compressible: true,
        extensions: ["appcache", "manifest"]
      },
      "text/calendar": {
        source: "iana",
        extensions: ["ics", "ifb"]
      },
      "text/calender": {
        compressible: true
      },
      "text/cmd": {
        compressible: true
      },
      "text/coffeescript": {
        extensions: ["coffee", "litcoffee"]
      },
      "text/cql": {
        source: "iana"
      },
      "text/cql-expression": {
        source: "iana"
      },
      "text/cql-identifier": {
        source: "iana"
      },
      "text/css": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["css"]
      },
      "text/csv": {
        source: "iana",
        compressible: true,
        extensions: ["csv"]
      },
      "text/csv-schema": {
        source: "iana"
      },
      "text/directory": {
        source: "iana"
      },
      "text/dns": {
        source: "iana"
      },
      "text/ecmascript": {
        source: "iana"
      },
      "text/encaprtp": {
        source: "iana"
      },
      "text/enriched": {
        source: "iana"
      },
      "text/fhirpath": {
        source: "iana"
      },
      "text/flexfec": {
        source: "iana"
      },
      "text/fwdred": {
        source: "iana"
      },
      "text/gff3": {
        source: "iana"
      },
      "text/grammar-ref-list": {
        source: "iana"
      },
      "text/html": {
        source: "iana",
        compressible: true,
        extensions: ["html", "htm", "shtml"]
      },
      "text/jade": {
        extensions: ["jade"]
      },
      "text/javascript": {
        source: "iana",
        compressible: true
      },
      "text/jcr-cnd": {
        source: "iana"
      },
      "text/jsx": {
        compressible: true,
        extensions: ["jsx"]
      },
      "text/less": {
        compressible: true,
        extensions: ["less"]
      },
      "text/markdown": {
        source: "iana",
        compressible: true,
        extensions: ["markdown", "md"]
      },
      "text/mathml": {
        source: "nginx",
        extensions: ["mml"]
      },
      "text/mdx": {
        compressible: true,
        extensions: ["mdx"]
      },
      "text/mizar": {
        source: "iana"
      },
      "text/n3": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["n3"]
      },
      "text/parameters": {
        source: "iana",
        charset: "UTF-8"
      },
      "text/parityfec": {
        source: "iana"
      },
      "text/plain": {
        source: "iana",
        compressible: true,
        extensions: ["txt", "text", "conf", "def", "list", "log", "in", "ini"]
      },
      "text/provenance-notation": {
        source: "iana",
        charset: "UTF-8"
      },
      "text/prs.fallenstein.rst": {
        source: "iana"
      },
      "text/prs.lines.tag": {
        source: "iana",
        extensions: ["dsc"]
      },
      "text/prs.prop.logic": {
        source: "iana"
      },
      "text/raptorfec": {
        source: "iana"
      },
      "text/red": {
        source: "iana"
      },
      "text/rfc822-headers": {
        source: "iana"
      },
      "text/richtext": {
        source: "iana",
        compressible: true,
        extensions: ["rtx"]
      },
      "text/rtf": {
        source: "iana",
        compressible: true,
        extensions: ["rtf"]
      },
      "text/rtp-enc-aescm128": {
        source: "iana"
      },
      "text/rtploopback": {
        source: "iana"
      },
      "text/rtx": {
        source: "iana"
      },
      "text/sgml": {
        source: "iana",
        extensions: ["sgml", "sgm"]
      },
      "text/shaclc": {
        source: "iana"
      },
      "text/shex": {
        source: "iana",
        extensions: ["shex"]
      },
      "text/slim": {
        extensions: ["slim", "slm"]
      },
      "text/spdx": {
        source: "iana",
        extensions: ["spdx"]
      },
      "text/strings": {
        source: "iana"
      },
      "text/stylus": {
        extensions: ["stylus", "styl"]
      },
      "text/t140": {
        source: "iana"
      },
      "text/tab-separated-values": {
        source: "iana",
        compressible: true,
        extensions: ["tsv"]
      },
      "text/troff": {
        source: "iana",
        extensions: ["t", "tr", "roff", "man", "me", "ms"]
      },
      "text/turtle": {
        source: "iana",
        charset: "UTF-8",
        extensions: ["ttl"]
      },
      "text/ulpfec": {
        source: "iana"
      },
      "text/uri-list": {
        source: "iana",
        compressible: true,
        extensions: ["uri", "uris", "urls"]
      },
      "text/vcard": {
        source: "iana",
        compressible: true,
        extensions: ["vcard"]
      },
      "text/vnd.a": {
        source: "iana"
      },
      "text/vnd.abc": {
        source: "iana"
      },
      "text/vnd.ascii-art": {
        source: "iana"
      },
      "text/vnd.curl": {
        source: "iana",
        extensions: ["curl"]
      },
      "text/vnd.curl.dcurl": {
        source: "apache",
        extensions: ["dcurl"]
      },
      "text/vnd.curl.mcurl": {
        source: "apache",
        extensions: ["mcurl"]
      },
      "text/vnd.curl.scurl": {
        source: "apache",
        extensions: ["scurl"]
      },
      "text/vnd.debian.copyright": {
        source: "iana",
        charset: "UTF-8"
      },
      "text/vnd.dmclientscript": {
        source: "iana"
      },
      "text/vnd.dvb.subtitle": {
        source: "iana",
        extensions: ["sub"]
      },
      "text/vnd.esmertec.theme-descriptor": {
        source: "iana",
        charset: "UTF-8"
      },
      "text/vnd.familysearch.gedcom": {
        source: "iana",
        extensions: ["ged"]
      },
      "text/vnd.ficlab.flt": {
        source: "iana"
      },
      "text/vnd.fly": {
        source: "iana",
        extensions: ["fly"]
      },
      "text/vnd.fmi.flexstor": {
        source: "iana",
        extensions: ["flx"]
      },
      "text/vnd.gml": {
        source: "iana"
      },
      "text/vnd.graphviz": {
        source: "iana",
        extensions: ["gv"]
      },
      "text/vnd.hans": {
        source: "iana"
      },
      "text/vnd.hgl": {
        source: "iana"
      },
      "text/vnd.in3d.3dml": {
        source: "iana",
        extensions: ["3dml"]
      },
      "text/vnd.in3d.spot": {
        source: "iana",
        extensions: ["spot"]
      },
      "text/vnd.iptc.newsml": {
        source: "iana"
      },
      "text/vnd.iptc.nitf": {
        source: "iana"
      },
      "text/vnd.latex-z": {
        source: "iana"
      },
      "text/vnd.motorola.reflex": {
        source: "iana"
      },
      "text/vnd.ms-mediapackage": {
        source: "iana"
      },
      "text/vnd.net2phone.commcenter.command": {
        source: "iana"
      },
      "text/vnd.radisys.msml-basic-layout": {
        source: "iana"
      },
      "text/vnd.senx.warpscript": {
        source: "iana"
      },
      "text/vnd.si.uricatalogue": {
        source: "iana"
      },
      "text/vnd.sosi": {
        source: "iana"
      },
      "text/vnd.sun.j2me.app-descriptor": {
        source: "iana",
        charset: "UTF-8",
        extensions: ["jad"]
      },
      "text/vnd.trolltech.linguist": {
        source: "iana",
        charset: "UTF-8"
      },
      "text/vnd.wap.si": {
        source: "iana"
      },
      "text/vnd.wap.sl": {
        source: "iana"
      },
      "text/vnd.wap.wml": {
        source: "iana",
        extensions: ["wml"]
      },
      "text/vnd.wap.wmlscript": {
        source: "iana",
        extensions: ["wmls"]
      },
      "text/vtt": {
        source: "iana",
        charset: "UTF-8",
        compressible: true,
        extensions: ["vtt"]
      },
      "text/x-asm": {
        source: "apache",
        extensions: ["s", "asm"]
      },
      "text/x-c": {
        source: "apache",
        extensions: ["c", "cc", "cxx", "cpp", "h", "hh", "dic"]
      },
      "text/x-component": {
        source: "nginx",
        extensions: ["htc"]
      },
      "text/x-fortran": {
        source: "apache",
        extensions: ["f", "for", "f77", "f90"]
      },
      "text/x-gwt-rpc": {
        compressible: true
      },
      "text/x-handlebars-template": {
        extensions: ["hbs"]
      },
      "text/x-java-source": {
        source: "apache",
        extensions: ["java"]
      },
      "text/x-jquery-tmpl": {
        compressible: true
      },
      "text/x-lua": {
        extensions: ["lua"]
      },
      "text/x-markdown": {
        compressible: true,
        extensions: ["mkd"]
      },
      "text/x-nfo": {
        source: "apache",
        extensions: ["nfo"]
      },
      "text/x-opml": {
        source: "apache",
        extensions: ["opml"]
      },
      "text/x-org": {
        compressible: true,
        extensions: ["org"]
      },
      "text/x-pascal": {
        source: "apache",
        extensions: ["p", "pas"]
      },
      "text/x-processing": {
        compressible: true,
        extensions: ["pde"]
      },
      "text/x-sass": {
        extensions: ["sass"]
      },
      "text/x-scss": {
        extensions: ["scss"]
      },
      "text/x-setext": {
        source: "apache",
        extensions: ["etx"]
      },
      "text/x-sfv": {
        source: "apache",
        extensions: ["sfv"]
      },
      "text/x-suse-ymp": {
        compressible: true,
        extensions: ["ymp"]
      },
      "text/x-uuencode": {
        source: "apache",
        extensions: ["uu"]
      },
      "text/x-vcalendar": {
        source: "apache",
        extensions: ["vcs"]
      },
      "text/x-vcard": {
        source: "apache",
        extensions: ["vcf"]
      },
      "text/xml": {
        source: "iana",
        compressible: true,
        extensions: ["xml"]
      },
      "text/xml-external-parsed-entity": {
        source: "iana"
      },
      "text/yaml": {
        compressible: true,
        extensions: ["yaml", "yml"]
      },
      "video/1d-interleaved-parityfec": {
        source: "iana"
      },
      "video/3gpp": {
        source: "iana",
        extensions: ["3gp", "3gpp"]
      },
      "video/3gpp-tt": {
        source: "iana"
      },
      "video/3gpp2": {
        source: "iana",
        extensions: ["3g2"]
      },
      "video/av1": {
        source: "iana"
      },
      "video/bmpeg": {
        source: "iana"
      },
      "video/bt656": {
        source: "iana"
      },
      "video/celb": {
        source: "iana"
      },
      "video/dv": {
        source: "iana"
      },
      "video/encaprtp": {
        source: "iana"
      },
      "video/ffv1": {
        source: "iana"
      },
      "video/flexfec": {
        source: "iana"
      },
      "video/h261": {
        source: "iana",
        extensions: ["h261"]
      },
      "video/h263": {
        source: "iana",
        extensions: ["h263"]
      },
      "video/h263-1998": {
        source: "iana"
      },
      "video/h263-2000": {
        source: "iana"
      },
      "video/h264": {
        source: "iana",
        extensions: ["h264"]
      },
      "video/h264-rcdo": {
        source: "iana"
      },
      "video/h264-svc": {
        source: "iana"
      },
      "video/h265": {
        source: "iana"
      },
      "video/iso.segment": {
        source: "iana",
        extensions: ["m4s"]
      },
      "video/jpeg": {
        source: "iana",
        extensions: ["jpgv"]
      },
      "video/jpeg2000": {
        source: "iana"
      },
      "video/jpm": {
        source: "apache",
        extensions: ["jpm", "jpgm"]
      },
      "video/jxsv": {
        source: "iana"
      },
      "video/mj2": {
        source: "iana",
        extensions: ["mj2", "mjp2"]
      },
      "video/mp1s": {
        source: "iana"
      },
      "video/mp2p": {
        source: "iana"
      },
      "video/mp2t": {
        source: "iana",
        extensions: ["ts"]
      },
      "video/mp4": {
        source: "iana",
        compressible: false,
        extensions: ["mp4", "mp4v", "mpg4"]
      },
      "video/mp4v-es": {
        source: "iana"
      },
      "video/mpeg": {
        source: "iana",
        compressible: false,
        extensions: ["mpeg", "mpg", "mpe", "m1v", "m2v"]
      },
      "video/mpeg4-generic": {
        source: "iana"
      },
      "video/mpv": {
        source: "iana"
      },
      "video/nv": {
        source: "iana"
      },
      "video/ogg": {
        source: "iana",
        compressible: false,
        extensions: ["ogv"]
      },
      "video/parityfec": {
        source: "iana"
      },
      "video/pointer": {
        source: "iana"
      },
      "video/quicktime": {
        source: "iana",
        compressible: false,
        extensions: ["qt", "mov"]
      },
      "video/raptorfec": {
        source: "iana"
      },
      "video/raw": {
        source: "iana"
      },
      "video/rtp-enc-aescm128": {
        source: "iana"
      },
      "video/rtploopback": {
        source: "iana"
      },
      "video/rtx": {
        source: "iana"
      },
      "video/scip": {
        source: "iana"
      },
      "video/smpte291": {
        source: "iana"
      },
      "video/smpte292m": {
        source: "iana"
      },
      "video/ulpfec": {
        source: "iana"
      },
      "video/vc1": {
        source: "iana"
      },
      "video/vc2": {
        source: "iana"
      },
      "video/vnd.cctv": {
        source: "iana"
      },
      "video/vnd.dece.hd": {
        source: "iana",
        extensions: ["uvh", "uvvh"]
      },
      "video/vnd.dece.mobile": {
        source: "iana",
        extensions: ["uvm", "uvvm"]
      },
      "video/vnd.dece.mp4": {
        source: "iana"
      },
      "video/vnd.dece.pd": {
        source: "iana",
        extensions: ["uvp", "uvvp"]
      },
      "video/vnd.dece.sd": {
        source: "iana",
        extensions: ["uvs", "uvvs"]
      },
      "video/vnd.dece.video": {
        source: "iana",
        extensions: ["uvv", "uvvv"]
      },
      "video/vnd.directv.mpeg": {
        source: "iana"
      },
      "video/vnd.directv.mpeg-tts": {
        source: "iana"
      },
      "video/vnd.dlna.mpeg-tts": {
        source: "iana"
      },
      "video/vnd.dvb.file": {
        source: "iana",
        extensions: ["dvb"]
      },
      "video/vnd.fvt": {
        source: "iana",
        extensions: ["fvt"]
      },
      "video/vnd.hns.video": {
        source: "iana"
      },
      "video/vnd.iptvforum.1dparityfec-1010": {
        source: "iana"
      },
      "video/vnd.iptvforum.1dparityfec-2005": {
        source: "iana"
      },
      "video/vnd.iptvforum.2dparityfec-1010": {
        source: "iana"
      },
      "video/vnd.iptvforum.2dparityfec-2005": {
        source: "iana"
      },
      "video/vnd.iptvforum.ttsavc": {
        source: "iana"
      },
      "video/vnd.iptvforum.ttsmpeg2": {
        source: "iana"
      },
      "video/vnd.motorola.video": {
        source: "iana"
      },
      "video/vnd.motorola.videop": {
        source: "iana"
      },
      "video/vnd.mpegurl": {
        source: "iana",
        extensions: ["mxu", "m4u"]
      },
      "video/vnd.ms-playready.media.pyv": {
        source: "iana",
        extensions: ["pyv"]
      },
      "video/vnd.nokia.interleaved-multimedia": {
        source: "iana"
      },
      "video/vnd.nokia.mp4vr": {
        source: "iana"
      },
      "video/vnd.nokia.videovoip": {
        source: "iana"
      },
      "video/vnd.objectvideo": {
        source: "iana"
      },
      "video/vnd.radgamettools.bink": {
        source: "iana"
      },
      "video/vnd.radgamettools.smacker": {
        source: "iana"
      },
      "video/vnd.sealed.mpeg1": {
        source: "iana"
      },
      "video/vnd.sealed.mpeg4": {
        source: "iana"
      },
      "video/vnd.sealed.swf": {
        source: "iana"
      },
      "video/vnd.sealedmedia.softseal.mov": {
        source: "iana"
      },
      "video/vnd.uvvu.mp4": {
        source: "iana",
        extensions: ["uvu", "uvvu"]
      },
      "video/vnd.vivo": {
        source: "iana",
        extensions: ["viv"]
      },
      "video/vnd.youtube.yt": {
        source: "iana"
      },
      "video/vp8": {
        source: "iana"
      },
      "video/vp9": {
        source: "iana"
      },
      "video/webm": {
        source: "apache",
        compressible: false,
        extensions: ["webm"]
      },
      "video/x-f4v": {
        source: "apache",
        extensions: ["f4v"]
      },
      "video/x-fli": {
        source: "apache",
        extensions: ["fli"]
      },
      "video/x-flv": {
        source: "apache",
        compressible: false,
        extensions: ["flv"]
      },
      "video/x-m4v": {
        source: "apache",
        extensions: ["m4v"]
      },
      "video/x-matroska": {
        source: "apache",
        compressible: false,
        extensions: ["mkv", "mk3d", "mks"]
      },
      "video/x-mng": {
        source: "apache",
        extensions: ["mng"]
      },
      "video/x-ms-asf": {
        source: "apache",
        extensions: ["asf", "asx"]
      },
      "video/x-ms-vob": {
        source: "apache",
        extensions: ["vob"]
      },
      "video/x-ms-wm": {
        source: "apache",
        extensions: ["wm"]
      },
      "video/x-ms-wmv": {
        source: "apache",
        compressible: false,
        extensions: ["wmv"]
      },
      "video/x-ms-wmx": {
        source: "apache",
        extensions: ["wmx"]
      },
      "video/x-ms-wvx": {
        source: "apache",
        extensions: ["wvx"]
      },
      "video/x-msvideo": {
        source: "apache",
        extensions: ["avi"]
      },
      "video/x-sgi-movie": {
        source: "apache",
        extensions: ["movie"]
      },
      "video/x-smv": {
        source: "apache",
        extensions: ["smv"]
      },
      "x-conference/x-cooltalk": {
        source: "apache",
        extensions: ["ice"]
      },
      "x-shader/x-fragment": {
        compressible: true
      },
      "x-shader/x-vertex": {
        compressible: true
      }
    };
  }
});

// node_modules/mime-db/index.js
var require_mime_db = __commonJS({
  "node_modules/mime-db/index.js"(exports2, module2) {
    module2.exports = require_db();
  }
});

// node_modules/mime-types/index.js
var require_mime_types = __commonJS({
  "node_modules/mime-types/index.js"(exports2) {
    "use strict";
    var db = require_mime_db();
    var extname = require("path").extname;
    var EXTRACT_TYPE_REGEXP = /^\s*([^;\s]*)(?:;|\s|$)/;
    var TEXT_TYPE_REGEXP = /^text\//i;
    exports2.charset = charset;
    exports2.charsets = { lookup: charset };
    exports2.contentType = contentType;
    exports2.extension = extension;
    exports2.extensions = /* @__PURE__ */ Object.create(null);
    exports2.lookup = lookup;
    exports2.types = /* @__PURE__ */ Object.create(null);
    populateMaps(exports2.extensions, exports2.types);
    function charset(type) {
      if (!type || typeof type !== "string") {
        return false;
      }
      var match = EXTRACT_TYPE_REGEXP.exec(type);
      var mime = match && db[match[1].toLowerCase()];
      if (mime && mime.charset) {
        return mime.charset;
      }
      if (match && TEXT_TYPE_REGEXP.test(match[1])) {
        return "UTF-8";
      }
      return false;
    }
    function contentType(str2) {
      if (!str2 || typeof str2 !== "string") {
        return false;
      }
      var mime = str2.indexOf("/") === -1 ? exports2.lookup(str2) : str2;
      if (!mime) {
        return false;
      }
      if (mime.indexOf("charset") === -1) {
        var charset2 = exports2.charset(mime);
        if (charset2)
          mime += "; charset=" + charset2.toLowerCase();
      }
      return mime;
    }
    function extension(type) {
      if (!type || typeof type !== "string") {
        return false;
      }
      var match = EXTRACT_TYPE_REGEXP.exec(type);
      var exts = match && exports2.extensions[match[1].toLowerCase()];
      if (!exts || !exts.length) {
        return false;
      }
      return exts[0];
    }
    function lookup(path) {
      if (!path || typeof path !== "string") {
        return false;
      }
      var extension2 = extname("x." + path).toLowerCase().substr(1);
      if (!extension2) {
        return false;
      }
      return exports2.types[extension2] || false;
    }
    function populateMaps(extensions, types) {
      var preference = ["nginx", "apache", void 0, "iana"];
      Object.keys(db).forEach(function forEachMimeType(type) {
        var mime = db[type];
        var exts = mime.extensions;
        if (!exts || !exts.length) {
          return;
        }
        extensions[type] = exts;
        for (var i = 0; i < exts.length; i++) {
          var extension2 = exts[i];
          if (types[extension2]) {
            var from = preference.indexOf(db[types[extension2]].source);
            var to = preference.indexOf(mime.source);
            if (types[extension2] !== "application/octet-stream" && (from > to || from === to && types[extension2].substr(0, 12) === "application/")) {
              continue;
            }
          }
          types[extension2] = type;
        }
      });
    }
  }
});

// node_modules/asynckit/lib/defer.js
var require_defer = __commonJS({
  "node_modules/asynckit/lib/defer.js"(exports2, module2) {
    module2.exports = defer;
    function defer(fn) {
      var nextTick = typeof setImmediate == "function" ? setImmediate : typeof process == "object" && typeof process.nextTick == "function" ? process.nextTick : null;
      if (nextTick) {
        nextTick(fn);
      } else {
        setTimeout(fn, 0);
      }
    }
  }
});

// node_modules/asynckit/lib/async.js
var require_async = __commonJS({
  "node_modules/asynckit/lib/async.js"(exports2, module2) {
    var defer = require_defer();
    module2.exports = async;
    function async(callback) {
      var isAsync = false;
      defer(function() {
        isAsync = true;
      });
      return function async_callback(err, result) {
        if (isAsync) {
          callback(err, result);
        } else {
          defer(function nextTick_callback() {
            callback(err, result);
          });
        }
      };
    }
  }
});

// node_modules/asynckit/lib/abort.js
var require_abort = __commonJS({
  "node_modules/asynckit/lib/abort.js"(exports2, module2) {
    module2.exports = abort;
    function abort(state) {
      Object.keys(state.jobs).forEach(clean.bind(state));
      state.jobs = {};
    }
    function clean(key) {
      if (typeof this.jobs[key] == "function") {
        this.jobs[key]();
      }
    }
  }
});

// node_modules/asynckit/lib/iterate.js
var require_iterate = __commonJS({
  "node_modules/asynckit/lib/iterate.js"(exports2, module2) {
    var async = require_async();
    var abort = require_abort();
    module2.exports = iterate;
    function iterate(list, iterator, state, callback) {
      var key = state["keyedList"] ? state["keyedList"][state.index] : state.index;
      state.jobs[key] = runJob(iterator, key, list[key], function(error, output) {
        if (!(key in state.jobs)) {
          return;
        }
        delete state.jobs[key];
        if (error) {
          abort(state);
        } else {
          state.results[key] = output;
        }
        callback(error, state.results);
      });
    }
    function runJob(iterator, key, item, callback) {
      var aborter;
      if (iterator.length == 2) {
        aborter = iterator(item, async(callback));
      } else {
        aborter = iterator(item, key, async(callback));
      }
      return aborter;
    }
  }
});

// node_modules/asynckit/lib/state.js
var require_state = __commonJS({
  "node_modules/asynckit/lib/state.js"(exports2, module2) {
    module2.exports = state;
    function state(list, sortMethod) {
      var isNamedList = !Array.isArray(list), initState = {
        index: 0,
        keyedList: isNamedList || sortMethod ? Object.keys(list) : null,
        jobs: {},
        results: isNamedList ? {} : [],
        size: isNamedList ? Object.keys(list).length : list.length
      };
      if (sortMethod) {
        initState.keyedList.sort(isNamedList ? sortMethod : function(a, b) {
          return sortMethod(list[a], list[b]);
        });
      }
      return initState;
    }
  }
});

// node_modules/asynckit/lib/terminator.js
var require_terminator = __commonJS({
  "node_modules/asynckit/lib/terminator.js"(exports2, module2) {
    var abort = require_abort();
    var async = require_async();
    module2.exports = terminator;
    function terminator(callback) {
      if (!Object.keys(this.jobs).length) {
        return;
      }
      this.index = this.size;
      abort(this);
      async(callback)(null, this.results);
    }
  }
});

// node_modules/asynckit/parallel.js
var require_parallel = __commonJS({
  "node_modules/asynckit/parallel.js"(exports2, module2) {
    var iterate = require_iterate();
    var initState = require_state();
    var terminator = require_terminator();
    module2.exports = parallel;
    function parallel(list, iterator, callback) {
      var state = initState(list);
      while (state.index < (state["keyedList"] || list).length) {
        iterate(list, iterator, state, function(error, result) {
          if (error) {
            callback(error, result);
            return;
          }
          if (Object.keys(state.jobs).length === 0) {
            callback(null, state.results);
            return;
          }
        });
        state.index++;
      }
      return terminator.bind(state, callback);
    }
  }
});

// node_modules/asynckit/serialOrdered.js
var require_serialOrdered = __commonJS({
  "node_modules/asynckit/serialOrdered.js"(exports2, module2) {
    var iterate = require_iterate();
    var initState = require_state();
    var terminator = require_terminator();
    module2.exports = serialOrdered;
    module2.exports.ascending = ascending;
    module2.exports.descending = descending;
    function serialOrdered(list, iterator, sortMethod, callback) {
      var state = initState(list, sortMethod);
      iterate(list, iterator, state, function iteratorHandler(error, result) {
        if (error) {
          callback(error, result);
          return;
        }
        state.index++;
        if (state.index < (state["keyedList"] || list).length) {
          iterate(list, iterator, state, iteratorHandler);
          return;
        }
        callback(null, state.results);
      });
      return terminator.bind(state, callback);
    }
    function ascending(a, b) {
      return a < b ? -1 : a > b ? 1 : 0;
    }
    function descending(a, b) {
      return -1 * ascending(a, b);
    }
  }
});

// node_modules/asynckit/serial.js
var require_serial = __commonJS({
  "node_modules/asynckit/serial.js"(exports2, module2) {
    var serialOrdered = require_serialOrdered();
    module2.exports = serial;
    function serial(list, iterator, callback) {
      return serialOrdered(list, iterator, null, callback);
    }
  }
});

// node_modules/asynckit/index.js
var require_asynckit = __commonJS({
  "node_modules/asynckit/index.js"(exports2, module2) {
    module2.exports = {
      parallel: require_parallel(),
      serial: require_serial(),
      serialOrdered: require_serialOrdered()
    };
  }
});

// node_modules/es-object-atoms/index.js
var require_es_object_atoms = __commonJS({
  "node_modules/es-object-atoms/index.js"(exports2, module2) {
    "use strict";
    module2.exports = Object;
  }
});

// node_modules/es-errors/index.js
var require_es_errors = __commonJS({
  "node_modules/es-errors/index.js"(exports2, module2) {
    "use strict";
    module2.exports = Error;
  }
});

// node_modules/es-errors/eval.js
var require_eval = __commonJS({
  "node_modules/es-errors/eval.js"(exports2, module2) {
    "use strict";
    module2.exports = EvalError;
  }
});

// node_modules/es-errors/range.js
var require_range = __commonJS({
  "node_modules/es-errors/range.js"(exports2, module2) {
    "use strict";
    module2.exports = RangeError;
  }
});

// node_modules/es-errors/ref.js
var require_ref = __commonJS({
  "node_modules/es-errors/ref.js"(exports2, module2) {
    "use strict";
    module2.exports = ReferenceError;
  }
});

// node_modules/es-errors/syntax.js
var require_syntax = __commonJS({
  "node_modules/es-errors/syntax.js"(exports2, module2) {
    "use strict";
    module2.exports = SyntaxError;
  }
});

// node_modules/es-errors/type.js
var require_type = __commonJS({
  "node_modules/es-errors/type.js"(exports2, module2) {
    "use strict";
    module2.exports = TypeError;
  }
});

// node_modules/es-errors/uri.js
var require_uri = __commonJS({
  "node_modules/es-errors/uri.js"(exports2, module2) {
    "use strict";
    module2.exports = URIError;
  }
});

// node_modules/math-intrinsics/abs.js
var require_abs = __commonJS({
  "node_modules/math-intrinsics/abs.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.abs;
  }
});

// node_modules/math-intrinsics/floor.js
var require_floor = __commonJS({
  "node_modules/math-intrinsics/floor.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.floor;
  }
});

// node_modules/math-intrinsics/max.js
var require_max = __commonJS({
  "node_modules/math-intrinsics/max.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.max;
  }
});

// node_modules/math-intrinsics/min.js
var require_min = __commonJS({
  "node_modules/math-intrinsics/min.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.min;
  }
});

// node_modules/math-intrinsics/pow.js
var require_pow = __commonJS({
  "node_modules/math-intrinsics/pow.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.pow;
  }
});

// node_modules/math-intrinsics/round.js
var require_round = __commonJS({
  "node_modules/math-intrinsics/round.js"(exports2, module2) {
    "use strict";
    module2.exports = Math.round;
  }
});

// node_modules/math-intrinsics/isNaN.js
var require_isNaN = __commonJS({
  "node_modules/math-intrinsics/isNaN.js"(exports2, module2) {
    "use strict";
    module2.exports = Number.isNaN || function isNaN2(a) {
      return a !== a;
    };
  }
});

// node_modules/math-intrinsics/sign.js
var require_sign = __commonJS({
  "node_modules/math-intrinsics/sign.js"(exports2, module2) {
    "use strict";
    var $isNaN = require_isNaN();
    module2.exports = function sign(number) {
      if ($isNaN(number) || number === 0) {
        return number;
      }
      return number < 0 ? -1 : 1;
    };
  }
});

// node_modules/gopd/gOPD.js
var require_gOPD = __commonJS({
  "node_modules/gopd/gOPD.js"(exports2, module2) {
    "use strict";
    module2.exports = Object.getOwnPropertyDescriptor;
  }
});

// node_modules/gopd/index.js
var require_gopd = __commonJS({
  "node_modules/gopd/index.js"(exports2, module2) {
    "use strict";
    var $gOPD = require_gOPD();
    if ($gOPD) {
      try {
        $gOPD([], "length");
      } catch (e) {
        $gOPD = null;
      }
    }
    module2.exports = $gOPD;
  }
});

// node_modules/es-define-property/index.js
var require_es_define_property = __commonJS({
  "node_modules/es-define-property/index.js"(exports2, module2) {
    "use strict";
    var $defineProperty = Object.defineProperty || false;
    if ($defineProperty) {
      try {
        $defineProperty({}, "a", { value: 1 });
      } catch (e) {
        $defineProperty = false;
      }
    }
    module2.exports = $defineProperty;
  }
});

// node_modules/has-symbols/shams.js
var require_shams = __commonJS({
  "node_modules/has-symbols/shams.js"(exports2, module2) {
    "use strict";
    module2.exports = function hasSymbols() {
      if (typeof Symbol !== "function" || typeof Object.getOwnPropertySymbols !== "function") {
        return false;
      }
      if (typeof Symbol.iterator === "symbol") {
        return true;
      }
      var obj = {};
      var sym = Symbol("test");
      var symObj = Object(sym);
      if (typeof sym === "string") {
        return false;
      }
      if (Object.prototype.toString.call(sym) !== "[object Symbol]") {
        return false;
      }
      if (Object.prototype.toString.call(symObj) !== "[object Symbol]") {
        return false;
      }
      var symVal = 42;
      obj[sym] = symVal;
      for (var _ in obj) {
        return false;
      }
      if (typeof Object.keys === "function" && Object.keys(obj).length !== 0) {
        return false;
      }
      if (typeof Object.getOwnPropertyNames === "function" && Object.getOwnPropertyNames(obj).length !== 0) {
        return false;
      }
      var syms = Object.getOwnPropertySymbols(obj);
      if (syms.length !== 1 || syms[0] !== sym) {
        return false;
      }
      if (!Object.prototype.propertyIsEnumerable.call(obj, sym)) {
        return false;
      }
      if (typeof Object.getOwnPropertyDescriptor === "function") {
        var descriptor = (
          /** @type {PropertyDescriptor} */
          Object.getOwnPropertyDescriptor(obj, sym)
        );
        if (descriptor.value !== symVal || descriptor.enumerable !== true) {
          return false;
        }
      }
      return true;
    };
  }
});

// node_modules/has-symbols/index.js
var require_has_symbols = __commonJS({
  "node_modules/has-symbols/index.js"(exports2, module2) {
    "use strict";
    var origSymbol = typeof Symbol !== "undefined" && Symbol;
    var hasSymbolSham = require_shams();
    module2.exports = function hasNativeSymbols() {
      if (typeof origSymbol !== "function") {
        return false;
      }
      if (typeof Symbol !== "function") {
        return false;
      }
      if (typeof origSymbol("foo") !== "symbol") {
        return false;
      }
      if (typeof Symbol("bar") !== "symbol") {
        return false;
      }
      return hasSymbolSham();
    };
  }
});

// node_modules/get-proto/Reflect.getPrototypeOf.js
var require_Reflect_getPrototypeOf = __commonJS({
  "node_modules/get-proto/Reflect.getPrototypeOf.js"(exports2, module2) {
    "use strict";
    module2.exports = typeof Reflect !== "undefined" && Reflect.getPrototypeOf || null;
  }
});

// node_modules/get-proto/Object.getPrototypeOf.js
var require_Object_getPrototypeOf = __commonJS({
  "node_modules/get-proto/Object.getPrototypeOf.js"(exports2, module2) {
    "use strict";
    var $Object = require_es_object_atoms();
    module2.exports = $Object.getPrototypeOf || null;
  }
});

// node_modules/function-bind/implementation.js
var require_implementation = __commonJS({
  "node_modules/function-bind/implementation.js"(exports2, module2) {
    "use strict";
    var ERROR_MESSAGE = "Function.prototype.bind called on incompatible ";
    var toStr = Object.prototype.toString;
    var max = Math.max;
    var funcType = "[object Function]";
    var concatty = function concatty2(a, b) {
      var arr = [];
      for (var i = 0; i < a.length; i += 1) {
        arr[i] = a[i];
      }
      for (var j = 0; j < b.length; j += 1) {
        arr[j + a.length] = b[j];
      }
      return arr;
    };
    var slicy = function slicy2(arrLike, offset) {
      var arr = [];
      for (var i = offset || 0, j = 0; i < arrLike.length; i += 1, j += 1) {
        arr[j] = arrLike[i];
      }
      return arr;
    };
    var joiny = function(arr, joiner) {
      var str2 = "";
      for (var i = 0; i < arr.length; i += 1) {
        str2 += arr[i];
        if (i + 1 < arr.length) {
          str2 += joiner;
        }
      }
      return str2;
    };
    module2.exports = function bind(that) {
      var target = this;
      if (typeof target !== "function" || toStr.apply(target) !== funcType) {
        throw new TypeError(ERROR_MESSAGE + target);
      }
      var args2 = slicy(arguments, 1);
      var bound;
      var binder = function() {
        if (this instanceof bound) {
          var result = target.apply(
            this,
            concatty(args2, arguments)
          );
          if (Object(result) === result) {
            return result;
          }
          return this;
        }
        return target.apply(
          that,
          concatty(args2, arguments)
        );
      };
      var boundLength = max(0, target.length - args2.length);
      var boundArgs = [];
      for (var i = 0; i < boundLength; i++) {
        boundArgs[i] = "$" + i;
      }
      bound = Function("binder", "return function (" + joiny(boundArgs, ",") + "){ return binder.apply(this,arguments); }")(binder);
      if (target.prototype) {
        var Empty = function Empty2() {
        };
        Empty.prototype = target.prototype;
        bound.prototype = new Empty();
        Empty.prototype = null;
      }
      return bound;
    };
  }
});

// node_modules/function-bind/index.js
var require_function_bind = __commonJS({
  "node_modules/function-bind/index.js"(exports2, module2) {
    "use strict";
    var implementation = require_implementation();
    module2.exports = Function.prototype.bind || implementation;
  }
});

// node_modules/call-bind-apply-helpers/functionCall.js
var require_functionCall = __commonJS({
  "node_modules/call-bind-apply-helpers/functionCall.js"(exports2, module2) {
    "use strict";
    module2.exports = Function.prototype.call;
  }
});

// node_modules/call-bind-apply-helpers/functionApply.js
var require_functionApply = __commonJS({
  "node_modules/call-bind-apply-helpers/functionApply.js"(exports2, module2) {
    "use strict";
    module2.exports = Function.prototype.apply;
  }
});

// node_modules/call-bind-apply-helpers/reflectApply.js
var require_reflectApply = __commonJS({
  "node_modules/call-bind-apply-helpers/reflectApply.js"(exports2, module2) {
    "use strict";
    module2.exports = typeof Reflect !== "undefined" && Reflect && Reflect.apply;
  }
});

// node_modules/call-bind-apply-helpers/actualApply.js
var require_actualApply = __commonJS({
  "node_modules/call-bind-apply-helpers/actualApply.js"(exports2, module2) {
    "use strict";
    var bind = require_function_bind();
    var $apply = require_functionApply();
    var $call = require_functionCall();
    var $reflectApply = require_reflectApply();
    module2.exports = $reflectApply || bind.call($call, $apply);
  }
});

// node_modules/call-bind-apply-helpers/index.js
var require_call_bind_apply_helpers = __commonJS({
  "node_modules/call-bind-apply-helpers/index.js"(exports2, module2) {
    "use strict";
    var bind = require_function_bind();
    var $TypeError = require_type();
    var $call = require_functionCall();
    var $actualApply = require_actualApply();
    module2.exports = function callBindBasic(args2) {
      if (args2.length < 1 || typeof args2[0] !== "function") {
        throw new $TypeError("a function is required");
      }
      return $actualApply(bind, $call, args2);
    };
  }
});

// node_modules/dunder-proto/get.js
var require_get = __commonJS({
  "node_modules/dunder-proto/get.js"(exports2, module2) {
    "use strict";
    var callBind = require_call_bind_apply_helpers();
    var gOPD = require_gopd();
    var hasProtoAccessor;
    try {
      hasProtoAccessor = /** @type {{ __proto__?: typeof Array.prototype }} */
      [].__proto__ === Array.prototype;
    } catch (e) {
      if (!e || typeof e !== "object" || !("code" in e) || e.code !== "ERR_PROTO_ACCESS") {
        throw e;
      }
    }
    var desc = !!hasProtoAccessor && gOPD && gOPD(
      Object.prototype,
      /** @type {keyof typeof Object.prototype} */
      "__proto__"
    );
    var $Object = Object;
    var $getPrototypeOf = $Object.getPrototypeOf;
    module2.exports = desc && typeof desc.get === "function" ? callBind([desc.get]) : typeof $getPrototypeOf === "function" ? (
      /** @type {import('./get')} */
      function getDunder(value) {
        return $getPrototypeOf(value == null ? value : $Object(value));
      }
    ) : false;
  }
});

// node_modules/get-proto/index.js
var require_get_proto = __commonJS({
  "node_modules/get-proto/index.js"(exports2, module2) {
    "use strict";
    var reflectGetProto = require_Reflect_getPrototypeOf();
    var originalGetProto = require_Object_getPrototypeOf();
    var getDunderProto = require_get();
    module2.exports = reflectGetProto ? function getProto(O) {
      return reflectGetProto(O);
    } : originalGetProto ? function getProto(O) {
      if (!O || typeof O !== "object" && typeof O !== "function") {
        throw new TypeError("getProto: not an object");
      }
      return originalGetProto(O);
    } : getDunderProto ? function getProto(O) {
      return getDunderProto(O);
    } : null;
  }
});

// node_modules/hasown/index.js
var require_hasown = __commonJS({
  "node_modules/hasown/index.js"(exports2, module2) {
    "use strict";
    var call = Function.prototype.call;
    var $hasOwn = Object.prototype.hasOwnProperty;
    var bind = require_function_bind();
    module2.exports = bind.call(call, $hasOwn);
  }
});

// node_modules/get-intrinsic/index.js
var require_get_intrinsic = __commonJS({
  "node_modules/get-intrinsic/index.js"(exports2, module2) {
    "use strict";
    var undefined2;
    var $Object = require_es_object_atoms();
    var $Error = require_es_errors();
    var $EvalError = require_eval();
    var $RangeError = require_range();
    var $ReferenceError = require_ref();
    var $SyntaxError = require_syntax();
    var $TypeError = require_type();
    var $URIError = require_uri();
    var abs = require_abs();
    var floor = require_floor();
    var max = require_max();
    var min = require_min();
    var pow = require_pow();
    var round = require_round();
    var sign = require_sign();
    var $Function = Function;
    var getEvalledConstructor = function(expressionSyntax) {
      try {
        return $Function('"use strict"; return (' + expressionSyntax + ").constructor;")();
      } catch (e) {
      }
    };
    var $gOPD = require_gopd();
    var $defineProperty = require_es_define_property();
    var throwTypeError = function() {
      throw new $TypeError();
    };
    var ThrowTypeError = $gOPD ? function() {
      try {
        arguments.callee;
        return throwTypeError;
      } catch (calleeThrows) {
        try {
          return $gOPD(arguments, "callee").get;
        } catch (gOPDthrows) {
          return throwTypeError;
        }
      }
    }() : throwTypeError;
    var hasSymbols = require_has_symbols()();
    var getProto = require_get_proto();
    var $ObjectGPO = require_Object_getPrototypeOf();
    var $ReflectGPO = require_Reflect_getPrototypeOf();
    var $apply = require_functionApply();
    var $call = require_functionCall();
    var needsEval = {};
    var TypedArray = typeof Uint8Array === "undefined" || !getProto ? undefined2 : getProto(Uint8Array);
    var INTRINSICS = {
      __proto__: null,
      "%AggregateError%": typeof AggregateError === "undefined" ? undefined2 : AggregateError,
      "%Array%": Array,
      "%ArrayBuffer%": typeof ArrayBuffer === "undefined" ? undefined2 : ArrayBuffer,
      "%ArrayIteratorPrototype%": hasSymbols && getProto ? getProto([][Symbol.iterator]()) : undefined2,
      "%AsyncFromSyncIteratorPrototype%": undefined2,
      "%AsyncFunction%": needsEval,
      "%AsyncGenerator%": needsEval,
      "%AsyncGeneratorFunction%": needsEval,
      "%AsyncIteratorPrototype%": needsEval,
      "%Atomics%": typeof Atomics === "undefined" ? undefined2 : Atomics,
      "%BigInt%": typeof BigInt === "undefined" ? undefined2 : BigInt,
      "%BigInt64Array%": typeof BigInt64Array === "undefined" ? undefined2 : BigInt64Array,
      "%BigUint64Array%": typeof BigUint64Array === "undefined" ? undefined2 : BigUint64Array,
      "%Boolean%": Boolean,
      "%DataView%": typeof DataView === "undefined" ? undefined2 : DataView,
      "%Date%": Date,
      "%decodeURI%": decodeURI,
      "%decodeURIComponent%": decodeURIComponent,
      "%encodeURI%": encodeURI,
      "%encodeURIComponent%": encodeURIComponent,
      "%Error%": $Error,
      "%eval%": eval,
      // eslint-disable-line no-eval
      "%EvalError%": $EvalError,
      "%Float16Array%": typeof Float16Array === "undefined" ? undefined2 : Float16Array,
      "%Float32Array%": typeof Float32Array === "undefined" ? undefined2 : Float32Array,
      "%Float64Array%": typeof Float64Array === "undefined" ? undefined2 : Float64Array,
      "%FinalizationRegistry%": typeof FinalizationRegistry === "undefined" ? undefined2 : FinalizationRegistry,
      "%Function%": $Function,
      "%GeneratorFunction%": needsEval,
      "%Int8Array%": typeof Int8Array === "undefined" ? undefined2 : Int8Array,
      "%Int16Array%": typeof Int16Array === "undefined" ? undefined2 : Int16Array,
      "%Int32Array%": typeof Int32Array === "undefined" ? undefined2 : Int32Array,
      "%isFinite%": isFinite,
      "%isNaN%": isNaN,
      "%IteratorPrototype%": hasSymbols && getProto ? getProto(getProto([][Symbol.iterator]())) : undefined2,
      "%JSON%": typeof JSON === "object" ? JSON : undefined2,
      "%Map%": typeof Map === "undefined" ? undefined2 : Map,
      "%MapIteratorPrototype%": typeof Map === "undefined" || !hasSymbols || !getProto ? undefined2 : getProto((/* @__PURE__ */ new Map())[Symbol.iterator]()),
      "%Math%": Math,
      "%Number%": Number,
      "%Object%": $Object,
      "%Object.getOwnPropertyDescriptor%": $gOPD,
      "%parseFloat%": parseFloat,
      "%parseInt%": parseInt,
      "%Promise%": typeof Promise === "undefined" ? undefined2 : Promise,
      "%Proxy%": typeof Proxy === "undefined" ? undefined2 : Proxy,
      "%RangeError%": $RangeError,
      "%ReferenceError%": $ReferenceError,
      "%Reflect%": typeof Reflect === "undefined" ? undefined2 : Reflect,
      "%RegExp%": RegExp,
      "%Set%": typeof Set === "undefined" ? undefined2 : Set,
      "%SetIteratorPrototype%": typeof Set === "undefined" || !hasSymbols || !getProto ? undefined2 : getProto((/* @__PURE__ */ new Set())[Symbol.iterator]()),
      "%SharedArrayBuffer%": typeof SharedArrayBuffer === "undefined" ? undefined2 : SharedArrayBuffer,
      "%String%": String,
      "%StringIteratorPrototype%": hasSymbols && getProto ? getProto(""[Symbol.iterator]()) : undefined2,
      "%Symbol%": hasSymbols ? Symbol : undefined2,
      "%SyntaxError%": $SyntaxError,
      "%ThrowTypeError%": ThrowTypeError,
      "%TypedArray%": TypedArray,
      "%TypeError%": $TypeError,
      "%Uint8Array%": typeof Uint8Array === "undefined" ? undefined2 : Uint8Array,
      "%Uint8ClampedArray%": typeof Uint8ClampedArray === "undefined" ? undefined2 : Uint8ClampedArray,
      "%Uint16Array%": typeof Uint16Array === "undefined" ? undefined2 : Uint16Array,
      "%Uint32Array%": typeof Uint32Array === "undefined" ? undefined2 : Uint32Array,
      "%URIError%": $URIError,
      "%WeakMap%": typeof WeakMap === "undefined" ? undefined2 : WeakMap,
      "%WeakRef%": typeof WeakRef === "undefined" ? undefined2 : WeakRef,
      "%WeakSet%": typeof WeakSet === "undefined" ? undefined2 : WeakSet,
      "%Function.prototype.call%": $call,
      "%Function.prototype.apply%": $apply,
      "%Object.defineProperty%": $defineProperty,
      "%Object.getPrototypeOf%": $ObjectGPO,
      "%Math.abs%": abs,
      "%Math.floor%": floor,
      "%Math.max%": max,
      "%Math.min%": min,
      "%Math.pow%": pow,
      "%Math.round%": round,
      "%Math.sign%": sign,
      "%Reflect.getPrototypeOf%": $ReflectGPO
    };
    if (getProto) {
      try {
        null.error;
      } catch (e) {
        errorProto = getProto(getProto(e));
        INTRINSICS["%Error.prototype%"] = errorProto;
      }
    }
    var errorProto;
    var doEval = function doEval2(name) {
      var value;
      if (name === "%AsyncFunction%") {
        value = getEvalledConstructor("async function () {}");
      } else if (name === "%GeneratorFunction%") {
        value = getEvalledConstructor("function* () {}");
      } else if (name === "%AsyncGeneratorFunction%") {
        value = getEvalledConstructor("async function* () {}");
      } else if (name === "%AsyncGenerator%") {
        var fn = doEval2("%AsyncGeneratorFunction%");
        if (fn) {
          value = fn.prototype;
        }
      } else if (name === "%AsyncIteratorPrototype%") {
        var gen = doEval2("%AsyncGenerator%");
        if (gen && getProto) {
          value = getProto(gen.prototype);
        }
      }
      INTRINSICS[name] = value;
      return value;
    };
    var LEGACY_ALIASES = {
      __proto__: null,
      "%ArrayBufferPrototype%": ["ArrayBuffer", "prototype"],
      "%ArrayPrototype%": ["Array", "prototype"],
      "%ArrayProto_entries%": ["Array", "prototype", "entries"],
      "%ArrayProto_forEach%": ["Array", "prototype", "forEach"],
      "%ArrayProto_keys%": ["Array", "prototype", "keys"],
      "%ArrayProto_values%": ["Array", "prototype", "values"],
      "%AsyncFunctionPrototype%": ["AsyncFunction", "prototype"],
      "%AsyncGenerator%": ["AsyncGeneratorFunction", "prototype"],
      "%AsyncGeneratorPrototype%": ["AsyncGeneratorFunction", "prototype", "prototype"],
      "%BooleanPrototype%": ["Boolean", "prototype"],
      "%DataViewPrototype%": ["DataView", "prototype"],
      "%DatePrototype%": ["Date", "prototype"],
      "%ErrorPrototype%": ["Error", "prototype"],
      "%EvalErrorPrototype%": ["EvalError", "prototype"],
      "%Float32ArrayPrototype%": ["Float32Array", "prototype"],
      "%Float64ArrayPrototype%": ["Float64Array", "prototype"],
      "%FunctionPrototype%": ["Function", "prototype"],
      "%Generator%": ["GeneratorFunction", "prototype"],
      "%GeneratorPrototype%": ["GeneratorFunction", "prototype", "prototype"],
      "%Int8ArrayPrototype%": ["Int8Array", "prototype"],
      "%Int16ArrayPrototype%": ["Int16Array", "prototype"],
      "%Int32ArrayPrototype%": ["Int32Array", "prototype"],
      "%JSONParse%": ["JSON", "parse"],
      "%JSONStringify%": ["JSON", "stringify"],
      "%MapPrototype%": ["Map", "prototype"],
      "%NumberPrototype%": ["Number", "prototype"],
      "%ObjectPrototype%": ["Object", "prototype"],
      "%ObjProto_toString%": ["Object", "prototype", "toString"],
      "%ObjProto_valueOf%": ["Object", "prototype", "valueOf"],
      "%PromisePrototype%": ["Promise", "prototype"],
      "%PromiseProto_then%": ["Promise", "prototype", "then"],
      "%Promise_all%": ["Promise", "all"],
      "%Promise_reject%": ["Promise", "reject"],
      "%Promise_resolve%": ["Promise", "resolve"],
      "%RangeErrorPrototype%": ["RangeError", "prototype"],
      "%ReferenceErrorPrototype%": ["ReferenceError", "prototype"],
      "%RegExpPrototype%": ["RegExp", "prototype"],
      "%SetPrototype%": ["Set", "prototype"],
      "%SharedArrayBufferPrototype%": ["SharedArrayBuffer", "prototype"],
      "%StringPrototype%": ["String", "prototype"],
      "%SymbolPrototype%": ["Symbol", "prototype"],
      "%SyntaxErrorPrototype%": ["SyntaxError", "prototype"],
      "%TypedArrayPrototype%": ["TypedArray", "prototype"],
      "%TypeErrorPrototype%": ["TypeError", "prototype"],
      "%Uint8ArrayPrototype%": ["Uint8Array", "prototype"],
      "%Uint8ClampedArrayPrototype%": ["Uint8ClampedArray", "prototype"],
      "%Uint16ArrayPrototype%": ["Uint16Array", "prototype"],
      "%Uint32ArrayPrototype%": ["Uint32Array", "prototype"],
      "%URIErrorPrototype%": ["URIError", "prototype"],
      "%WeakMapPrototype%": ["WeakMap", "prototype"],
      "%WeakSetPrototype%": ["WeakSet", "prototype"]
    };
    var bind = require_function_bind();
    var hasOwn = require_hasown();
    var $concat = bind.call($call, Array.prototype.concat);
    var $spliceApply = bind.call($apply, Array.prototype.splice);
    var $replace = bind.call($call, String.prototype.replace);
    var $strSlice = bind.call($call, String.prototype.slice);
    var $exec = bind.call($call, RegExp.prototype.exec);
    var rePropName = /[^%.[\]]+|\[(?:(-?\d+(?:\.\d+)?)|(["'])((?:(?!\2)[^\\]|\\.)*?)\2)\]|(?=(?:\.|\[\])(?:\.|\[\]|%$))/g;
    var reEscapeChar = /\\(\\)?/g;
    var stringToPath = function stringToPath2(string) {
      var first = $strSlice(string, 0, 1);
      var last = $strSlice(string, -1);
      if (first === "%" && last !== "%") {
        throw new $SyntaxError("invalid intrinsic syntax, expected closing `%`");
      } else if (last === "%" && first !== "%") {
        throw new $SyntaxError("invalid intrinsic syntax, expected opening `%`");
      }
      var result = [];
      $replace(string, rePropName, function(match, number, quote, subString) {
        result[result.length] = quote ? $replace(subString, reEscapeChar, "$1") : number || match;
      });
      return result;
    };
    var getBaseIntrinsic = function getBaseIntrinsic2(name, allowMissing) {
      var intrinsicName = name;
      var alias;
      if (hasOwn(LEGACY_ALIASES, intrinsicName)) {
        alias = LEGACY_ALIASES[intrinsicName];
        intrinsicName = "%" + alias[0] + "%";
      }
      if (hasOwn(INTRINSICS, intrinsicName)) {
        var value = INTRINSICS[intrinsicName];
        if (value === needsEval) {
          value = doEval(intrinsicName);
        }
        if (typeof value === "undefined" && !allowMissing) {
          throw new $TypeError("intrinsic " + name + " exists, but is not available. Please file an issue!");
        }
        return {
          alias,
          name: intrinsicName,
          value
        };
      }
      throw new $SyntaxError("intrinsic " + name + " does not exist!");
    };
    module2.exports = function GetIntrinsic(name, allowMissing) {
      if (typeof name !== "string" || name.length === 0) {
        throw new $TypeError("intrinsic name must be a non-empty string");
      }
      if (arguments.length > 1 && typeof allowMissing !== "boolean") {
        throw new $TypeError('"allowMissing" argument must be a boolean');
      }
      if ($exec(/^%?[^%]*%?$/, name) === null) {
        throw new $SyntaxError("`%` may not be present anywhere but at the beginning and end of the intrinsic name");
      }
      var parts = stringToPath(name);
      var intrinsicBaseName = parts.length > 0 ? parts[0] : "";
      var intrinsic = getBaseIntrinsic("%" + intrinsicBaseName + "%", allowMissing);
      var intrinsicRealName = intrinsic.name;
      var value = intrinsic.value;
      var skipFurtherCaching = false;
      var alias = intrinsic.alias;
      if (alias) {
        intrinsicBaseName = alias[0];
        $spliceApply(parts, $concat([0, 1], alias));
      }
      for (var i = 1, isOwn = true; i < parts.length; i += 1) {
        var part = parts[i];
        var first = $strSlice(part, 0, 1);
        var last = $strSlice(part, -1);
        if ((first === '"' || first === "'" || first === "`" || (last === '"' || last === "'" || last === "`")) && first !== last) {
          throw new $SyntaxError("property names with quotes must have matching quotes");
        }
        if (part === "constructor" || !isOwn) {
          skipFurtherCaching = true;
        }
        intrinsicBaseName += "." + part;
        intrinsicRealName = "%" + intrinsicBaseName + "%";
        if (hasOwn(INTRINSICS, intrinsicRealName)) {
          value = INTRINSICS[intrinsicRealName];
        } else if (value != null) {
          if (!(part in value)) {
            if (!allowMissing) {
              throw new $TypeError("base intrinsic for " + name + " exists, but the property is not available.");
            }
            return void 0;
          }
          if ($gOPD && i + 1 >= parts.length) {
            var desc = $gOPD(value, part);
            isOwn = !!desc;
            if (isOwn && "get" in desc && !("originalValue" in desc.get)) {
              value = desc.get;
            } else {
              value = value[part];
            }
          } else {
            isOwn = hasOwn(value, part);
            value = value[part];
          }
          if (isOwn && !skipFurtherCaching) {
            INTRINSICS[intrinsicRealName] = value;
          }
        }
      }
      return value;
    };
  }
});

// node_modules/has-tostringtag/shams.js
var require_shams2 = __commonJS({
  "node_modules/has-tostringtag/shams.js"(exports2, module2) {
    "use strict";
    var hasSymbols = require_shams();
    module2.exports = function hasToStringTagShams() {
      return hasSymbols() && !!Symbol.toStringTag;
    };
  }
});

// node_modules/es-set-tostringtag/index.js
var require_es_set_tostringtag = __commonJS({
  "node_modules/es-set-tostringtag/index.js"(exports2, module2) {
    "use strict";
    var GetIntrinsic = require_get_intrinsic();
    var $defineProperty = GetIntrinsic("%Object.defineProperty%", true);
    var hasToStringTag = require_shams2()();
    var hasOwn = require_hasown();
    var $TypeError = require_type();
    var toStringTag = hasToStringTag ? Symbol.toStringTag : null;
    module2.exports = function setToStringTag(object, value) {
      var overrideIfSet = arguments.length > 2 && !!arguments[2] && arguments[2].force;
      var nonConfigurable = arguments.length > 2 && !!arguments[2] && arguments[2].nonConfigurable;
      if (typeof overrideIfSet !== "undefined" && typeof overrideIfSet !== "boolean" || typeof nonConfigurable !== "undefined" && typeof nonConfigurable !== "boolean") {
        throw new $TypeError("if provided, the `overrideIfSet` and `nonConfigurable` options must be booleans");
      }
      if (toStringTag && (overrideIfSet || !hasOwn(object, toStringTag))) {
        if ($defineProperty) {
          $defineProperty(object, toStringTag, {
            configurable: !nonConfigurable,
            enumerable: false,
            value,
            writable: false
          });
        } else {
          object[toStringTag] = value;
        }
      }
    };
  }
});

// node_modules/form-data/lib/populate.js
var require_populate = __commonJS({
  "node_modules/form-data/lib/populate.js"(exports2, module2) {
    "use strict";
    module2.exports = function(dst, src) {
      Object.keys(src).forEach(function(prop) {
        dst[prop] = dst[prop] || src[prop];
      });
      return dst;
    };
  }
});

// node_modules/form-data/lib/form_data.js
var require_form_data = __commonJS({
  "node_modules/form-data/lib/form_data.js"(exports2, module2) {
    "use strict";
    var CombinedStream = require_combined_stream();
    var util = require("util");
    var path = require("path");
    var http = require("http");
    var https = require("https");
    var parseUrl = require("url").parse;
    var fs = require("fs");
    var Stream = require("stream").Stream;
    var crypto = require("crypto");
    var mime = require_mime_types();
    var asynckit = require_asynckit();
    var setToStringTag = require_es_set_tostringtag();
    var hasOwn = require_hasown();
    var populate = require_populate();
    function FormData2(options) {
      if (!(this instanceof FormData2)) {
        return new FormData2(options);
      }
      this._overheadLength = 0;
      this._valueLength = 0;
      this._valuesToMeasure = [];
      CombinedStream.call(this);
      options = options || {};
      for (var option in options) {
        this[option] = options[option];
      }
    }
    util.inherits(FormData2, CombinedStream);
    FormData2.LINE_BREAK = "\r\n";
    FormData2.DEFAULT_CONTENT_TYPE = "application/octet-stream";
    FormData2.prototype.append = function(field, value, options) {
      options = options || {};
      if (typeof options === "string") {
        options = { filename: options };
      }
      var append = CombinedStream.prototype.append.bind(this);
      if (typeof value === "number" || value == null) {
        value = String(value);
      }
      if (Array.isArray(value)) {
        this._error(new Error("Arrays are not supported."));
        return;
      }
      var header = this._multiPartHeader(field, value, options);
      var footer = this._multiPartFooter();
      append(header);
      append(value);
      append(footer);
      this._trackLength(header, value, options);
    };
    FormData2.prototype._trackLength = function(header, value, options) {
      var valueLength = 0;
      if (options.knownLength != null) {
        valueLength += Number(options.knownLength);
      } else if (Buffer.isBuffer(value)) {
        valueLength = value.length;
      } else if (typeof value === "string") {
        valueLength = Buffer.byteLength(value);
      }
      this._valueLength += valueLength;
      this._overheadLength += Buffer.byteLength(header) + FormData2.LINE_BREAK.length;
      if (!value || !value.path && !(value.readable && hasOwn(value, "httpVersion")) && !(value instanceof Stream)) {
        return;
      }
      if (!options.knownLength) {
        this._valuesToMeasure.push(value);
      }
    };
    FormData2.prototype._lengthRetriever = function(value, callback) {
      if (hasOwn(value, "fd")) {
        if (value.end != void 0 && value.end != Infinity && value.start != void 0) {
          callback(null, value.end + 1 - (value.start ? value.start : 0));
        } else {
          fs.stat(value.path, function(err, stat) {
            if (err) {
              callback(err);
              return;
            }
            var fileSize = stat.size - (value.start ? value.start : 0);
            callback(null, fileSize);
          });
        }
      } else if (hasOwn(value, "httpVersion")) {
        callback(null, Number(value.headers["content-length"]));
      } else if (hasOwn(value, "httpModule")) {
        value.on("response", function(response) {
          value.pause();
          callback(null, Number(response.headers["content-length"]));
        });
        value.resume();
      } else {
        callback("Unknown stream");
      }
    };
    FormData2.prototype._multiPartHeader = function(field, value, options) {
      if (typeof options.header === "string") {
        return options.header;
      }
      var contentDisposition = this._getContentDisposition(value, options);
      var contentType = this._getContentType(value, options);
      var contents = "";
      var headers = {
        // add custom disposition as third element or keep it two elements if not
        "Content-Disposition": ["form-data", 'name="' + field + '"'].concat(contentDisposition || []),
        // if no content type. allow it to be empty array
        "Content-Type": [].concat(contentType || [])
      };
      if (typeof options.header === "object") {
        populate(headers, options.header);
      }
      var header;
      for (var prop in headers) {
        if (hasOwn(headers, prop)) {
          header = headers[prop];
          if (header == null) {
            continue;
          }
          if (!Array.isArray(header)) {
            header = [header];
          }
          if (header.length) {
            contents += prop + ": " + header.join("; ") + FormData2.LINE_BREAK;
          }
        }
      }
      return "--" + this.getBoundary() + FormData2.LINE_BREAK + contents + FormData2.LINE_BREAK;
    };
    FormData2.prototype._getContentDisposition = function(value, options) {
      var filename;
      if (typeof options.filepath === "string") {
        filename = path.normalize(options.filepath).replace(/\\/g, "/");
      } else if (options.filename || value && (value.name || value.path)) {
        filename = path.basename(options.filename || value && (value.name || value.path));
      } else if (value && value.readable && hasOwn(value, "httpVersion")) {
        filename = path.basename(value.client._httpMessage.path || "");
      }
      if (filename) {
        return 'filename="' + filename + '"';
      }
    };
    FormData2.prototype._getContentType = function(value, options) {
      var contentType = options.contentType;
      if (!contentType && value && value.name) {
        contentType = mime.lookup(value.name);
      }
      if (!contentType && value && value.path) {
        contentType = mime.lookup(value.path);
      }
      if (!contentType && value && value.readable && hasOwn(value, "httpVersion")) {
        contentType = value.headers["content-type"];
      }
      if (!contentType && (options.filepath || options.filename)) {
        contentType = mime.lookup(options.filepath || options.filename);
      }
      if (!contentType && value && typeof value === "object") {
        contentType = FormData2.DEFAULT_CONTENT_TYPE;
      }
      return contentType;
    };
    FormData2.prototype._multiPartFooter = function() {
      return function(next) {
        var footer = FormData2.LINE_BREAK;
        var lastPart = this._streams.length === 0;
        if (lastPart) {
          footer += this._lastBoundary();
        }
        next(footer);
      }.bind(this);
    };
    FormData2.prototype._lastBoundary = function() {
      return "--" + this.getBoundary() + "--" + FormData2.LINE_BREAK;
    };
    FormData2.prototype.getHeaders = function(userHeaders) {
      var header;
      var formHeaders = {
        "content-type": "multipart/form-data; boundary=" + this.getBoundary()
      };
      for (header in userHeaders) {
        if (hasOwn(userHeaders, header)) {
          formHeaders[header.toLowerCase()] = userHeaders[header];
        }
      }
      return formHeaders;
    };
    FormData2.prototype.setBoundary = function(boundary) {
      if (typeof boundary !== "string") {
        throw new TypeError("FormData boundary must be a string");
      }
      this._boundary = boundary;
    };
    FormData2.prototype.getBoundary = function() {
      if (!this._boundary) {
        this._generateBoundary();
      }
      return this._boundary;
    };
    FormData2.prototype.getBuffer = function() {
      var dataBuffer = new Buffer.alloc(0);
      var boundary = this.getBoundary();
      for (var i = 0, len = this._streams.length; i < len; i++) {
        if (typeof this._streams[i] !== "function") {
          if (Buffer.isBuffer(this._streams[i])) {
            dataBuffer = Buffer.concat([dataBuffer, this._streams[i]]);
          } else {
            dataBuffer = Buffer.concat([dataBuffer, Buffer.from(this._streams[i])]);
          }
          if (typeof this._streams[i] !== "string" || this._streams[i].substring(2, boundary.length + 2) !== boundary) {
            dataBuffer = Buffer.concat([dataBuffer, Buffer.from(FormData2.LINE_BREAK)]);
          }
        }
      }
      return Buffer.concat([dataBuffer, Buffer.from(this._lastBoundary())]);
    };
    FormData2.prototype._generateBoundary = function() {
      this._boundary = "--------------------------" + crypto.randomBytes(12).toString("hex");
    };
    FormData2.prototype.getLengthSync = function() {
      var knownLength = this._overheadLength + this._valueLength;
      if (this._streams.length) {
        knownLength += this._lastBoundary().length;
      }
      if (!this.hasKnownLength()) {
        this._error(new Error("Cannot calculate proper length in synchronous way."));
      }
      return knownLength;
    };
    FormData2.prototype.hasKnownLength = function() {
      var hasKnownLength = true;
      if (this._valuesToMeasure.length) {
        hasKnownLength = false;
      }
      return hasKnownLength;
    };
    FormData2.prototype.getLength = function(cb) {
      var knownLength = this._overheadLength + this._valueLength;
      if (this._streams.length) {
        knownLength += this._lastBoundary().length;
      }
      if (!this._valuesToMeasure.length) {
        process.nextTick(cb.bind(this, null, knownLength));
        return;
      }
      asynckit.parallel(this._valuesToMeasure, this._lengthRetriever, function(err, values) {
        if (err) {
          cb(err);
          return;
        }
        values.forEach(function(length) {
          knownLength += length;
        });
        cb(null, knownLength);
      });
    };
    FormData2.prototype.submit = function(params, cb) {
      var request;
      var options;
      var defaults = { method: "post" };
      if (typeof params === "string") {
        params = parseUrl(params);
        options = populate({
          port: params.port,
          path: params.pathname,
          host: params.hostname,
          protocol: params.protocol
        }, defaults);
      } else {
        options = populate(params, defaults);
        if (!options.port) {
          options.port = options.protocol === "https:" ? 443 : 80;
        }
      }
      options.headers = this.getHeaders(params.headers);
      if (options.protocol === "https:") {
        request = https.request(options);
      } else {
        request = http.request(options);
      }
      this.getLength(function(err, length) {
        if (err && err !== "Unknown stream") {
          this._error(err);
          return;
        }
        if (length) {
          request.setHeader("Content-Length", length);
        }
        this.pipe(request);
        if (cb) {
          var onResponse;
          var callback = function(error, responce) {
            request.removeListener("error", callback);
            request.removeListener("response", onResponse);
            return cb.call(this, error, responce);
          };
          onResponse = callback.bind(this, null);
          request.on("error", callback);
          request.on("response", onResponse);
        }
      }.bind(this));
      return request;
    };
    FormData2.prototype._error = function(err) {
      if (!this.error) {
        this.error = err;
        this.pause();
        this.emit("error", err);
      }
    };
    FormData2.prototype.toString = function() {
      return "[object FormData]";
    };
    setToStringTag(FormData2.prototype, "FormData");
    module2.exports = FormData2;
  }
});

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
    Utils.getConfigValue = function(val, row2) {
      if (typeof val === "string")
        val = this.removeNamespaceFromVariable(val);
      if (typeof val === "string" && row2[val]) {
        const rad = row2[val];
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

// node_modules/@terminusdb/terminusdb-client/lib/typedef.js
var require_typedef = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/typedef.js"(exports2, module2) {
    var Utils = require_utils();
    var { ACTIONS } = Utils.ACTIONS;
    module2.exports = {};
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/const.js
var require_const = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/const.js"(exports2, module2) {
    module2.exports = Object.freeze({
      POST: "POST",
      GET: "GET",
      DELETE: "DELETE",
      PUT: "PUT",
      HEAD: "HEAD",
      QUERY_DOCUMENT: "QUERY_DOCUMENT",
      SQUASH_BRANCH: "SQUASH_BRANCH",
      UPDATE_SCHEMA: "UPDATE_SCHEMA",
      CONNECT: "connect",
      CREATE_DATABASE: "create_database",
      READ_DATABASE: "read_database",
      UPDATE_DATABASE: "update_database",
      CREATE_USER: "create_user",
      READ_USER: "read_user",
      UPDATE_USER: "update_user",
      CREATE_ORGANIZATION: "create_organization",
      READ_ORGANIZATION: "read_organization",
      UPDATE_ORGANIZATION: "update_organization",
      GET_ROLES: "get_roles",
      UPDATE_ROLES: "update_roles",
      CREATE_GRAPH: "create_graph",
      GET_TRIPLES: "get_triples",
      INSERT_TRIPLES: "insert_triples",
      UPDATE_TRIPLES: "update_triples",
      CLASS_FRAME: "class_frame",
      WOQL_QUERY: "woql_query",
      CLONE: "clone",
      CSV: "csv",
      WOQL: "woql",
      FRAME: "frame",
      PUSH: "push",
      PULL: "pull",
      FETCH: "fetch",
      REBASE: "rebase",
      RESET: "reset",
      BRANCH: "branch",
      REMOTE: "remote",
      CREATE_REMOTE: "create_remote",
      GET_REMOTE: "get_remote",
      UPDATE_REMOTE: "update_remote",
      DELETE_REMOTE: "delete_remote",
      RESET_BRANCH: "reset_branch",
      ADD_CSV: "add_csv",
      GET_CSV: "get_csv",
      UPDATE_CSV: "update_csv",
      MESSAGE: "message",
      ACTION: "action",
      INFO: "info",
      OPTIMIZE_SYSTEM: "optimize_system",
      GET_DIFF: "getDiff",
      PATCH: "patch"
    });
  }
});

// node_modules/pako/lib/zlib/trees.js
var require_trees = __commonJS({
  "node_modules/pako/lib/zlib/trees.js"(exports2, module2) {
    "use strict";
    var Z_FIXED = 4;
    var Z_BINARY = 0;
    var Z_TEXT = 1;
    var Z_UNKNOWN = 2;
    function zero(buf) {
      let len = buf.length;
      while (--len >= 0) {
        buf[len] = 0;
      }
    }
    var STORED_BLOCK = 0;
    var STATIC_TREES = 1;
    var DYN_TREES = 2;
    var MIN_MATCH = 3;
    var MAX_MATCH = 258;
    var LENGTH_CODES = 29;
    var LITERALS = 256;
    var L_CODES = LITERALS + 1 + LENGTH_CODES;
    var D_CODES = 30;
    var BL_CODES = 19;
    var HEAP_SIZE = 2 * L_CODES + 1;
    var MAX_BITS = 15;
    var Buf_size = 16;
    var MAX_BL_BITS = 7;
    var END_BLOCK = 256;
    var REP_3_6 = 16;
    var REPZ_3_10 = 17;
    var REPZ_11_138 = 18;
    var extra_lbits = (
      /* extra bits for each length code */
      new Uint8Array([0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0])
    );
    var extra_dbits = (
      /* extra bits for each distance code */
      new Uint8Array([0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13])
    );
    var extra_blbits = (
      /* extra bits for each bit length code */
      new Uint8Array([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7])
    );
    var bl_order = new Uint8Array([16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]);
    var DIST_CODE_LEN = 512;
    var static_ltree = new Array((L_CODES + 2) * 2);
    zero(static_ltree);
    var static_dtree = new Array(D_CODES * 2);
    zero(static_dtree);
    var _dist_code = new Array(DIST_CODE_LEN);
    zero(_dist_code);
    var _length_code = new Array(MAX_MATCH - MIN_MATCH + 1);
    zero(_length_code);
    var base_length = new Array(LENGTH_CODES);
    zero(base_length);
    var base_dist = new Array(D_CODES);
    zero(base_dist);
    function StaticTreeDesc(static_tree, extra_bits, extra_base, elems, max_length) {
      this.static_tree = static_tree;
      this.extra_bits = extra_bits;
      this.extra_base = extra_base;
      this.elems = elems;
      this.max_length = max_length;
      this.has_stree = static_tree && static_tree.length;
    }
    var static_l_desc;
    var static_d_desc;
    var static_bl_desc;
    function TreeDesc(dyn_tree, stat_desc) {
      this.dyn_tree = dyn_tree;
      this.max_code = 0;
      this.stat_desc = stat_desc;
    }
    var d_code = (dist) => {
      return dist < 256 ? _dist_code[dist] : _dist_code[256 + (dist >>> 7)];
    };
    var put_short = (s, w) => {
      s.pending_buf[s.pending++] = w & 255;
      s.pending_buf[s.pending++] = w >>> 8 & 255;
    };
    var send_bits = (s, value, length) => {
      if (s.bi_valid > Buf_size - length) {
        s.bi_buf |= value << s.bi_valid & 65535;
        put_short(s, s.bi_buf);
        s.bi_buf = value >> Buf_size - s.bi_valid;
        s.bi_valid += length - Buf_size;
      } else {
        s.bi_buf |= value << s.bi_valid & 65535;
        s.bi_valid += length;
      }
    };
    var send_code = (s, c, tree) => {
      send_bits(
        s,
        tree[c * 2],
        tree[c * 2 + 1]
        /*.Len*/
      );
    };
    var bi_reverse = (code, len) => {
      let res = 0;
      do {
        res |= code & 1;
        code >>>= 1;
        res <<= 1;
      } while (--len > 0);
      return res >>> 1;
    };
    var bi_flush = (s) => {
      if (s.bi_valid === 16) {
        put_short(s, s.bi_buf);
        s.bi_buf = 0;
        s.bi_valid = 0;
      } else if (s.bi_valid >= 8) {
        s.pending_buf[s.pending++] = s.bi_buf & 255;
        s.bi_buf >>= 8;
        s.bi_valid -= 8;
      }
    };
    var gen_bitlen = (s, desc) => {
      const tree = desc.dyn_tree;
      const max_code = desc.max_code;
      const stree = desc.stat_desc.static_tree;
      const has_stree = desc.stat_desc.has_stree;
      const extra = desc.stat_desc.extra_bits;
      const base = desc.stat_desc.extra_base;
      const max_length = desc.stat_desc.max_length;
      let h;
      let n, m;
      let bits;
      let xbits;
      let f;
      let overflow = 0;
      for (bits = 0; bits <= MAX_BITS; bits++) {
        s.bl_count[bits] = 0;
      }
      tree[s.heap[s.heap_max] * 2 + 1] = 0;
      for (h = s.heap_max + 1; h < HEAP_SIZE; h++) {
        n = s.heap[h];
        bits = tree[tree[n * 2 + 1] * 2 + 1] + 1;
        if (bits > max_length) {
          bits = max_length;
          overflow++;
        }
        tree[n * 2 + 1] = bits;
        if (n > max_code) {
          continue;
        }
        s.bl_count[bits]++;
        xbits = 0;
        if (n >= base) {
          xbits = extra[n - base];
        }
        f = tree[n * 2];
        s.opt_len += f * (bits + xbits);
        if (has_stree) {
          s.static_len += f * (stree[n * 2 + 1] + xbits);
        }
      }
      if (overflow === 0) {
        return;
      }
      do {
        bits = max_length - 1;
        while (s.bl_count[bits] === 0) {
          bits--;
        }
        s.bl_count[bits]--;
        s.bl_count[bits + 1] += 2;
        s.bl_count[max_length]--;
        overflow -= 2;
      } while (overflow > 0);
      for (bits = max_length; bits !== 0; bits--) {
        n = s.bl_count[bits];
        while (n !== 0) {
          m = s.heap[--h];
          if (m > max_code) {
            continue;
          }
          if (tree[m * 2 + 1] !== bits) {
            s.opt_len += (bits - tree[m * 2 + 1]) * tree[m * 2];
            tree[m * 2 + 1] = bits;
          }
          n--;
        }
      }
    };
    var gen_codes = (tree, max_code, bl_count) => {
      const next_code = new Array(MAX_BITS + 1);
      let code = 0;
      let bits;
      let n;
      for (bits = 1; bits <= MAX_BITS; bits++) {
        code = code + bl_count[bits - 1] << 1;
        next_code[bits] = code;
      }
      for (n = 0; n <= max_code; n++) {
        let len = tree[n * 2 + 1];
        if (len === 0) {
          continue;
        }
        tree[n * 2] = bi_reverse(next_code[len]++, len);
      }
    };
    var tr_static_init = () => {
      let n;
      let bits;
      let length;
      let code;
      let dist;
      const bl_count = new Array(MAX_BITS + 1);
      length = 0;
      for (code = 0; code < LENGTH_CODES - 1; code++) {
        base_length[code] = length;
        for (n = 0; n < 1 << extra_lbits[code]; n++) {
          _length_code[length++] = code;
        }
      }
      _length_code[length - 1] = code;
      dist = 0;
      for (code = 0; code < 16; code++) {
        base_dist[code] = dist;
        for (n = 0; n < 1 << extra_dbits[code]; n++) {
          _dist_code[dist++] = code;
        }
      }
      dist >>= 7;
      for (; code < D_CODES; code++) {
        base_dist[code] = dist << 7;
        for (n = 0; n < 1 << extra_dbits[code] - 7; n++) {
          _dist_code[256 + dist++] = code;
        }
      }
      for (bits = 0; bits <= MAX_BITS; bits++) {
        bl_count[bits] = 0;
      }
      n = 0;
      while (n <= 143) {
        static_ltree[n * 2 + 1] = 8;
        n++;
        bl_count[8]++;
      }
      while (n <= 255) {
        static_ltree[n * 2 + 1] = 9;
        n++;
        bl_count[9]++;
      }
      while (n <= 279) {
        static_ltree[n * 2 + 1] = 7;
        n++;
        bl_count[7]++;
      }
      while (n <= 287) {
        static_ltree[n * 2 + 1] = 8;
        n++;
        bl_count[8]++;
      }
      gen_codes(static_ltree, L_CODES + 1, bl_count);
      for (n = 0; n < D_CODES; n++) {
        static_dtree[n * 2 + 1] = 5;
        static_dtree[n * 2] = bi_reverse(n, 5);
      }
      static_l_desc = new StaticTreeDesc(static_ltree, extra_lbits, LITERALS + 1, L_CODES, MAX_BITS);
      static_d_desc = new StaticTreeDesc(static_dtree, extra_dbits, 0, D_CODES, MAX_BITS);
      static_bl_desc = new StaticTreeDesc(new Array(0), extra_blbits, 0, BL_CODES, MAX_BL_BITS);
    };
    var init_block = (s) => {
      let n;
      for (n = 0; n < L_CODES; n++) {
        s.dyn_ltree[n * 2] = 0;
      }
      for (n = 0; n < D_CODES; n++) {
        s.dyn_dtree[n * 2] = 0;
      }
      for (n = 0; n < BL_CODES; n++) {
        s.bl_tree[n * 2] = 0;
      }
      s.dyn_ltree[END_BLOCK * 2] = 1;
      s.opt_len = s.static_len = 0;
      s.sym_next = s.matches = 0;
    };
    var bi_windup = (s) => {
      if (s.bi_valid > 8) {
        put_short(s, s.bi_buf);
      } else if (s.bi_valid > 0) {
        s.pending_buf[s.pending++] = s.bi_buf;
      }
      s.bi_buf = 0;
      s.bi_valid = 0;
    };
    var smaller = (tree, n, m, depth) => {
      const _n2 = n * 2;
      const _m2 = m * 2;
      return tree[_n2] < tree[_m2] || tree[_n2] === tree[_m2] && depth[n] <= depth[m];
    };
    var pqdownheap = (s, tree, k) => {
      const v = s.heap[k];
      let j = k << 1;
      while (j <= s.heap_len) {
        if (j < s.heap_len && smaller(tree, s.heap[j + 1], s.heap[j], s.depth)) {
          j++;
        }
        if (smaller(tree, v, s.heap[j], s.depth)) {
          break;
        }
        s.heap[k] = s.heap[j];
        k = j;
        j <<= 1;
      }
      s.heap[k] = v;
    };
    var compress_block = (s, ltree, dtree) => {
      let dist;
      let lc;
      let sx = 0;
      let code;
      let extra;
      if (s.sym_next !== 0) {
        do {
          dist = s.pending_buf[s.sym_buf + sx++] & 255;
          dist += (s.pending_buf[s.sym_buf + sx++] & 255) << 8;
          lc = s.pending_buf[s.sym_buf + sx++];
          if (dist === 0) {
            send_code(s, lc, ltree);
          } else {
            code = _length_code[lc];
            send_code(s, code + LITERALS + 1, ltree);
            extra = extra_lbits[code];
            if (extra !== 0) {
              lc -= base_length[code];
              send_bits(s, lc, extra);
            }
            dist--;
            code = d_code(dist);
            send_code(s, code, dtree);
            extra = extra_dbits[code];
            if (extra !== 0) {
              dist -= base_dist[code];
              send_bits(s, dist, extra);
            }
          }
        } while (sx < s.sym_next);
      }
      send_code(s, END_BLOCK, ltree);
    };
    var build_tree = (s, desc) => {
      const tree = desc.dyn_tree;
      const stree = desc.stat_desc.static_tree;
      const has_stree = desc.stat_desc.has_stree;
      const elems = desc.stat_desc.elems;
      let n, m;
      let max_code = -1;
      let node;
      s.heap_len = 0;
      s.heap_max = HEAP_SIZE;
      for (n = 0; n < elems; n++) {
        if (tree[n * 2] !== 0) {
          s.heap[++s.heap_len] = max_code = n;
          s.depth[n] = 0;
        } else {
          tree[n * 2 + 1] = 0;
        }
      }
      while (s.heap_len < 2) {
        node = s.heap[++s.heap_len] = max_code < 2 ? ++max_code : 0;
        tree[node * 2] = 1;
        s.depth[node] = 0;
        s.opt_len--;
        if (has_stree) {
          s.static_len -= stree[node * 2 + 1];
        }
      }
      desc.max_code = max_code;
      for (n = s.heap_len >> 1; n >= 1; n--) {
        pqdownheap(s, tree, n);
      }
      node = elems;
      do {
        n = s.heap[
          1
          /*SMALLEST*/
        ];
        s.heap[
          1
          /*SMALLEST*/
        ] = s.heap[s.heap_len--];
        pqdownheap(
          s,
          tree,
          1
          /*SMALLEST*/
        );
        m = s.heap[
          1
          /*SMALLEST*/
        ];
        s.heap[--s.heap_max] = n;
        s.heap[--s.heap_max] = m;
        tree[node * 2] = tree[n * 2] + tree[m * 2];
        s.depth[node] = (s.depth[n] >= s.depth[m] ? s.depth[n] : s.depth[m]) + 1;
        tree[n * 2 + 1] = tree[m * 2 + 1] = node;
        s.heap[
          1
          /*SMALLEST*/
        ] = node++;
        pqdownheap(
          s,
          tree,
          1
          /*SMALLEST*/
        );
      } while (s.heap_len >= 2);
      s.heap[--s.heap_max] = s.heap[
        1
        /*SMALLEST*/
      ];
      gen_bitlen(s, desc);
      gen_codes(tree, max_code, s.bl_count);
    };
    var scan_tree = (s, tree, max_code) => {
      let n;
      let prevlen = -1;
      let curlen;
      let nextlen = tree[0 * 2 + 1];
      let count = 0;
      let max_count = 7;
      let min_count = 4;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      }
      tree[(max_code + 1) * 2 + 1] = 65535;
      for (n = 0; n <= max_code; n++) {
        curlen = nextlen;
        nextlen = tree[(n + 1) * 2 + 1];
        if (++count < max_count && curlen === nextlen) {
          continue;
        } else if (count < min_count) {
          s.bl_tree[curlen * 2] += count;
        } else if (curlen !== 0) {
          if (curlen !== prevlen) {
            s.bl_tree[curlen * 2]++;
          }
          s.bl_tree[REP_3_6 * 2]++;
        } else if (count <= 10) {
          s.bl_tree[REPZ_3_10 * 2]++;
        } else {
          s.bl_tree[REPZ_11_138 * 2]++;
        }
        count = 0;
        prevlen = curlen;
        if (nextlen === 0) {
          max_count = 138;
          min_count = 3;
        } else if (curlen === nextlen) {
          max_count = 6;
          min_count = 3;
        } else {
          max_count = 7;
          min_count = 4;
        }
      }
    };
    var send_tree = (s, tree, max_code) => {
      let n;
      let prevlen = -1;
      let curlen;
      let nextlen = tree[0 * 2 + 1];
      let count = 0;
      let max_count = 7;
      let min_count = 4;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      }
      for (n = 0; n <= max_code; n++) {
        curlen = nextlen;
        nextlen = tree[(n + 1) * 2 + 1];
        if (++count < max_count && curlen === nextlen) {
          continue;
        } else if (count < min_count) {
          do {
            send_code(s, curlen, s.bl_tree);
          } while (--count !== 0);
        } else if (curlen !== 0) {
          if (curlen !== prevlen) {
            send_code(s, curlen, s.bl_tree);
            count--;
          }
          send_code(s, REP_3_6, s.bl_tree);
          send_bits(s, count - 3, 2);
        } else if (count <= 10) {
          send_code(s, REPZ_3_10, s.bl_tree);
          send_bits(s, count - 3, 3);
        } else {
          send_code(s, REPZ_11_138, s.bl_tree);
          send_bits(s, count - 11, 7);
        }
        count = 0;
        prevlen = curlen;
        if (nextlen === 0) {
          max_count = 138;
          min_count = 3;
        } else if (curlen === nextlen) {
          max_count = 6;
          min_count = 3;
        } else {
          max_count = 7;
          min_count = 4;
        }
      }
    };
    var build_bl_tree = (s) => {
      let max_blindex;
      scan_tree(s, s.dyn_ltree, s.l_desc.max_code);
      scan_tree(s, s.dyn_dtree, s.d_desc.max_code);
      build_tree(s, s.bl_desc);
      for (max_blindex = BL_CODES - 1; max_blindex >= 3; max_blindex--) {
        if (s.bl_tree[bl_order[max_blindex] * 2 + 1] !== 0) {
          break;
        }
      }
      s.opt_len += 3 * (max_blindex + 1) + 5 + 5 + 4;
      return max_blindex;
    };
    var send_all_trees = (s, lcodes, dcodes, blcodes) => {
      let rank;
      send_bits(s, lcodes - 257, 5);
      send_bits(s, dcodes - 1, 5);
      send_bits(s, blcodes - 4, 4);
      for (rank = 0; rank < blcodes; rank++) {
        send_bits(s, s.bl_tree[bl_order[rank] * 2 + 1], 3);
      }
      send_tree(s, s.dyn_ltree, lcodes - 1);
      send_tree(s, s.dyn_dtree, dcodes - 1);
    };
    var detect_data_type = (s) => {
      let block_mask = 4093624447;
      let n;
      for (n = 0; n <= 31; n++, block_mask >>>= 1) {
        if (block_mask & 1 && s.dyn_ltree[n * 2] !== 0) {
          return Z_BINARY;
        }
      }
      if (s.dyn_ltree[9 * 2] !== 0 || s.dyn_ltree[10 * 2] !== 0 || s.dyn_ltree[13 * 2] !== 0) {
        return Z_TEXT;
      }
      for (n = 32; n < LITERALS; n++) {
        if (s.dyn_ltree[n * 2] !== 0) {
          return Z_TEXT;
        }
      }
      return Z_BINARY;
    };
    var static_init_done = false;
    var _tr_init = (s) => {
      if (!static_init_done) {
        tr_static_init();
        static_init_done = true;
      }
      s.l_desc = new TreeDesc(s.dyn_ltree, static_l_desc);
      s.d_desc = new TreeDesc(s.dyn_dtree, static_d_desc);
      s.bl_desc = new TreeDesc(s.bl_tree, static_bl_desc);
      s.bi_buf = 0;
      s.bi_valid = 0;
      init_block(s);
    };
    var _tr_stored_block = (s, buf, stored_len, last) => {
      send_bits(s, (STORED_BLOCK << 1) + (last ? 1 : 0), 3);
      bi_windup(s);
      put_short(s, stored_len);
      put_short(s, ~stored_len);
      if (stored_len) {
        s.pending_buf.set(s.window.subarray(buf, buf + stored_len), s.pending);
      }
      s.pending += stored_len;
    };
    var _tr_align = (s) => {
      send_bits(s, STATIC_TREES << 1, 3);
      send_code(s, END_BLOCK, static_ltree);
      bi_flush(s);
    };
    var _tr_flush_block = (s, buf, stored_len, last) => {
      let opt_lenb, static_lenb;
      let max_blindex = 0;
      if (s.level > 0) {
        if (s.strm.data_type === Z_UNKNOWN) {
          s.strm.data_type = detect_data_type(s);
        }
        build_tree(s, s.l_desc);
        build_tree(s, s.d_desc);
        max_blindex = build_bl_tree(s);
        opt_lenb = s.opt_len + 3 + 7 >>> 3;
        static_lenb = s.static_len + 3 + 7 >>> 3;
        if (static_lenb <= opt_lenb) {
          opt_lenb = static_lenb;
        }
      } else {
        opt_lenb = static_lenb = stored_len + 5;
      }
      if (stored_len + 4 <= opt_lenb && buf !== -1) {
        _tr_stored_block(s, buf, stored_len, last);
      } else if (s.strategy === Z_FIXED || static_lenb === opt_lenb) {
        send_bits(s, (STATIC_TREES << 1) + (last ? 1 : 0), 3);
        compress_block(s, static_ltree, static_dtree);
      } else {
        send_bits(s, (DYN_TREES << 1) + (last ? 1 : 0), 3);
        send_all_trees(s, s.l_desc.max_code + 1, s.d_desc.max_code + 1, max_blindex + 1);
        compress_block(s, s.dyn_ltree, s.dyn_dtree);
      }
      init_block(s);
      if (last) {
        bi_windup(s);
      }
    };
    var _tr_tally = (s, dist, lc) => {
      s.pending_buf[s.sym_buf + s.sym_next++] = dist;
      s.pending_buf[s.sym_buf + s.sym_next++] = dist >> 8;
      s.pending_buf[s.sym_buf + s.sym_next++] = lc;
      if (dist === 0) {
        s.dyn_ltree[lc * 2]++;
      } else {
        s.matches++;
        dist--;
        s.dyn_ltree[(_length_code[lc] + LITERALS + 1) * 2]++;
        s.dyn_dtree[d_code(dist) * 2]++;
      }
      return s.sym_next === s.sym_end;
    };
    module2.exports._tr_init = _tr_init;
    module2.exports._tr_stored_block = _tr_stored_block;
    module2.exports._tr_flush_block = _tr_flush_block;
    module2.exports._tr_tally = _tr_tally;
    module2.exports._tr_align = _tr_align;
  }
});

// node_modules/pako/lib/zlib/adler32.js
var require_adler32 = __commonJS({
  "node_modules/pako/lib/zlib/adler32.js"(exports2, module2) {
    "use strict";
    var adler32 = (adler, buf, len, pos) => {
      let s1 = adler & 65535 | 0, s2 = adler >>> 16 & 65535 | 0, n = 0;
      while (len !== 0) {
        n = len > 2e3 ? 2e3 : len;
        len -= n;
        do {
          s1 = s1 + buf[pos++] | 0;
          s2 = s2 + s1 | 0;
        } while (--n);
        s1 %= 65521;
        s2 %= 65521;
      }
      return s1 | s2 << 16 | 0;
    };
    module2.exports = adler32;
  }
});

// node_modules/pako/lib/zlib/crc32.js
var require_crc32 = __commonJS({
  "node_modules/pako/lib/zlib/crc32.js"(exports2, module2) {
    "use strict";
    var makeTable = () => {
      let c, table = [];
      for (var n = 0; n < 256; n++) {
        c = n;
        for (var k = 0; k < 8; k++) {
          c = c & 1 ? 3988292384 ^ c >>> 1 : c >>> 1;
        }
        table[n] = c;
      }
      return table;
    };
    var crcTable = new Uint32Array(makeTable());
    var crc32 = (crc, buf, len, pos) => {
      const t = crcTable;
      const end = pos + len;
      crc ^= -1;
      for (let i = pos; i < end; i++) {
        crc = crc >>> 8 ^ t[(crc ^ buf[i]) & 255];
      }
      return crc ^ -1;
    };
    module2.exports = crc32;
  }
});

// node_modules/pako/lib/zlib/messages.js
var require_messages = __commonJS({
  "node_modules/pako/lib/zlib/messages.js"(exports2, module2) {
    "use strict";
    module2.exports = {
      2: "need dictionary",
      /* Z_NEED_DICT       2  */
      1: "stream end",
      /* Z_STREAM_END      1  */
      0: "",
      /* Z_OK              0  */
      "-1": "file error",
      /* Z_ERRNO         (-1) */
      "-2": "stream error",
      /* Z_STREAM_ERROR  (-2) */
      "-3": "data error",
      /* Z_DATA_ERROR    (-3) */
      "-4": "insufficient memory",
      /* Z_MEM_ERROR     (-4) */
      "-5": "buffer error",
      /* Z_BUF_ERROR     (-5) */
      "-6": "incompatible version"
      /* Z_VERSION_ERROR (-6) */
    };
  }
});

// node_modules/pako/lib/zlib/constants.js
var require_constants = __commonJS({
  "node_modules/pako/lib/zlib/constants.js"(exports2, module2) {
    "use strict";
    module2.exports = {
      /* Allowed flush values; see deflate() and inflate() below for details */
      Z_NO_FLUSH: 0,
      Z_PARTIAL_FLUSH: 1,
      Z_SYNC_FLUSH: 2,
      Z_FULL_FLUSH: 3,
      Z_FINISH: 4,
      Z_BLOCK: 5,
      Z_TREES: 6,
      /* Return codes for the compression/decompression functions. Negative values
      * are errors, positive values are used for special but normal events.
      */
      Z_OK: 0,
      Z_STREAM_END: 1,
      Z_NEED_DICT: 2,
      Z_ERRNO: -1,
      Z_STREAM_ERROR: -2,
      Z_DATA_ERROR: -3,
      Z_MEM_ERROR: -4,
      Z_BUF_ERROR: -5,
      //Z_VERSION_ERROR: -6,
      /* compression levels */
      Z_NO_COMPRESSION: 0,
      Z_BEST_SPEED: 1,
      Z_BEST_COMPRESSION: 9,
      Z_DEFAULT_COMPRESSION: -1,
      Z_FILTERED: 1,
      Z_HUFFMAN_ONLY: 2,
      Z_RLE: 3,
      Z_FIXED: 4,
      Z_DEFAULT_STRATEGY: 0,
      /* Possible values of the data_type field (though see inflate()) */
      Z_BINARY: 0,
      Z_TEXT: 1,
      //Z_ASCII:                1, // = Z_TEXT (deprecated)
      Z_UNKNOWN: 2,
      /* The deflate compression method */
      Z_DEFLATED: 8
      //Z_NULL:                 null // Use -1 or null inline, depending on var type
    };
  }
});

// node_modules/pako/lib/zlib/deflate.js
var require_deflate = __commonJS({
  "node_modules/pako/lib/zlib/deflate.js"(exports2, module2) {
    "use strict";
    var { _tr_init, _tr_stored_block, _tr_flush_block, _tr_tally, _tr_align } = require_trees();
    var adler32 = require_adler32();
    var crc32 = require_crc32();
    var msg = require_messages();
    var {
      Z_NO_FLUSH,
      Z_PARTIAL_FLUSH,
      Z_FULL_FLUSH,
      Z_FINISH,
      Z_BLOCK,
      Z_OK,
      Z_STREAM_END,
      Z_STREAM_ERROR,
      Z_DATA_ERROR,
      Z_BUF_ERROR,
      Z_DEFAULT_COMPRESSION,
      Z_FILTERED,
      Z_HUFFMAN_ONLY,
      Z_RLE,
      Z_FIXED,
      Z_DEFAULT_STRATEGY,
      Z_UNKNOWN,
      Z_DEFLATED
    } = require_constants();
    var MAX_MEM_LEVEL = 9;
    var MAX_WBITS = 15;
    var DEF_MEM_LEVEL = 8;
    var LENGTH_CODES = 29;
    var LITERALS = 256;
    var L_CODES = LITERALS + 1 + LENGTH_CODES;
    var D_CODES = 30;
    var BL_CODES = 19;
    var HEAP_SIZE = 2 * L_CODES + 1;
    var MAX_BITS = 15;
    var MIN_MATCH = 3;
    var MAX_MATCH = 258;
    var MIN_LOOKAHEAD = MAX_MATCH + MIN_MATCH + 1;
    var PRESET_DICT = 32;
    var INIT_STATE = 42;
    var GZIP_STATE = 57;
    var EXTRA_STATE = 69;
    var NAME_STATE = 73;
    var COMMENT_STATE = 91;
    var HCRC_STATE = 103;
    var BUSY_STATE = 113;
    var FINISH_STATE = 666;
    var BS_NEED_MORE = 1;
    var BS_BLOCK_DONE = 2;
    var BS_FINISH_STARTED = 3;
    var BS_FINISH_DONE = 4;
    var OS_CODE = 3;
    var err = (strm, errorCode) => {
      strm.msg = msg[errorCode];
      return errorCode;
    };
    var rank = (f) => {
      return f * 2 - (f > 4 ? 9 : 0);
    };
    var zero = (buf) => {
      let len = buf.length;
      while (--len >= 0) {
        buf[len] = 0;
      }
    };
    var slide_hash = (s) => {
      let n, m;
      let p;
      let wsize = s.w_size;
      n = s.hash_size;
      p = n;
      do {
        m = s.head[--p];
        s.head[p] = m >= wsize ? m - wsize : 0;
      } while (--n);
      n = wsize;
      p = n;
      do {
        m = s.prev[--p];
        s.prev[p] = m >= wsize ? m - wsize : 0;
      } while (--n);
    };
    var HASH_ZLIB = (s, prev, data) => (prev << s.hash_shift ^ data) & s.hash_mask;
    var HASH = HASH_ZLIB;
    var flush_pending = (strm) => {
      const s = strm.state;
      let len = s.pending;
      if (len > strm.avail_out) {
        len = strm.avail_out;
      }
      if (len === 0) {
        return;
      }
      strm.output.set(s.pending_buf.subarray(s.pending_out, s.pending_out + len), strm.next_out);
      strm.next_out += len;
      s.pending_out += len;
      strm.total_out += len;
      strm.avail_out -= len;
      s.pending -= len;
      if (s.pending === 0) {
        s.pending_out = 0;
      }
    };
    var flush_block_only = (s, last) => {
      _tr_flush_block(s, s.block_start >= 0 ? s.block_start : -1, s.strstart - s.block_start, last);
      s.block_start = s.strstart;
      flush_pending(s.strm);
    };
    var put_byte = (s, b) => {
      s.pending_buf[s.pending++] = b;
    };
    var putShortMSB = (s, b) => {
      s.pending_buf[s.pending++] = b >>> 8 & 255;
      s.pending_buf[s.pending++] = b & 255;
    };
    var read_buf = (strm, buf, start, size) => {
      let len = strm.avail_in;
      if (len > size) {
        len = size;
      }
      if (len === 0) {
        return 0;
      }
      strm.avail_in -= len;
      buf.set(strm.input.subarray(strm.next_in, strm.next_in + len), start);
      if (strm.state.wrap === 1) {
        strm.adler = adler32(strm.adler, buf, len, start);
      } else if (strm.state.wrap === 2) {
        strm.adler = crc32(strm.adler, buf, len, start);
      }
      strm.next_in += len;
      strm.total_in += len;
      return len;
    };
    var longest_match = (s, cur_match) => {
      let chain_length = s.max_chain_length;
      let scan = s.strstart;
      let match;
      let len;
      let best_len = s.prev_length;
      let nice_match = s.nice_match;
      const limit = s.strstart > s.w_size - MIN_LOOKAHEAD ? s.strstart - (s.w_size - MIN_LOOKAHEAD) : 0;
      const _win = s.window;
      const wmask = s.w_mask;
      const prev = s.prev;
      const strend = s.strstart + MAX_MATCH;
      let scan_end1 = _win[scan + best_len - 1];
      let scan_end = _win[scan + best_len];
      if (s.prev_length >= s.good_match) {
        chain_length >>= 2;
      }
      if (nice_match > s.lookahead) {
        nice_match = s.lookahead;
      }
      do {
        match = cur_match;
        if (_win[match + best_len] !== scan_end || _win[match + best_len - 1] !== scan_end1 || _win[match] !== _win[scan] || _win[++match] !== _win[scan + 1]) {
          continue;
        }
        scan += 2;
        match++;
        do {
        } while (_win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && scan < strend);
        len = MAX_MATCH - (strend - scan);
        scan = strend - MAX_MATCH;
        if (len > best_len) {
          s.match_start = cur_match;
          best_len = len;
          if (len >= nice_match) {
            break;
          }
          scan_end1 = _win[scan + best_len - 1];
          scan_end = _win[scan + best_len];
        }
      } while ((cur_match = prev[cur_match & wmask]) > limit && --chain_length !== 0);
      if (best_len <= s.lookahead) {
        return best_len;
      }
      return s.lookahead;
    };
    var fill_window = (s) => {
      const _w_size = s.w_size;
      let n, more, str2;
      do {
        more = s.window_size - s.lookahead - s.strstart;
        if (s.strstart >= _w_size + (_w_size - MIN_LOOKAHEAD)) {
          s.window.set(s.window.subarray(_w_size, _w_size + _w_size - more), 0);
          s.match_start -= _w_size;
          s.strstart -= _w_size;
          s.block_start -= _w_size;
          if (s.insert > s.strstart) {
            s.insert = s.strstart;
          }
          slide_hash(s);
          more += _w_size;
        }
        if (s.strm.avail_in === 0) {
          break;
        }
        n = read_buf(s.strm, s.window, s.strstart + s.lookahead, more);
        s.lookahead += n;
        if (s.lookahead + s.insert >= MIN_MATCH) {
          str2 = s.strstart - s.insert;
          s.ins_h = s.window[str2];
          s.ins_h = HASH(s, s.ins_h, s.window[str2 + 1]);
          while (s.insert) {
            s.ins_h = HASH(s, s.ins_h, s.window[str2 + MIN_MATCH - 1]);
            s.prev[str2 & s.w_mask] = s.head[s.ins_h];
            s.head[s.ins_h] = str2;
            str2++;
            s.insert--;
            if (s.lookahead + s.insert < MIN_MATCH) {
              break;
            }
          }
        }
      } while (s.lookahead < MIN_LOOKAHEAD && s.strm.avail_in !== 0);
    };
    var deflate_stored = (s, flush) => {
      let min_block = s.pending_buf_size - 5 > s.w_size ? s.w_size : s.pending_buf_size - 5;
      let len, left, have, last = 0;
      let used = s.strm.avail_in;
      do {
        len = 65535;
        have = s.bi_valid + 42 >> 3;
        if (s.strm.avail_out < have) {
          break;
        }
        have = s.strm.avail_out - have;
        left = s.strstart - s.block_start;
        if (len > left + s.strm.avail_in) {
          len = left + s.strm.avail_in;
        }
        if (len > have) {
          len = have;
        }
        if (len < min_block && (len === 0 && flush !== Z_FINISH || flush === Z_NO_FLUSH || len !== left + s.strm.avail_in)) {
          break;
        }
        last = flush === Z_FINISH && len === left + s.strm.avail_in ? 1 : 0;
        _tr_stored_block(s, 0, 0, last);
        s.pending_buf[s.pending - 4] = len;
        s.pending_buf[s.pending - 3] = len >> 8;
        s.pending_buf[s.pending - 2] = ~len;
        s.pending_buf[s.pending - 1] = ~len >> 8;
        flush_pending(s.strm);
        if (left) {
          if (left > len) {
            left = len;
          }
          s.strm.output.set(s.window.subarray(s.block_start, s.block_start + left), s.strm.next_out);
          s.strm.next_out += left;
          s.strm.avail_out -= left;
          s.strm.total_out += left;
          s.block_start += left;
          len -= left;
        }
        if (len) {
          read_buf(s.strm, s.strm.output, s.strm.next_out, len);
          s.strm.next_out += len;
          s.strm.avail_out -= len;
          s.strm.total_out += len;
        }
      } while (last === 0);
      used -= s.strm.avail_in;
      if (used) {
        if (used >= s.w_size) {
          s.matches = 2;
          s.window.set(s.strm.input.subarray(s.strm.next_in - s.w_size, s.strm.next_in), 0);
          s.strstart = s.w_size;
          s.insert = s.strstart;
        } else {
          if (s.window_size - s.strstart <= used) {
            s.strstart -= s.w_size;
            s.window.set(s.window.subarray(s.w_size, s.w_size + s.strstart), 0);
            if (s.matches < 2) {
              s.matches++;
            }
            if (s.insert > s.strstart) {
              s.insert = s.strstart;
            }
          }
          s.window.set(s.strm.input.subarray(s.strm.next_in - used, s.strm.next_in), s.strstart);
          s.strstart += used;
          s.insert += used > s.w_size - s.insert ? s.w_size - s.insert : used;
        }
        s.block_start = s.strstart;
      }
      if (s.high_water < s.strstart) {
        s.high_water = s.strstart;
      }
      if (last) {
        return BS_FINISH_DONE;
      }
      if (flush !== Z_NO_FLUSH && flush !== Z_FINISH && s.strm.avail_in === 0 && s.strstart === s.block_start) {
        return BS_BLOCK_DONE;
      }
      have = s.window_size - s.strstart;
      if (s.strm.avail_in > have && s.block_start >= s.w_size) {
        s.block_start -= s.w_size;
        s.strstart -= s.w_size;
        s.window.set(s.window.subarray(s.w_size, s.w_size + s.strstart), 0);
        if (s.matches < 2) {
          s.matches++;
        }
        have += s.w_size;
        if (s.insert > s.strstart) {
          s.insert = s.strstart;
        }
      }
      if (have > s.strm.avail_in) {
        have = s.strm.avail_in;
      }
      if (have) {
        read_buf(s.strm, s.window, s.strstart, have);
        s.strstart += have;
        s.insert += have > s.w_size - s.insert ? s.w_size - s.insert : have;
      }
      if (s.high_water < s.strstart) {
        s.high_water = s.strstart;
      }
      have = s.bi_valid + 42 >> 3;
      have = s.pending_buf_size - have > 65535 ? 65535 : s.pending_buf_size - have;
      min_block = have > s.w_size ? s.w_size : have;
      left = s.strstart - s.block_start;
      if (left >= min_block || (left || flush === Z_FINISH) && flush !== Z_NO_FLUSH && s.strm.avail_in === 0 && left <= have) {
        len = left > have ? have : left;
        last = flush === Z_FINISH && s.strm.avail_in === 0 && len === left ? 1 : 0;
        _tr_stored_block(s, s.block_start, len, last);
        s.block_start += len;
        flush_pending(s.strm);
      }
      return last ? BS_FINISH_STARTED : BS_NEED_MORE;
    };
    var deflate_fast = (s, flush) => {
      let hash_head;
      let bflush;
      for (; ; ) {
        if (s.lookahead < MIN_LOOKAHEAD) {
          fill_window(s);
          if (s.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        hash_head = 0;
        if (s.lookahead >= MIN_MATCH) {
          s.ins_h = HASH(s, s.ins_h, s.window[s.strstart + MIN_MATCH - 1]);
          hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = s.strstart;
        }
        if (hash_head !== 0 && s.strstart - hash_head <= s.w_size - MIN_LOOKAHEAD) {
          s.match_length = longest_match(s, hash_head);
        }
        if (s.match_length >= MIN_MATCH) {
          bflush = _tr_tally(s, s.strstart - s.match_start, s.match_length - MIN_MATCH);
          s.lookahead -= s.match_length;
          if (s.match_length <= s.max_lazy_match && s.lookahead >= MIN_MATCH) {
            s.match_length--;
            do {
              s.strstart++;
              s.ins_h = HASH(s, s.ins_h, s.window[s.strstart + MIN_MATCH - 1]);
              hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
              s.head[s.ins_h] = s.strstart;
            } while (--s.match_length !== 0);
            s.strstart++;
          } else {
            s.strstart += s.match_length;
            s.match_length = 0;
            s.ins_h = s.window[s.strstart];
            s.ins_h = HASH(s, s.ins_h, s.window[s.strstart + 1]);
          }
        } else {
          bflush = _tr_tally(s, 0, s.window[s.strstart]);
          s.lookahead--;
          s.strstart++;
        }
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = s.strstart < MIN_MATCH - 1 ? s.strstart : MIN_MATCH - 1;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.sym_next) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    };
    var deflate_slow = (s, flush) => {
      let hash_head;
      let bflush;
      let max_insert;
      for (; ; ) {
        if (s.lookahead < MIN_LOOKAHEAD) {
          fill_window(s);
          if (s.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        hash_head = 0;
        if (s.lookahead >= MIN_MATCH) {
          s.ins_h = HASH(s, s.ins_h, s.window[s.strstart + MIN_MATCH - 1]);
          hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = s.strstart;
        }
        s.prev_length = s.match_length;
        s.prev_match = s.match_start;
        s.match_length = MIN_MATCH - 1;
        if (hash_head !== 0 && s.prev_length < s.max_lazy_match && s.strstart - hash_head <= s.w_size - MIN_LOOKAHEAD) {
          s.match_length = longest_match(s, hash_head);
          if (s.match_length <= 5 && (s.strategy === Z_FILTERED || s.match_length === MIN_MATCH && s.strstart - s.match_start > 4096)) {
            s.match_length = MIN_MATCH - 1;
          }
        }
        if (s.prev_length >= MIN_MATCH && s.match_length <= s.prev_length) {
          max_insert = s.strstart + s.lookahead - MIN_MATCH;
          bflush = _tr_tally(s, s.strstart - 1 - s.prev_match, s.prev_length - MIN_MATCH);
          s.lookahead -= s.prev_length - 1;
          s.prev_length -= 2;
          do {
            if (++s.strstart <= max_insert) {
              s.ins_h = HASH(s, s.ins_h, s.window[s.strstart + MIN_MATCH - 1]);
              hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
              s.head[s.ins_h] = s.strstart;
            }
          } while (--s.prev_length !== 0);
          s.match_available = 0;
          s.match_length = MIN_MATCH - 1;
          s.strstart++;
          if (bflush) {
            flush_block_only(s, false);
            if (s.strm.avail_out === 0) {
              return BS_NEED_MORE;
            }
          }
        } else if (s.match_available) {
          bflush = _tr_tally(s, 0, s.window[s.strstart - 1]);
          if (bflush) {
            flush_block_only(s, false);
          }
          s.strstart++;
          s.lookahead--;
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        } else {
          s.match_available = 1;
          s.strstart++;
          s.lookahead--;
        }
      }
      if (s.match_available) {
        bflush = _tr_tally(s, 0, s.window[s.strstart - 1]);
        s.match_available = 0;
      }
      s.insert = s.strstart < MIN_MATCH - 1 ? s.strstart : MIN_MATCH - 1;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.sym_next) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    };
    var deflate_rle = (s, flush) => {
      let bflush;
      let prev;
      let scan, strend;
      const _win = s.window;
      for (; ; ) {
        if (s.lookahead <= MAX_MATCH) {
          fill_window(s);
          if (s.lookahead <= MAX_MATCH && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        s.match_length = 0;
        if (s.lookahead >= MIN_MATCH && s.strstart > 0) {
          scan = s.strstart - 1;
          prev = _win[scan];
          if (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan]) {
            strend = s.strstart + MAX_MATCH;
            do {
            } while (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && scan < strend);
            s.match_length = MAX_MATCH - (strend - scan);
            if (s.match_length > s.lookahead) {
              s.match_length = s.lookahead;
            }
          }
        }
        if (s.match_length >= MIN_MATCH) {
          bflush = _tr_tally(s, 1, s.match_length - MIN_MATCH);
          s.lookahead -= s.match_length;
          s.strstart += s.match_length;
          s.match_length = 0;
        } else {
          bflush = _tr_tally(s, 0, s.window[s.strstart]);
          s.lookahead--;
          s.strstart++;
        }
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = 0;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.sym_next) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    };
    var deflate_huff = (s, flush) => {
      let bflush;
      for (; ; ) {
        if (s.lookahead === 0) {
          fill_window(s);
          if (s.lookahead === 0) {
            if (flush === Z_NO_FLUSH) {
              return BS_NEED_MORE;
            }
            break;
          }
        }
        s.match_length = 0;
        bflush = _tr_tally(s, 0, s.window[s.strstart]);
        s.lookahead--;
        s.strstart++;
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = 0;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.sym_next) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    };
    function Config(good_length, max_lazy, nice_length, max_chain, func) {
      this.good_length = good_length;
      this.max_lazy = max_lazy;
      this.nice_length = nice_length;
      this.max_chain = max_chain;
      this.func = func;
    }
    var configuration_table = [
      /*      good lazy nice chain */
      new Config(0, 0, 0, 0, deflate_stored),
      /* 0 store only */
      new Config(4, 4, 8, 4, deflate_fast),
      /* 1 max speed, no lazy matches */
      new Config(4, 5, 16, 8, deflate_fast),
      /* 2 */
      new Config(4, 6, 32, 32, deflate_fast),
      /* 3 */
      new Config(4, 4, 16, 16, deflate_slow),
      /* 4 lazy matches */
      new Config(8, 16, 32, 32, deflate_slow),
      /* 5 */
      new Config(8, 16, 128, 128, deflate_slow),
      /* 6 */
      new Config(8, 32, 128, 256, deflate_slow),
      /* 7 */
      new Config(32, 128, 258, 1024, deflate_slow),
      /* 8 */
      new Config(32, 258, 258, 4096, deflate_slow)
      /* 9 max compression */
    ];
    var lm_init = (s) => {
      s.window_size = 2 * s.w_size;
      zero(s.head);
      s.max_lazy_match = configuration_table[s.level].max_lazy;
      s.good_match = configuration_table[s.level].good_length;
      s.nice_match = configuration_table[s.level].nice_length;
      s.max_chain_length = configuration_table[s.level].max_chain;
      s.strstart = 0;
      s.block_start = 0;
      s.lookahead = 0;
      s.insert = 0;
      s.match_length = s.prev_length = MIN_MATCH - 1;
      s.match_available = 0;
      s.ins_h = 0;
    };
    function DeflateState() {
      this.strm = null;
      this.status = 0;
      this.pending_buf = null;
      this.pending_buf_size = 0;
      this.pending_out = 0;
      this.pending = 0;
      this.wrap = 0;
      this.gzhead = null;
      this.gzindex = 0;
      this.method = Z_DEFLATED;
      this.last_flush = -1;
      this.w_size = 0;
      this.w_bits = 0;
      this.w_mask = 0;
      this.window = null;
      this.window_size = 0;
      this.prev = null;
      this.head = null;
      this.ins_h = 0;
      this.hash_size = 0;
      this.hash_bits = 0;
      this.hash_mask = 0;
      this.hash_shift = 0;
      this.block_start = 0;
      this.match_length = 0;
      this.prev_match = 0;
      this.match_available = 0;
      this.strstart = 0;
      this.match_start = 0;
      this.lookahead = 0;
      this.prev_length = 0;
      this.max_chain_length = 0;
      this.max_lazy_match = 0;
      this.level = 0;
      this.strategy = 0;
      this.good_match = 0;
      this.nice_match = 0;
      this.dyn_ltree = new Uint16Array(HEAP_SIZE * 2);
      this.dyn_dtree = new Uint16Array((2 * D_CODES + 1) * 2);
      this.bl_tree = new Uint16Array((2 * BL_CODES + 1) * 2);
      zero(this.dyn_ltree);
      zero(this.dyn_dtree);
      zero(this.bl_tree);
      this.l_desc = null;
      this.d_desc = null;
      this.bl_desc = null;
      this.bl_count = new Uint16Array(MAX_BITS + 1);
      this.heap = new Uint16Array(2 * L_CODES + 1);
      zero(this.heap);
      this.heap_len = 0;
      this.heap_max = 0;
      this.depth = new Uint16Array(2 * L_CODES + 1);
      zero(this.depth);
      this.sym_buf = 0;
      this.lit_bufsize = 0;
      this.sym_next = 0;
      this.sym_end = 0;
      this.opt_len = 0;
      this.static_len = 0;
      this.matches = 0;
      this.insert = 0;
      this.bi_buf = 0;
      this.bi_valid = 0;
    }
    var deflateStateCheck = (strm) => {
      if (!strm) {
        return 1;
      }
      const s = strm.state;
      if (!s || s.strm !== strm || s.status !== INIT_STATE && //#ifdef GZIP
      s.status !== GZIP_STATE && //#endif
      s.status !== EXTRA_STATE && s.status !== NAME_STATE && s.status !== COMMENT_STATE && s.status !== HCRC_STATE && s.status !== BUSY_STATE && s.status !== FINISH_STATE) {
        return 1;
      }
      return 0;
    };
    var deflateResetKeep = (strm) => {
      if (deflateStateCheck(strm)) {
        return err(strm, Z_STREAM_ERROR);
      }
      strm.total_in = strm.total_out = 0;
      strm.data_type = Z_UNKNOWN;
      const s = strm.state;
      s.pending = 0;
      s.pending_out = 0;
      if (s.wrap < 0) {
        s.wrap = -s.wrap;
      }
      s.status = //#ifdef GZIP
      s.wrap === 2 ? GZIP_STATE : (
        //#endif
        s.wrap ? INIT_STATE : BUSY_STATE
      );
      strm.adler = s.wrap === 2 ? 0 : 1;
      s.last_flush = -2;
      _tr_init(s);
      return Z_OK;
    };
    var deflateReset = (strm) => {
      const ret = deflateResetKeep(strm);
      if (ret === Z_OK) {
        lm_init(strm.state);
      }
      return ret;
    };
    var deflateSetHeader = (strm, head) => {
      if (deflateStateCheck(strm) || strm.state.wrap !== 2) {
        return Z_STREAM_ERROR;
      }
      strm.state.gzhead = head;
      return Z_OK;
    };
    var deflateInit2 = (strm, level, method, windowBits, memLevel, strategy) => {
      if (!strm) {
        return Z_STREAM_ERROR;
      }
      let wrap = 1;
      if (level === Z_DEFAULT_COMPRESSION) {
        level = 6;
      }
      if (windowBits < 0) {
        wrap = 0;
        windowBits = -windowBits;
      } else if (windowBits > 15) {
        wrap = 2;
        windowBits -= 16;
      }
      if (memLevel < 1 || memLevel > MAX_MEM_LEVEL || method !== Z_DEFLATED || windowBits < 8 || windowBits > 15 || level < 0 || level > 9 || strategy < 0 || strategy > Z_FIXED || windowBits === 8 && wrap !== 1) {
        return err(strm, Z_STREAM_ERROR);
      }
      if (windowBits === 8) {
        windowBits = 9;
      }
      const s = new DeflateState();
      strm.state = s;
      s.strm = strm;
      s.status = INIT_STATE;
      s.wrap = wrap;
      s.gzhead = null;
      s.w_bits = windowBits;
      s.w_size = 1 << s.w_bits;
      s.w_mask = s.w_size - 1;
      s.hash_bits = memLevel + 7;
      s.hash_size = 1 << s.hash_bits;
      s.hash_mask = s.hash_size - 1;
      s.hash_shift = ~~((s.hash_bits + MIN_MATCH - 1) / MIN_MATCH);
      s.window = new Uint8Array(s.w_size * 2);
      s.head = new Uint16Array(s.hash_size);
      s.prev = new Uint16Array(s.w_size);
      s.lit_bufsize = 1 << memLevel + 6;
      s.pending_buf_size = s.lit_bufsize * 4;
      s.pending_buf = new Uint8Array(s.pending_buf_size);
      s.sym_buf = s.lit_bufsize;
      s.sym_end = (s.lit_bufsize - 1) * 3;
      s.level = level;
      s.strategy = strategy;
      s.method = method;
      return deflateReset(strm);
    };
    var deflateInit = (strm, level) => {
      return deflateInit2(strm, level, Z_DEFLATED, MAX_WBITS, DEF_MEM_LEVEL, Z_DEFAULT_STRATEGY);
    };
    var deflate = (strm, flush) => {
      if (deflateStateCheck(strm) || flush > Z_BLOCK || flush < 0) {
        return strm ? err(strm, Z_STREAM_ERROR) : Z_STREAM_ERROR;
      }
      const s = strm.state;
      if (!strm.output || strm.avail_in !== 0 && !strm.input || s.status === FINISH_STATE && flush !== Z_FINISH) {
        return err(strm, strm.avail_out === 0 ? Z_BUF_ERROR : Z_STREAM_ERROR);
      }
      const old_flush = s.last_flush;
      s.last_flush = flush;
      if (s.pending !== 0) {
        flush_pending(strm);
        if (strm.avail_out === 0) {
          s.last_flush = -1;
          return Z_OK;
        }
      } else if (strm.avail_in === 0 && rank(flush) <= rank(old_flush) && flush !== Z_FINISH) {
        return err(strm, Z_BUF_ERROR);
      }
      if (s.status === FINISH_STATE && strm.avail_in !== 0) {
        return err(strm, Z_BUF_ERROR);
      }
      if (s.status === INIT_STATE && s.wrap === 0) {
        s.status = BUSY_STATE;
      }
      if (s.status === INIT_STATE) {
        let header = Z_DEFLATED + (s.w_bits - 8 << 4) << 8;
        let level_flags = -1;
        if (s.strategy >= Z_HUFFMAN_ONLY || s.level < 2) {
          level_flags = 0;
        } else if (s.level < 6) {
          level_flags = 1;
        } else if (s.level === 6) {
          level_flags = 2;
        } else {
          level_flags = 3;
        }
        header |= level_flags << 6;
        if (s.strstart !== 0) {
          header |= PRESET_DICT;
        }
        header += 31 - header % 31;
        putShortMSB(s, header);
        if (s.strstart !== 0) {
          putShortMSB(s, strm.adler >>> 16);
          putShortMSB(s, strm.adler & 65535);
        }
        strm.adler = 1;
        s.status = BUSY_STATE;
        flush_pending(strm);
        if (s.pending !== 0) {
          s.last_flush = -1;
          return Z_OK;
        }
      }
      if (s.status === GZIP_STATE) {
        strm.adler = 0;
        put_byte(s, 31);
        put_byte(s, 139);
        put_byte(s, 8);
        if (!s.gzhead) {
          put_byte(s, 0);
          put_byte(s, 0);
          put_byte(s, 0);
          put_byte(s, 0);
          put_byte(s, 0);
          put_byte(s, s.level === 9 ? 2 : s.strategy >= Z_HUFFMAN_ONLY || s.level < 2 ? 4 : 0);
          put_byte(s, OS_CODE);
          s.status = BUSY_STATE;
          flush_pending(strm);
          if (s.pending !== 0) {
            s.last_flush = -1;
            return Z_OK;
          }
        } else {
          put_byte(
            s,
            (s.gzhead.text ? 1 : 0) + (s.gzhead.hcrc ? 2 : 0) + (!s.gzhead.extra ? 0 : 4) + (!s.gzhead.name ? 0 : 8) + (!s.gzhead.comment ? 0 : 16)
          );
          put_byte(s, s.gzhead.time & 255);
          put_byte(s, s.gzhead.time >> 8 & 255);
          put_byte(s, s.gzhead.time >> 16 & 255);
          put_byte(s, s.gzhead.time >> 24 & 255);
          put_byte(s, s.level === 9 ? 2 : s.strategy >= Z_HUFFMAN_ONLY || s.level < 2 ? 4 : 0);
          put_byte(s, s.gzhead.os & 255);
          if (s.gzhead.extra && s.gzhead.extra.length) {
            put_byte(s, s.gzhead.extra.length & 255);
            put_byte(s, s.gzhead.extra.length >> 8 & 255);
          }
          if (s.gzhead.hcrc) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending, 0);
          }
          s.gzindex = 0;
          s.status = EXTRA_STATE;
        }
      }
      if (s.status === EXTRA_STATE) {
        if (s.gzhead.extra) {
          let beg = s.pending;
          let left = (s.gzhead.extra.length & 65535) - s.gzindex;
          while (s.pending + left > s.pending_buf_size) {
            let copy = s.pending_buf_size - s.pending;
            s.pending_buf.set(s.gzhead.extra.subarray(s.gzindex, s.gzindex + copy), s.pending);
            s.pending = s.pending_buf_size;
            if (s.gzhead.hcrc && s.pending > beg) {
              strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
            }
            s.gzindex += copy;
            flush_pending(strm);
            if (s.pending !== 0) {
              s.last_flush = -1;
              return Z_OK;
            }
            beg = 0;
            left -= copy;
          }
          let gzhead_extra = new Uint8Array(s.gzhead.extra);
          s.pending_buf.set(gzhead_extra.subarray(s.gzindex, s.gzindex + left), s.pending);
          s.pending += left;
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
          s.gzindex = 0;
        }
        s.status = NAME_STATE;
      }
      if (s.status === NAME_STATE) {
        if (s.gzhead.name) {
          let beg = s.pending;
          let val;
          do {
            if (s.pending === s.pending_buf_size) {
              if (s.gzhead.hcrc && s.pending > beg) {
                strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
              }
              flush_pending(strm);
              if (s.pending !== 0) {
                s.last_flush = -1;
                return Z_OK;
              }
              beg = 0;
            }
            if (s.gzindex < s.gzhead.name.length) {
              val = s.gzhead.name.charCodeAt(s.gzindex++) & 255;
            } else {
              val = 0;
            }
            put_byte(s, val);
          } while (val !== 0);
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
          s.gzindex = 0;
        }
        s.status = COMMENT_STATE;
      }
      if (s.status === COMMENT_STATE) {
        if (s.gzhead.comment) {
          let beg = s.pending;
          let val;
          do {
            if (s.pending === s.pending_buf_size) {
              if (s.gzhead.hcrc && s.pending > beg) {
                strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
              }
              flush_pending(strm);
              if (s.pending !== 0) {
                s.last_flush = -1;
                return Z_OK;
              }
              beg = 0;
            }
            if (s.gzindex < s.gzhead.comment.length) {
              val = s.gzhead.comment.charCodeAt(s.gzindex++) & 255;
            } else {
              val = 0;
            }
            put_byte(s, val);
          } while (val !== 0);
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
        }
        s.status = HCRC_STATE;
      }
      if (s.status === HCRC_STATE) {
        if (s.gzhead.hcrc) {
          if (s.pending + 2 > s.pending_buf_size) {
            flush_pending(strm);
            if (s.pending !== 0) {
              s.last_flush = -1;
              return Z_OK;
            }
          }
          put_byte(s, strm.adler & 255);
          put_byte(s, strm.adler >> 8 & 255);
          strm.adler = 0;
        }
        s.status = BUSY_STATE;
        flush_pending(strm);
        if (s.pending !== 0) {
          s.last_flush = -1;
          return Z_OK;
        }
      }
      if (strm.avail_in !== 0 || s.lookahead !== 0 || flush !== Z_NO_FLUSH && s.status !== FINISH_STATE) {
        let bstate = s.level === 0 ? deflate_stored(s, flush) : s.strategy === Z_HUFFMAN_ONLY ? deflate_huff(s, flush) : s.strategy === Z_RLE ? deflate_rle(s, flush) : configuration_table[s.level].func(s, flush);
        if (bstate === BS_FINISH_STARTED || bstate === BS_FINISH_DONE) {
          s.status = FINISH_STATE;
        }
        if (bstate === BS_NEED_MORE || bstate === BS_FINISH_STARTED) {
          if (strm.avail_out === 0) {
            s.last_flush = -1;
          }
          return Z_OK;
        }
        if (bstate === BS_BLOCK_DONE) {
          if (flush === Z_PARTIAL_FLUSH) {
            _tr_align(s);
          } else if (flush !== Z_BLOCK) {
            _tr_stored_block(s, 0, 0, false);
            if (flush === Z_FULL_FLUSH) {
              zero(s.head);
              if (s.lookahead === 0) {
                s.strstart = 0;
                s.block_start = 0;
                s.insert = 0;
              }
            }
          }
          flush_pending(strm);
          if (strm.avail_out === 0) {
            s.last_flush = -1;
            return Z_OK;
          }
        }
      }
      if (flush !== Z_FINISH) {
        return Z_OK;
      }
      if (s.wrap <= 0) {
        return Z_STREAM_END;
      }
      if (s.wrap === 2) {
        put_byte(s, strm.adler & 255);
        put_byte(s, strm.adler >> 8 & 255);
        put_byte(s, strm.adler >> 16 & 255);
        put_byte(s, strm.adler >> 24 & 255);
        put_byte(s, strm.total_in & 255);
        put_byte(s, strm.total_in >> 8 & 255);
        put_byte(s, strm.total_in >> 16 & 255);
        put_byte(s, strm.total_in >> 24 & 255);
      } else {
        putShortMSB(s, strm.adler >>> 16);
        putShortMSB(s, strm.adler & 65535);
      }
      flush_pending(strm);
      if (s.wrap > 0) {
        s.wrap = -s.wrap;
      }
      return s.pending !== 0 ? Z_OK : Z_STREAM_END;
    };
    var deflateEnd = (strm) => {
      if (deflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const status = strm.state.status;
      strm.state = null;
      return status === BUSY_STATE ? err(strm, Z_DATA_ERROR) : Z_OK;
    };
    var deflateSetDictionary = (strm, dictionary) => {
      let dictLength = dictionary.length;
      if (deflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const s = strm.state;
      const wrap = s.wrap;
      if (wrap === 2 || wrap === 1 && s.status !== INIT_STATE || s.lookahead) {
        return Z_STREAM_ERROR;
      }
      if (wrap === 1) {
        strm.adler = adler32(strm.adler, dictionary, dictLength, 0);
      }
      s.wrap = 0;
      if (dictLength >= s.w_size) {
        if (wrap === 0) {
          zero(s.head);
          s.strstart = 0;
          s.block_start = 0;
          s.insert = 0;
        }
        let tmpDict = new Uint8Array(s.w_size);
        tmpDict.set(dictionary.subarray(dictLength - s.w_size, dictLength), 0);
        dictionary = tmpDict;
        dictLength = s.w_size;
      }
      const avail = strm.avail_in;
      const next = strm.next_in;
      const input = strm.input;
      strm.avail_in = dictLength;
      strm.next_in = 0;
      strm.input = dictionary;
      fill_window(s);
      while (s.lookahead >= MIN_MATCH) {
        let str2 = s.strstart;
        let n = s.lookahead - (MIN_MATCH - 1);
        do {
          s.ins_h = HASH(s, s.ins_h, s.window[str2 + MIN_MATCH - 1]);
          s.prev[str2 & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = str2;
          str2++;
        } while (--n);
        s.strstart = str2;
        s.lookahead = MIN_MATCH - 1;
        fill_window(s);
      }
      s.strstart += s.lookahead;
      s.block_start = s.strstart;
      s.insert = s.lookahead;
      s.lookahead = 0;
      s.match_length = s.prev_length = MIN_MATCH - 1;
      s.match_available = 0;
      strm.next_in = next;
      strm.input = input;
      strm.avail_in = avail;
      s.wrap = wrap;
      return Z_OK;
    };
    module2.exports.deflateInit = deflateInit;
    module2.exports.deflateInit2 = deflateInit2;
    module2.exports.deflateReset = deflateReset;
    module2.exports.deflateResetKeep = deflateResetKeep;
    module2.exports.deflateSetHeader = deflateSetHeader;
    module2.exports.deflate = deflate;
    module2.exports.deflateEnd = deflateEnd;
    module2.exports.deflateSetDictionary = deflateSetDictionary;
    module2.exports.deflateInfo = "pako deflate (from Nodeca project)";
  }
});

// node_modules/pako/lib/utils/common.js
var require_common = __commonJS({
  "node_modules/pako/lib/utils/common.js"(exports2, module2) {
    "use strict";
    var _has = (obj, key) => {
      return Object.prototype.hasOwnProperty.call(obj, key);
    };
    module2.exports.assign = function(obj) {
      const sources = Array.prototype.slice.call(arguments, 1);
      while (sources.length) {
        const source = sources.shift();
        if (!source) {
          continue;
        }
        if (typeof source !== "object") {
          throw new TypeError(source + "must be non-object");
        }
        for (const p in source) {
          if (_has(source, p)) {
            obj[p] = source[p];
          }
        }
      }
      return obj;
    };
    module2.exports.flattenChunks = (chunks) => {
      let len = 0;
      for (let i = 0, l = chunks.length; i < l; i++) {
        len += chunks[i].length;
      }
      const result = new Uint8Array(len);
      for (let i = 0, pos = 0, l = chunks.length; i < l; i++) {
        let chunk = chunks[i];
        result.set(chunk, pos);
        pos += chunk.length;
      }
      return result;
    };
  }
});

// node_modules/pako/lib/utils/strings.js
var require_strings = __commonJS({
  "node_modules/pako/lib/utils/strings.js"(exports2, module2) {
    "use strict";
    var STR_APPLY_UIA_OK = true;
    try {
      String.fromCharCode.apply(null, new Uint8Array(1));
    } catch (__) {
      STR_APPLY_UIA_OK = false;
    }
    var _utf8len = new Uint8Array(256);
    for (let q = 0; q < 256; q++) {
      _utf8len[q] = q >= 252 ? 6 : q >= 248 ? 5 : q >= 240 ? 4 : q >= 224 ? 3 : q >= 192 ? 2 : 1;
    }
    _utf8len[254] = _utf8len[254] = 1;
    module2.exports.string2buf = (str2) => {
      if (typeof TextEncoder === "function" && TextEncoder.prototype.encode) {
        return new TextEncoder().encode(str2);
      }
      let buf, c, c2, m_pos, i, str_len = str2.length, buf_len = 0;
      for (m_pos = 0; m_pos < str_len; m_pos++) {
        c = str2.charCodeAt(m_pos);
        if ((c & 64512) === 55296 && m_pos + 1 < str_len) {
          c2 = str2.charCodeAt(m_pos + 1);
          if ((c2 & 64512) === 56320) {
            c = 65536 + (c - 55296 << 10) + (c2 - 56320);
            m_pos++;
          }
        }
        buf_len += c < 128 ? 1 : c < 2048 ? 2 : c < 65536 ? 3 : 4;
      }
      buf = new Uint8Array(buf_len);
      for (i = 0, m_pos = 0; i < buf_len; m_pos++) {
        c = str2.charCodeAt(m_pos);
        if ((c & 64512) === 55296 && m_pos + 1 < str_len) {
          c2 = str2.charCodeAt(m_pos + 1);
          if ((c2 & 64512) === 56320) {
            c = 65536 + (c - 55296 << 10) + (c2 - 56320);
            m_pos++;
          }
        }
        if (c < 128) {
          buf[i++] = c;
        } else if (c < 2048) {
          buf[i++] = 192 | c >>> 6;
          buf[i++] = 128 | c & 63;
        } else if (c < 65536) {
          buf[i++] = 224 | c >>> 12;
          buf[i++] = 128 | c >>> 6 & 63;
          buf[i++] = 128 | c & 63;
        } else {
          buf[i++] = 240 | c >>> 18;
          buf[i++] = 128 | c >>> 12 & 63;
          buf[i++] = 128 | c >>> 6 & 63;
          buf[i++] = 128 | c & 63;
        }
      }
      return buf;
    };
    var buf2binstring = (buf, len) => {
      if (len < 65534) {
        if (buf.subarray && STR_APPLY_UIA_OK) {
          return String.fromCharCode.apply(null, buf.length === len ? buf : buf.subarray(0, len));
        }
      }
      let result = "";
      for (let i = 0; i < len; i++) {
        result += String.fromCharCode(buf[i]);
      }
      return result;
    };
    module2.exports.buf2string = (buf, max) => {
      const len = max || buf.length;
      if (typeof TextDecoder === "function" && TextDecoder.prototype.decode) {
        return new TextDecoder().decode(buf.subarray(0, max));
      }
      let i, out;
      const utf16buf = new Array(len * 2);
      for (out = 0, i = 0; i < len; ) {
        let c = buf[i++];
        if (c < 128) {
          utf16buf[out++] = c;
          continue;
        }
        let c_len = _utf8len[c];
        if (c_len > 4) {
          utf16buf[out++] = 65533;
          i += c_len - 1;
          continue;
        }
        c &= c_len === 2 ? 31 : c_len === 3 ? 15 : 7;
        while (c_len > 1 && i < len) {
          c = c << 6 | buf[i++] & 63;
          c_len--;
        }
        if (c_len > 1) {
          utf16buf[out++] = 65533;
          continue;
        }
        if (c < 65536) {
          utf16buf[out++] = c;
        } else {
          c -= 65536;
          utf16buf[out++] = 55296 | c >> 10 & 1023;
          utf16buf[out++] = 56320 | c & 1023;
        }
      }
      return buf2binstring(utf16buf, out);
    };
    module2.exports.utf8border = (buf, max) => {
      max = max || buf.length;
      if (max > buf.length) {
        max = buf.length;
      }
      let pos = max - 1;
      while (pos >= 0 && (buf[pos] & 192) === 128) {
        pos--;
      }
      if (pos < 0) {
        return max;
      }
      if (pos === 0) {
        return max;
      }
      return pos + _utf8len[buf[pos]] > max ? pos : max;
    };
  }
});

// node_modules/pako/lib/zlib/zstream.js
var require_zstream = __commonJS({
  "node_modules/pako/lib/zlib/zstream.js"(exports2, module2) {
    "use strict";
    function ZStream() {
      this.input = null;
      this.next_in = 0;
      this.avail_in = 0;
      this.total_in = 0;
      this.output = null;
      this.next_out = 0;
      this.avail_out = 0;
      this.total_out = 0;
      this.msg = "";
      this.state = null;
      this.data_type = 2;
      this.adler = 0;
    }
    module2.exports = ZStream;
  }
});

// node_modules/pako/lib/deflate.js
var require_deflate2 = __commonJS({
  "node_modules/pako/lib/deflate.js"(exports2, module2) {
    "use strict";
    var zlib_deflate = require_deflate();
    var utils = require_common();
    var strings = require_strings();
    var msg = require_messages();
    var ZStream = require_zstream();
    var toString = Object.prototype.toString;
    var {
      Z_NO_FLUSH,
      Z_SYNC_FLUSH,
      Z_FULL_FLUSH,
      Z_FINISH,
      Z_OK,
      Z_STREAM_END,
      Z_DEFAULT_COMPRESSION,
      Z_DEFAULT_STRATEGY,
      Z_DEFLATED
    } = require_constants();
    function Deflate(options) {
      this.options = utils.assign({
        level: Z_DEFAULT_COMPRESSION,
        method: Z_DEFLATED,
        chunkSize: 16384,
        windowBits: 15,
        memLevel: 8,
        strategy: Z_DEFAULT_STRATEGY
      }, options || {});
      let opt = this.options;
      if (opt.raw && opt.windowBits > 0) {
        opt.windowBits = -opt.windowBits;
      } else if (opt.gzip && opt.windowBits > 0 && opt.windowBits < 16) {
        opt.windowBits += 16;
      }
      this.err = 0;
      this.msg = "";
      this.ended = false;
      this.chunks = [];
      this.strm = new ZStream();
      this.strm.avail_out = 0;
      let status = zlib_deflate.deflateInit2(
        this.strm,
        opt.level,
        opt.method,
        opt.windowBits,
        opt.memLevel,
        opt.strategy
      );
      if (status !== Z_OK) {
        throw new Error(msg[status]);
      }
      if (opt.header) {
        zlib_deflate.deflateSetHeader(this.strm, opt.header);
      }
      if (opt.dictionary) {
        let dict;
        if (typeof opt.dictionary === "string") {
          dict = strings.string2buf(opt.dictionary);
        } else if (toString.call(opt.dictionary) === "[object ArrayBuffer]") {
          dict = new Uint8Array(opt.dictionary);
        } else {
          dict = opt.dictionary;
        }
        status = zlib_deflate.deflateSetDictionary(this.strm, dict);
        if (status !== Z_OK) {
          throw new Error(msg[status]);
        }
        this._dict_set = true;
      }
    }
    Deflate.prototype.push = function(data, flush_mode) {
      const strm = this.strm;
      const chunkSize = this.options.chunkSize;
      let status, _flush_mode;
      if (this.ended) {
        return false;
      }
      if (flush_mode === ~~flush_mode)
        _flush_mode = flush_mode;
      else
        _flush_mode = flush_mode === true ? Z_FINISH : Z_NO_FLUSH;
      if (typeof data === "string") {
        strm.input = strings.string2buf(data);
      } else if (toString.call(data) === "[object ArrayBuffer]") {
        strm.input = new Uint8Array(data);
      } else {
        strm.input = data;
      }
      strm.next_in = 0;
      strm.avail_in = strm.input.length;
      for (; ; ) {
        if (strm.avail_out === 0) {
          strm.output = new Uint8Array(chunkSize);
          strm.next_out = 0;
          strm.avail_out = chunkSize;
        }
        if ((_flush_mode === Z_SYNC_FLUSH || _flush_mode === Z_FULL_FLUSH) && strm.avail_out <= 6) {
          this.onData(strm.output.subarray(0, strm.next_out));
          strm.avail_out = 0;
          continue;
        }
        status = zlib_deflate.deflate(strm, _flush_mode);
        if (status === Z_STREAM_END) {
          if (strm.next_out > 0) {
            this.onData(strm.output.subarray(0, strm.next_out));
          }
          status = zlib_deflate.deflateEnd(this.strm);
          this.onEnd(status);
          this.ended = true;
          return status === Z_OK;
        }
        if (strm.avail_out === 0) {
          this.onData(strm.output);
          continue;
        }
        if (_flush_mode > 0 && strm.next_out > 0) {
          this.onData(strm.output.subarray(0, strm.next_out));
          strm.avail_out = 0;
          continue;
        }
        if (strm.avail_in === 0)
          break;
      }
      return true;
    };
    Deflate.prototype.onData = function(chunk) {
      this.chunks.push(chunk);
    };
    Deflate.prototype.onEnd = function(status) {
      if (status === Z_OK) {
        this.result = utils.flattenChunks(this.chunks);
      }
      this.chunks = [];
      this.err = status;
      this.msg = this.strm.msg;
    };
    function deflate(input, options) {
      const deflator = new Deflate(options);
      deflator.push(input, true);
      if (deflator.err) {
        throw deflator.msg || msg[deflator.err];
      }
      return deflator.result;
    }
    function deflateRaw(input, options) {
      options = options || {};
      options.raw = true;
      return deflate(input, options);
    }
    function gzip(input, options) {
      options = options || {};
      options.gzip = true;
      return deflate(input, options);
    }
    module2.exports.Deflate = Deflate;
    module2.exports.deflate = deflate;
    module2.exports.deflateRaw = deflateRaw;
    module2.exports.gzip = gzip;
    module2.exports.constants = require_constants();
  }
});

// node_modules/pako/lib/zlib/inffast.js
var require_inffast = __commonJS({
  "node_modules/pako/lib/zlib/inffast.js"(exports2, module2) {
    "use strict";
    var BAD = 16209;
    var TYPE = 16191;
    module2.exports = function inflate_fast(strm, start) {
      let _in;
      let last;
      let _out;
      let beg;
      let end;
      let dmax;
      let wsize;
      let whave;
      let wnext;
      let s_window;
      let hold;
      let bits;
      let lcode;
      let dcode;
      let lmask;
      let dmask;
      let here;
      let op;
      let len;
      let dist;
      let from;
      let from_source;
      let input, output;
      const state = strm.state;
      _in = strm.next_in;
      input = strm.input;
      last = _in + (strm.avail_in - 5);
      _out = strm.next_out;
      output = strm.output;
      beg = _out - (start - strm.avail_out);
      end = _out + (strm.avail_out - 257);
      dmax = state.dmax;
      wsize = state.wsize;
      whave = state.whave;
      wnext = state.wnext;
      s_window = state.window;
      hold = state.hold;
      bits = state.bits;
      lcode = state.lencode;
      dcode = state.distcode;
      lmask = (1 << state.lenbits) - 1;
      dmask = (1 << state.distbits) - 1;
      top:
        do {
          if (bits < 15) {
            hold += input[_in++] << bits;
            bits += 8;
            hold += input[_in++] << bits;
            bits += 8;
          }
          here = lcode[hold & lmask];
          dolen:
            for (; ; ) {
              op = here >>> 24;
              hold >>>= op;
              bits -= op;
              op = here >>> 16 & 255;
              if (op === 0) {
                output[_out++] = here & 65535;
              } else if (op & 16) {
                len = here & 65535;
                op &= 15;
                if (op) {
                  if (bits < op) {
                    hold += input[_in++] << bits;
                    bits += 8;
                  }
                  len += hold & (1 << op) - 1;
                  hold >>>= op;
                  bits -= op;
                }
                if (bits < 15) {
                  hold += input[_in++] << bits;
                  bits += 8;
                  hold += input[_in++] << bits;
                  bits += 8;
                }
                here = dcode[hold & dmask];
                dodist:
                  for (; ; ) {
                    op = here >>> 24;
                    hold >>>= op;
                    bits -= op;
                    op = here >>> 16 & 255;
                    if (op & 16) {
                      dist = here & 65535;
                      op &= 15;
                      if (bits < op) {
                        hold += input[_in++] << bits;
                        bits += 8;
                        if (bits < op) {
                          hold += input[_in++] << bits;
                          bits += 8;
                        }
                      }
                      dist += hold & (1 << op) - 1;
                      if (dist > dmax) {
                        strm.msg = "invalid distance too far back";
                        state.mode = BAD;
                        break top;
                      }
                      hold >>>= op;
                      bits -= op;
                      op = _out - beg;
                      if (dist > op) {
                        op = dist - op;
                        if (op > whave) {
                          if (state.sane) {
                            strm.msg = "invalid distance too far back";
                            state.mode = BAD;
                            break top;
                          }
                        }
                        from = 0;
                        from_source = s_window;
                        if (wnext === 0) {
                          from += wsize - op;
                          if (op < len) {
                            len -= op;
                            do {
                              output[_out++] = s_window[from++];
                            } while (--op);
                            from = _out - dist;
                            from_source = output;
                          }
                        } else if (wnext < op) {
                          from += wsize + wnext - op;
                          op -= wnext;
                          if (op < len) {
                            len -= op;
                            do {
                              output[_out++] = s_window[from++];
                            } while (--op);
                            from = 0;
                            if (wnext < len) {
                              op = wnext;
                              len -= op;
                              do {
                                output[_out++] = s_window[from++];
                              } while (--op);
                              from = _out - dist;
                              from_source = output;
                            }
                          }
                        } else {
                          from += wnext - op;
                          if (op < len) {
                            len -= op;
                            do {
                              output[_out++] = s_window[from++];
                            } while (--op);
                            from = _out - dist;
                            from_source = output;
                          }
                        }
                        while (len > 2) {
                          output[_out++] = from_source[from++];
                          output[_out++] = from_source[from++];
                          output[_out++] = from_source[from++];
                          len -= 3;
                        }
                        if (len) {
                          output[_out++] = from_source[from++];
                          if (len > 1) {
                            output[_out++] = from_source[from++];
                          }
                        }
                      } else {
                        from = _out - dist;
                        do {
                          output[_out++] = output[from++];
                          output[_out++] = output[from++];
                          output[_out++] = output[from++];
                          len -= 3;
                        } while (len > 2);
                        if (len) {
                          output[_out++] = output[from++];
                          if (len > 1) {
                            output[_out++] = output[from++];
                          }
                        }
                      }
                    } else if ((op & 64) === 0) {
                      here = dcode[(here & 65535) + (hold & (1 << op) - 1)];
                      continue dodist;
                    } else {
                      strm.msg = "invalid distance code";
                      state.mode = BAD;
                      break top;
                    }
                    break;
                  }
              } else if ((op & 64) === 0) {
                here = lcode[(here & 65535) + (hold & (1 << op) - 1)];
                continue dolen;
              } else if (op & 32) {
                state.mode = TYPE;
                break top;
              } else {
                strm.msg = "invalid literal/length code";
                state.mode = BAD;
                break top;
              }
              break;
            }
        } while (_in < last && _out < end);
      len = bits >> 3;
      _in -= len;
      bits -= len << 3;
      hold &= (1 << bits) - 1;
      strm.next_in = _in;
      strm.next_out = _out;
      strm.avail_in = _in < last ? 5 + (last - _in) : 5 - (_in - last);
      strm.avail_out = _out < end ? 257 + (end - _out) : 257 - (_out - end);
      state.hold = hold;
      state.bits = bits;
      return;
    };
  }
});

// node_modules/pako/lib/zlib/inftrees.js
var require_inftrees = __commonJS({
  "node_modules/pako/lib/zlib/inftrees.js"(exports2, module2) {
    "use strict";
    var MAXBITS = 15;
    var ENOUGH_LENS = 852;
    var ENOUGH_DISTS = 592;
    var CODES = 0;
    var LENS = 1;
    var DISTS = 2;
    var lbase = new Uint16Array([
      /* Length codes 257..285 base */
      3,
      4,
      5,
      6,
      7,
      8,
      9,
      10,
      11,
      13,
      15,
      17,
      19,
      23,
      27,
      31,
      35,
      43,
      51,
      59,
      67,
      83,
      99,
      115,
      131,
      163,
      195,
      227,
      258,
      0,
      0
    ]);
    var lext = new Uint8Array([
      /* Length codes 257..285 extra */
      16,
      16,
      16,
      16,
      16,
      16,
      16,
      16,
      17,
      17,
      17,
      17,
      18,
      18,
      18,
      18,
      19,
      19,
      19,
      19,
      20,
      20,
      20,
      20,
      21,
      21,
      21,
      21,
      16,
      72,
      78
    ]);
    var dbase = new Uint16Array([
      /* Distance codes 0..29 base */
      1,
      2,
      3,
      4,
      5,
      7,
      9,
      13,
      17,
      25,
      33,
      49,
      65,
      97,
      129,
      193,
      257,
      385,
      513,
      769,
      1025,
      1537,
      2049,
      3073,
      4097,
      6145,
      8193,
      12289,
      16385,
      24577,
      0,
      0
    ]);
    var dext = new Uint8Array([
      /* Distance codes 0..29 extra */
      16,
      16,
      16,
      16,
      17,
      17,
      18,
      18,
      19,
      19,
      20,
      20,
      21,
      21,
      22,
      22,
      23,
      23,
      24,
      24,
      25,
      25,
      26,
      26,
      27,
      27,
      28,
      28,
      29,
      29,
      64,
      64
    ]);
    var inflate_table = (type, lens, lens_index, codes, table, table_index, work, opts2) => {
      const bits = opts2.bits;
      let len = 0;
      let sym = 0;
      let min = 0, max = 0;
      let root = 0;
      let curr = 0;
      let drop = 0;
      let left = 0;
      let used = 0;
      let huff = 0;
      let incr;
      let fill;
      let low;
      let mask;
      let next;
      let base = null;
      let match;
      const count = new Uint16Array(MAXBITS + 1);
      const offs = new Uint16Array(MAXBITS + 1);
      let extra = null;
      let here_bits, here_op, here_val;
      for (len = 0; len <= MAXBITS; len++) {
        count[len] = 0;
      }
      for (sym = 0; sym < codes; sym++) {
        count[lens[lens_index + sym]]++;
      }
      root = bits;
      for (max = MAXBITS; max >= 1; max--) {
        if (count[max] !== 0) {
          break;
        }
      }
      if (root > max) {
        root = max;
      }
      if (max === 0) {
        table[table_index++] = 1 << 24 | 64 << 16 | 0;
        table[table_index++] = 1 << 24 | 64 << 16 | 0;
        opts2.bits = 1;
        return 0;
      }
      for (min = 1; min < max; min++) {
        if (count[min] !== 0) {
          break;
        }
      }
      if (root < min) {
        root = min;
      }
      left = 1;
      for (len = 1; len <= MAXBITS; len++) {
        left <<= 1;
        left -= count[len];
        if (left < 0) {
          return -1;
        }
      }
      if (left > 0 && (type === CODES || max !== 1)) {
        return -1;
      }
      offs[1] = 0;
      for (len = 1; len < MAXBITS; len++) {
        offs[len + 1] = offs[len] + count[len];
      }
      for (sym = 0; sym < codes; sym++) {
        if (lens[lens_index + sym] !== 0) {
          work[offs[lens[lens_index + sym]]++] = sym;
        }
      }
      if (type === CODES) {
        base = extra = work;
        match = 20;
      } else if (type === LENS) {
        base = lbase;
        extra = lext;
        match = 257;
      } else {
        base = dbase;
        extra = dext;
        match = 0;
      }
      huff = 0;
      sym = 0;
      len = min;
      next = table_index;
      curr = root;
      drop = 0;
      low = -1;
      used = 1 << root;
      mask = used - 1;
      if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
        return 1;
      }
      for (; ; ) {
        here_bits = len - drop;
        if (work[sym] + 1 < match) {
          here_op = 0;
          here_val = work[sym];
        } else if (work[sym] >= match) {
          here_op = extra[work[sym] - match];
          here_val = base[work[sym] - match];
        } else {
          here_op = 32 + 64;
          here_val = 0;
        }
        incr = 1 << len - drop;
        fill = 1 << curr;
        min = fill;
        do {
          fill -= incr;
          table[next + (huff >> drop) + fill] = here_bits << 24 | here_op << 16 | here_val | 0;
        } while (fill !== 0);
        incr = 1 << len - 1;
        while (huff & incr) {
          incr >>= 1;
        }
        if (incr !== 0) {
          huff &= incr - 1;
          huff += incr;
        } else {
          huff = 0;
        }
        sym++;
        if (--count[len] === 0) {
          if (len === max) {
            break;
          }
          len = lens[lens_index + work[sym]];
        }
        if (len > root && (huff & mask) !== low) {
          if (drop === 0) {
            drop = root;
          }
          next += min;
          curr = len - drop;
          left = 1 << curr;
          while (curr + drop < max) {
            left -= count[curr + drop];
            if (left <= 0) {
              break;
            }
            curr++;
            left <<= 1;
          }
          used += 1 << curr;
          if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
            return 1;
          }
          low = huff & mask;
          table[low] = root << 24 | curr << 16 | next - table_index | 0;
        }
      }
      if (huff !== 0) {
        table[next + huff] = len - drop << 24 | 64 << 16 | 0;
      }
      opts2.bits = root;
      return 0;
    };
    module2.exports = inflate_table;
  }
});

// node_modules/pako/lib/zlib/inflate.js
var require_inflate = __commonJS({
  "node_modules/pako/lib/zlib/inflate.js"(exports2, module2) {
    "use strict";
    var adler32 = require_adler32();
    var crc32 = require_crc32();
    var inflate_fast = require_inffast();
    var inflate_table = require_inftrees();
    var CODES = 0;
    var LENS = 1;
    var DISTS = 2;
    var {
      Z_FINISH,
      Z_BLOCK,
      Z_TREES,
      Z_OK,
      Z_STREAM_END,
      Z_NEED_DICT,
      Z_STREAM_ERROR,
      Z_DATA_ERROR,
      Z_MEM_ERROR,
      Z_BUF_ERROR,
      Z_DEFLATED
    } = require_constants();
    var HEAD = 16180;
    var FLAGS = 16181;
    var TIME = 16182;
    var OS = 16183;
    var EXLEN = 16184;
    var EXTRA = 16185;
    var NAME = 16186;
    var COMMENT = 16187;
    var HCRC = 16188;
    var DICTID = 16189;
    var DICT = 16190;
    var TYPE = 16191;
    var TYPEDO = 16192;
    var STORED = 16193;
    var COPY_ = 16194;
    var COPY = 16195;
    var TABLE = 16196;
    var LENLENS = 16197;
    var CODELENS = 16198;
    var LEN_ = 16199;
    var LEN = 16200;
    var LENEXT = 16201;
    var DIST = 16202;
    var DISTEXT = 16203;
    var MATCH = 16204;
    var LIT = 16205;
    var CHECK = 16206;
    var LENGTH = 16207;
    var DONE = 16208;
    var BAD = 16209;
    var MEM = 16210;
    var SYNC = 16211;
    var ENOUGH_LENS = 852;
    var ENOUGH_DISTS = 592;
    var MAX_WBITS = 15;
    var DEF_WBITS = MAX_WBITS;
    var zswap32 = (q) => {
      return (q >>> 24 & 255) + (q >>> 8 & 65280) + ((q & 65280) << 8) + ((q & 255) << 24);
    };
    function InflateState() {
      this.strm = null;
      this.mode = 0;
      this.last = false;
      this.wrap = 0;
      this.havedict = false;
      this.flags = 0;
      this.dmax = 0;
      this.check = 0;
      this.total = 0;
      this.head = null;
      this.wbits = 0;
      this.wsize = 0;
      this.whave = 0;
      this.wnext = 0;
      this.window = null;
      this.hold = 0;
      this.bits = 0;
      this.length = 0;
      this.offset = 0;
      this.extra = 0;
      this.lencode = null;
      this.distcode = null;
      this.lenbits = 0;
      this.distbits = 0;
      this.ncode = 0;
      this.nlen = 0;
      this.ndist = 0;
      this.have = 0;
      this.next = null;
      this.lens = new Uint16Array(320);
      this.work = new Uint16Array(288);
      this.lendyn = null;
      this.distdyn = null;
      this.sane = 0;
      this.back = 0;
      this.was = 0;
    }
    var inflateStateCheck = (strm) => {
      if (!strm) {
        return 1;
      }
      const state = strm.state;
      if (!state || state.strm !== strm || state.mode < HEAD || state.mode > SYNC) {
        return 1;
      }
      return 0;
    };
    var inflateResetKeep = (strm) => {
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const state = strm.state;
      strm.total_in = strm.total_out = state.total = 0;
      strm.msg = "";
      if (state.wrap) {
        strm.adler = state.wrap & 1;
      }
      state.mode = HEAD;
      state.last = 0;
      state.havedict = 0;
      state.flags = -1;
      state.dmax = 32768;
      state.head = null;
      state.hold = 0;
      state.bits = 0;
      state.lencode = state.lendyn = new Int32Array(ENOUGH_LENS);
      state.distcode = state.distdyn = new Int32Array(ENOUGH_DISTS);
      state.sane = 1;
      state.back = -1;
      return Z_OK;
    };
    var inflateReset = (strm) => {
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const state = strm.state;
      state.wsize = 0;
      state.whave = 0;
      state.wnext = 0;
      return inflateResetKeep(strm);
    };
    var inflateReset2 = (strm, windowBits) => {
      let wrap;
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const state = strm.state;
      if (windowBits < 0) {
        wrap = 0;
        windowBits = -windowBits;
      } else {
        wrap = (windowBits >> 4) + 5;
        if (windowBits < 48) {
          windowBits &= 15;
        }
      }
      if (windowBits && (windowBits < 8 || windowBits > 15)) {
        return Z_STREAM_ERROR;
      }
      if (state.window !== null && state.wbits !== windowBits) {
        state.window = null;
      }
      state.wrap = wrap;
      state.wbits = windowBits;
      return inflateReset(strm);
    };
    var inflateInit2 = (strm, windowBits) => {
      if (!strm) {
        return Z_STREAM_ERROR;
      }
      const state = new InflateState();
      strm.state = state;
      state.strm = strm;
      state.window = null;
      state.mode = HEAD;
      const ret = inflateReset2(strm, windowBits);
      if (ret !== Z_OK) {
        strm.state = null;
      }
      return ret;
    };
    var inflateInit = (strm) => {
      return inflateInit2(strm, DEF_WBITS);
    };
    var virgin = true;
    var lenfix;
    var distfix;
    var fixedtables = (state) => {
      if (virgin) {
        lenfix = new Int32Array(512);
        distfix = new Int32Array(32);
        let sym = 0;
        while (sym < 144) {
          state.lens[sym++] = 8;
        }
        while (sym < 256) {
          state.lens[sym++] = 9;
        }
        while (sym < 280) {
          state.lens[sym++] = 7;
        }
        while (sym < 288) {
          state.lens[sym++] = 8;
        }
        inflate_table(LENS, state.lens, 0, 288, lenfix, 0, state.work, { bits: 9 });
        sym = 0;
        while (sym < 32) {
          state.lens[sym++] = 5;
        }
        inflate_table(DISTS, state.lens, 0, 32, distfix, 0, state.work, { bits: 5 });
        virgin = false;
      }
      state.lencode = lenfix;
      state.lenbits = 9;
      state.distcode = distfix;
      state.distbits = 5;
    };
    var updatewindow = (strm, src, end, copy) => {
      let dist;
      const state = strm.state;
      if (state.window === null) {
        state.wsize = 1 << state.wbits;
        state.wnext = 0;
        state.whave = 0;
        state.window = new Uint8Array(state.wsize);
      }
      if (copy >= state.wsize) {
        state.window.set(src.subarray(end - state.wsize, end), 0);
        state.wnext = 0;
        state.whave = state.wsize;
      } else {
        dist = state.wsize - state.wnext;
        if (dist > copy) {
          dist = copy;
        }
        state.window.set(src.subarray(end - copy, end - copy + dist), state.wnext);
        copy -= dist;
        if (copy) {
          state.window.set(src.subarray(end - copy, end), 0);
          state.wnext = copy;
          state.whave = state.wsize;
        } else {
          state.wnext += dist;
          if (state.wnext === state.wsize) {
            state.wnext = 0;
          }
          if (state.whave < state.wsize) {
            state.whave += dist;
          }
        }
      }
      return 0;
    };
    var inflate = (strm, flush) => {
      let state;
      let input, output;
      let next;
      let put;
      let have, left;
      let hold;
      let bits;
      let _in, _out;
      let copy;
      let from;
      let from_source;
      let here = 0;
      let here_bits, here_op, here_val;
      let last_bits, last_op, last_val;
      let len;
      let ret;
      const hbuf = new Uint8Array(4);
      let opts2;
      let n;
      const order = (
        /* permutation of code lengths */
        new Uint8Array([16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15])
      );
      if (inflateStateCheck(strm) || !strm.output || !strm.input && strm.avail_in !== 0) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if (state.mode === TYPE) {
        state.mode = TYPEDO;
      }
      put = strm.next_out;
      output = strm.output;
      left = strm.avail_out;
      next = strm.next_in;
      input = strm.input;
      have = strm.avail_in;
      hold = state.hold;
      bits = state.bits;
      _in = have;
      _out = left;
      ret = Z_OK;
      inf_leave:
        for (; ; ) {
          switch (state.mode) {
            case HEAD:
              if (state.wrap === 0) {
                state.mode = TYPEDO;
                break;
              }
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (state.wrap & 2 && hold === 35615) {
                if (state.wbits === 0) {
                  state.wbits = 15;
                }
                state.check = 0;
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
                hold = 0;
                bits = 0;
                state.mode = FLAGS;
                break;
              }
              if (state.head) {
                state.head.done = false;
              }
              if (!(state.wrap & 1) || /* check if zlib header allowed */
              (((hold & 255) << 8) + (hold >> 8)) % 31) {
                strm.msg = "incorrect header check";
                state.mode = BAD;
                break;
              }
              if ((hold & 15) !== Z_DEFLATED) {
                strm.msg = "unknown compression method";
                state.mode = BAD;
                break;
              }
              hold >>>= 4;
              bits -= 4;
              len = (hold & 15) + 8;
              if (state.wbits === 0) {
                state.wbits = len;
              }
              if (len > 15 || len > state.wbits) {
                strm.msg = "invalid window size";
                state.mode = BAD;
                break;
              }
              state.dmax = 1 << state.wbits;
              state.flags = 0;
              strm.adler = state.check = 1;
              state.mode = hold & 512 ? DICTID : TYPE;
              hold = 0;
              bits = 0;
              break;
            case FLAGS:
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.flags = hold;
              if ((state.flags & 255) !== Z_DEFLATED) {
                strm.msg = "unknown compression method";
                state.mode = BAD;
                break;
              }
              if (state.flags & 57344) {
                strm.msg = "unknown header flags set";
                state.mode = BAD;
                break;
              }
              if (state.head) {
                state.head.text = hold >> 8 & 1;
              }
              if (state.flags & 512 && state.wrap & 4) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = TIME;
            case TIME:
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (state.head) {
                state.head.time = hold;
              }
              if (state.flags & 512 && state.wrap & 4) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                hbuf[2] = hold >>> 16 & 255;
                hbuf[3] = hold >>> 24 & 255;
                state.check = crc32(state.check, hbuf, 4, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = OS;
            case OS:
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (state.head) {
                state.head.xflags = hold & 255;
                state.head.os = hold >> 8;
              }
              if (state.flags & 512 && state.wrap & 4) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = EXLEN;
            case EXLEN:
              if (state.flags & 1024) {
                while (bits < 16) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                state.length = hold;
                if (state.head) {
                  state.head.extra_len = hold;
                }
                if (state.flags & 512 && state.wrap & 4) {
                  hbuf[0] = hold & 255;
                  hbuf[1] = hold >>> 8 & 255;
                  state.check = crc32(state.check, hbuf, 2, 0);
                }
                hold = 0;
                bits = 0;
              } else if (state.head) {
                state.head.extra = null;
              }
              state.mode = EXTRA;
            case EXTRA:
              if (state.flags & 1024) {
                copy = state.length;
                if (copy > have) {
                  copy = have;
                }
                if (copy) {
                  if (state.head) {
                    len = state.head.extra_len - state.length;
                    if (!state.head.extra) {
                      state.head.extra = new Uint8Array(state.head.extra_len);
                    }
                    state.head.extra.set(
                      input.subarray(
                        next,
                        // extra field is limited to 65536 bytes
                        // - no need for additional size check
                        next + copy
                      ),
                      /*len + copy > state.head.extra_max - len ? state.head.extra_max : copy,*/
                      len
                    );
                  }
                  if (state.flags & 512 && state.wrap & 4) {
                    state.check = crc32(state.check, input, copy, next);
                  }
                  have -= copy;
                  next += copy;
                  state.length -= copy;
                }
                if (state.length) {
                  break inf_leave;
                }
              }
              state.length = 0;
              state.mode = NAME;
            case NAME:
              if (state.flags & 2048) {
                if (have === 0) {
                  break inf_leave;
                }
                copy = 0;
                do {
                  len = input[next + copy++];
                  if (state.head && len && state.length < 65536) {
                    state.head.name += String.fromCharCode(len);
                  }
                } while (len && copy < have);
                if (state.flags & 512 && state.wrap & 4) {
                  state.check = crc32(state.check, input, copy, next);
                }
                have -= copy;
                next += copy;
                if (len) {
                  break inf_leave;
                }
              } else if (state.head) {
                state.head.name = null;
              }
              state.length = 0;
              state.mode = COMMENT;
            case COMMENT:
              if (state.flags & 4096) {
                if (have === 0) {
                  break inf_leave;
                }
                copy = 0;
                do {
                  len = input[next + copy++];
                  if (state.head && len && state.length < 65536) {
                    state.head.comment += String.fromCharCode(len);
                  }
                } while (len && copy < have);
                if (state.flags & 512 && state.wrap & 4) {
                  state.check = crc32(state.check, input, copy, next);
                }
                have -= copy;
                next += copy;
                if (len) {
                  break inf_leave;
                }
              } else if (state.head) {
                state.head.comment = null;
              }
              state.mode = HCRC;
            case HCRC:
              if (state.flags & 512) {
                while (bits < 16) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                if (state.wrap & 4 && hold !== (state.check & 65535)) {
                  strm.msg = "header crc mismatch";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              if (state.head) {
                state.head.hcrc = state.flags >> 9 & 1;
                state.head.done = true;
              }
              strm.adler = state.check = 0;
              state.mode = TYPE;
              break;
            case DICTID:
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              strm.adler = state.check = zswap32(hold);
              hold = 0;
              bits = 0;
              state.mode = DICT;
            case DICT:
              if (state.havedict === 0) {
                strm.next_out = put;
                strm.avail_out = left;
                strm.next_in = next;
                strm.avail_in = have;
                state.hold = hold;
                state.bits = bits;
                return Z_NEED_DICT;
              }
              strm.adler = state.check = 1;
              state.mode = TYPE;
            case TYPE:
              if (flush === Z_BLOCK || flush === Z_TREES) {
                break inf_leave;
              }
            case TYPEDO:
              if (state.last) {
                hold >>>= bits & 7;
                bits -= bits & 7;
                state.mode = CHECK;
                break;
              }
              while (bits < 3) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.last = hold & 1;
              hold >>>= 1;
              bits -= 1;
              switch (hold & 3) {
                case 0:
                  state.mode = STORED;
                  break;
                case 1:
                  fixedtables(state);
                  state.mode = LEN_;
                  if (flush === Z_TREES) {
                    hold >>>= 2;
                    bits -= 2;
                    break inf_leave;
                  }
                  break;
                case 2:
                  state.mode = TABLE;
                  break;
                case 3:
                  strm.msg = "invalid block type";
                  state.mode = BAD;
              }
              hold >>>= 2;
              bits -= 2;
              break;
            case STORED:
              hold >>>= bits & 7;
              bits -= bits & 7;
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if ((hold & 65535) !== (hold >>> 16 ^ 65535)) {
                strm.msg = "invalid stored block lengths";
                state.mode = BAD;
                break;
              }
              state.length = hold & 65535;
              hold = 0;
              bits = 0;
              state.mode = COPY_;
              if (flush === Z_TREES) {
                break inf_leave;
              }
            case COPY_:
              state.mode = COPY;
            case COPY:
              copy = state.length;
              if (copy) {
                if (copy > have) {
                  copy = have;
                }
                if (copy > left) {
                  copy = left;
                }
                if (copy === 0) {
                  break inf_leave;
                }
                output.set(input.subarray(next, next + copy), put);
                have -= copy;
                next += copy;
                left -= copy;
                put += copy;
                state.length -= copy;
                break;
              }
              state.mode = TYPE;
              break;
            case TABLE:
              while (bits < 14) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.nlen = (hold & 31) + 257;
              hold >>>= 5;
              bits -= 5;
              state.ndist = (hold & 31) + 1;
              hold >>>= 5;
              bits -= 5;
              state.ncode = (hold & 15) + 4;
              hold >>>= 4;
              bits -= 4;
              if (state.nlen > 286 || state.ndist > 30) {
                strm.msg = "too many length or distance symbols";
                state.mode = BAD;
                break;
              }
              state.have = 0;
              state.mode = LENLENS;
            case LENLENS:
              while (state.have < state.ncode) {
                while (bits < 3) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                state.lens[order[state.have++]] = hold & 7;
                hold >>>= 3;
                bits -= 3;
              }
              while (state.have < 19) {
                state.lens[order[state.have++]] = 0;
              }
              state.lencode = state.lendyn;
              state.lenbits = 7;
              opts2 = { bits: state.lenbits };
              ret = inflate_table(CODES, state.lens, 0, 19, state.lencode, 0, state.work, opts2);
              state.lenbits = opts2.bits;
              if (ret) {
                strm.msg = "invalid code lengths set";
                state.mode = BAD;
                break;
              }
              state.have = 0;
              state.mode = CODELENS;
            case CODELENS:
              while (state.have < state.nlen + state.ndist) {
                for (; ; ) {
                  here = state.lencode[hold & (1 << state.lenbits) - 1];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                if (here_val < 16) {
                  hold >>>= here_bits;
                  bits -= here_bits;
                  state.lens[state.have++] = here_val;
                } else {
                  if (here_val === 16) {
                    n = here_bits + 2;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    if (state.have === 0) {
                      strm.msg = "invalid bit length repeat";
                      state.mode = BAD;
                      break;
                    }
                    len = state.lens[state.have - 1];
                    copy = 3 + (hold & 3);
                    hold >>>= 2;
                    bits -= 2;
                  } else if (here_val === 17) {
                    n = here_bits + 3;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    len = 0;
                    copy = 3 + (hold & 7);
                    hold >>>= 3;
                    bits -= 3;
                  } else {
                    n = here_bits + 7;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    len = 0;
                    copy = 11 + (hold & 127);
                    hold >>>= 7;
                    bits -= 7;
                  }
                  if (state.have + copy > state.nlen + state.ndist) {
                    strm.msg = "invalid bit length repeat";
                    state.mode = BAD;
                    break;
                  }
                  while (copy--) {
                    state.lens[state.have++] = len;
                  }
                }
              }
              if (state.mode === BAD) {
                break;
              }
              if (state.lens[256] === 0) {
                strm.msg = "invalid code -- missing end-of-block";
                state.mode = BAD;
                break;
              }
              state.lenbits = 9;
              opts2 = { bits: state.lenbits };
              ret = inflate_table(LENS, state.lens, 0, state.nlen, state.lencode, 0, state.work, opts2);
              state.lenbits = opts2.bits;
              if (ret) {
                strm.msg = "invalid literal/lengths set";
                state.mode = BAD;
                break;
              }
              state.distbits = 6;
              state.distcode = state.distdyn;
              opts2 = { bits: state.distbits };
              ret = inflate_table(DISTS, state.lens, state.nlen, state.ndist, state.distcode, 0, state.work, opts2);
              state.distbits = opts2.bits;
              if (ret) {
                strm.msg = "invalid distances set";
                state.mode = BAD;
                break;
              }
              state.mode = LEN_;
              if (flush === Z_TREES) {
                break inf_leave;
              }
            case LEN_:
              state.mode = LEN;
            case LEN:
              if (have >= 6 && left >= 258) {
                strm.next_out = put;
                strm.avail_out = left;
                strm.next_in = next;
                strm.avail_in = have;
                state.hold = hold;
                state.bits = bits;
                inflate_fast(strm, _out);
                put = strm.next_out;
                output = strm.output;
                left = strm.avail_out;
                next = strm.next_in;
                input = strm.input;
                have = strm.avail_in;
                hold = state.hold;
                bits = state.bits;
                if (state.mode === TYPE) {
                  state.back = -1;
                }
                break;
              }
              state.back = 0;
              for (; ; ) {
                here = state.lencode[hold & (1 << state.lenbits) - 1];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (here_op && (here_op & 240) === 0) {
                last_bits = here_bits;
                last_op = here_op;
                last_val = here_val;
                for (; ; ) {
                  here = state.lencode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (last_bits + here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                hold >>>= last_bits;
                bits -= last_bits;
                state.back += last_bits;
              }
              hold >>>= here_bits;
              bits -= here_bits;
              state.back += here_bits;
              state.length = here_val;
              if (here_op === 0) {
                state.mode = LIT;
                break;
              }
              if (here_op & 32) {
                state.back = -1;
                state.mode = TYPE;
                break;
              }
              if (here_op & 64) {
                strm.msg = "invalid literal/length code";
                state.mode = BAD;
                break;
              }
              state.extra = here_op & 15;
              state.mode = LENEXT;
            case LENEXT:
              if (state.extra) {
                n = state.extra;
                while (bits < n) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                state.length += hold & (1 << state.extra) - 1;
                hold >>>= state.extra;
                bits -= state.extra;
                state.back += state.extra;
              }
              state.was = state.length;
              state.mode = DIST;
            case DIST:
              for (; ; ) {
                here = state.distcode[hold & (1 << state.distbits) - 1];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if ((here_op & 240) === 0) {
                last_bits = here_bits;
                last_op = here_op;
                last_val = here_val;
                for (; ; ) {
                  here = state.distcode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (last_bits + here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                hold >>>= last_bits;
                bits -= last_bits;
                state.back += last_bits;
              }
              hold >>>= here_bits;
              bits -= here_bits;
              state.back += here_bits;
              if (here_op & 64) {
                strm.msg = "invalid distance code";
                state.mode = BAD;
                break;
              }
              state.offset = here_val;
              state.extra = here_op & 15;
              state.mode = DISTEXT;
            case DISTEXT:
              if (state.extra) {
                n = state.extra;
                while (bits < n) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                state.offset += hold & (1 << state.extra) - 1;
                hold >>>= state.extra;
                bits -= state.extra;
                state.back += state.extra;
              }
              if (state.offset > state.dmax) {
                strm.msg = "invalid distance too far back";
                state.mode = BAD;
                break;
              }
              state.mode = MATCH;
            case MATCH:
              if (left === 0) {
                break inf_leave;
              }
              copy = _out - left;
              if (state.offset > copy) {
                copy = state.offset - copy;
                if (copy > state.whave) {
                  if (state.sane) {
                    strm.msg = "invalid distance too far back";
                    state.mode = BAD;
                    break;
                  }
                }
                if (copy > state.wnext) {
                  copy -= state.wnext;
                  from = state.wsize - copy;
                } else {
                  from = state.wnext - copy;
                }
                if (copy > state.length) {
                  copy = state.length;
                }
                from_source = state.window;
              } else {
                from_source = output;
                from = put - state.offset;
                copy = state.length;
              }
              if (copy > left) {
                copy = left;
              }
              left -= copy;
              state.length -= copy;
              do {
                output[put++] = from_source[from++];
              } while (--copy);
              if (state.length === 0) {
                state.mode = LEN;
              }
              break;
            case LIT:
              if (left === 0) {
                break inf_leave;
              }
              output[put++] = state.length;
              left--;
              state.mode = LEN;
              break;
            case CHECK:
              if (state.wrap) {
                while (bits < 32) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold |= input[next++] << bits;
                  bits += 8;
                }
                _out -= left;
                strm.total_out += _out;
                state.total += _out;
                if (state.wrap & 4 && _out) {
                  strm.adler = state.check = /*UPDATE_CHECK(state.check, put - _out, _out);*/
                  state.flags ? crc32(state.check, output, _out, put - _out) : adler32(state.check, output, _out, put - _out);
                }
                _out = left;
                if (state.wrap & 4 && (state.flags ? hold : zswap32(hold)) !== state.check) {
                  strm.msg = "incorrect data check";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              state.mode = LENGTH;
            case LENGTH:
              if (state.wrap && state.flags) {
                while (bits < 32) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input[next++] << bits;
                  bits += 8;
                }
                if (state.wrap & 4 && hold !== (state.total & 4294967295)) {
                  strm.msg = "incorrect length check";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              state.mode = DONE;
            case DONE:
              ret = Z_STREAM_END;
              break inf_leave;
            case BAD:
              ret = Z_DATA_ERROR;
              break inf_leave;
            case MEM:
              return Z_MEM_ERROR;
            case SYNC:
            default:
              return Z_STREAM_ERROR;
          }
        }
      strm.next_out = put;
      strm.avail_out = left;
      strm.next_in = next;
      strm.avail_in = have;
      state.hold = hold;
      state.bits = bits;
      if (state.wsize || _out !== strm.avail_out && state.mode < BAD && (state.mode < CHECK || flush !== Z_FINISH)) {
        if (updatewindow(strm, strm.output, strm.next_out, _out - strm.avail_out)) {
          state.mode = MEM;
          return Z_MEM_ERROR;
        }
      }
      _in -= strm.avail_in;
      _out -= strm.avail_out;
      strm.total_in += _in;
      strm.total_out += _out;
      state.total += _out;
      if (state.wrap & 4 && _out) {
        strm.adler = state.check = /*UPDATE_CHECK(state.check, strm.next_out - _out, _out);*/
        state.flags ? crc32(state.check, output, _out, strm.next_out - _out) : adler32(state.check, output, _out, strm.next_out - _out);
      }
      strm.data_type = state.bits + (state.last ? 64 : 0) + (state.mode === TYPE ? 128 : 0) + (state.mode === LEN_ || state.mode === COPY_ ? 256 : 0);
      if ((_in === 0 && _out === 0 || flush === Z_FINISH) && ret === Z_OK) {
        ret = Z_BUF_ERROR;
      }
      return ret;
    };
    var inflateEnd = (strm) => {
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      let state = strm.state;
      if (state.window) {
        state.window = null;
      }
      strm.state = null;
      return Z_OK;
    };
    var inflateGetHeader = (strm, head) => {
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      const state = strm.state;
      if ((state.wrap & 2) === 0) {
        return Z_STREAM_ERROR;
      }
      state.head = head;
      head.done = false;
      return Z_OK;
    };
    var inflateSetDictionary = (strm, dictionary) => {
      const dictLength = dictionary.length;
      let state;
      let dictid;
      let ret;
      if (inflateStateCheck(strm)) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if (state.wrap !== 0 && state.mode !== DICT) {
        return Z_STREAM_ERROR;
      }
      if (state.mode === DICT) {
        dictid = 1;
        dictid = adler32(dictid, dictionary, dictLength, 0);
        if (dictid !== state.check) {
          return Z_DATA_ERROR;
        }
      }
      ret = updatewindow(strm, dictionary, dictLength, dictLength);
      if (ret) {
        state.mode = MEM;
        return Z_MEM_ERROR;
      }
      state.havedict = 1;
      return Z_OK;
    };
    module2.exports.inflateReset = inflateReset;
    module2.exports.inflateReset2 = inflateReset2;
    module2.exports.inflateResetKeep = inflateResetKeep;
    module2.exports.inflateInit = inflateInit;
    module2.exports.inflateInit2 = inflateInit2;
    module2.exports.inflate = inflate;
    module2.exports.inflateEnd = inflateEnd;
    module2.exports.inflateGetHeader = inflateGetHeader;
    module2.exports.inflateSetDictionary = inflateSetDictionary;
    module2.exports.inflateInfo = "pako inflate (from Nodeca project)";
  }
});

// node_modules/pako/lib/zlib/gzheader.js
var require_gzheader = __commonJS({
  "node_modules/pako/lib/zlib/gzheader.js"(exports2, module2) {
    "use strict";
    function GZheader() {
      this.text = 0;
      this.time = 0;
      this.xflags = 0;
      this.os = 0;
      this.extra = null;
      this.extra_len = 0;
      this.name = "";
      this.comment = "";
      this.hcrc = 0;
      this.done = false;
    }
    module2.exports = GZheader;
  }
});

// node_modules/pako/lib/inflate.js
var require_inflate2 = __commonJS({
  "node_modules/pako/lib/inflate.js"(exports2, module2) {
    "use strict";
    var zlib_inflate = require_inflate();
    var utils = require_common();
    var strings = require_strings();
    var msg = require_messages();
    var ZStream = require_zstream();
    var GZheader = require_gzheader();
    var toString = Object.prototype.toString;
    var {
      Z_NO_FLUSH,
      Z_FINISH,
      Z_OK,
      Z_STREAM_END,
      Z_NEED_DICT,
      Z_STREAM_ERROR,
      Z_DATA_ERROR,
      Z_MEM_ERROR
    } = require_constants();
    function Inflate(options) {
      this.options = utils.assign({
        chunkSize: 1024 * 64,
        windowBits: 15,
        to: ""
      }, options || {});
      const opt = this.options;
      if (opt.raw && opt.windowBits >= 0 && opt.windowBits < 16) {
        opt.windowBits = -opt.windowBits;
        if (opt.windowBits === 0) {
          opt.windowBits = -15;
        }
      }
      if (opt.windowBits >= 0 && opt.windowBits < 16 && !(options && options.windowBits)) {
        opt.windowBits += 32;
      }
      if (opt.windowBits > 15 && opt.windowBits < 48) {
        if ((opt.windowBits & 15) === 0) {
          opt.windowBits |= 15;
        }
      }
      this.err = 0;
      this.msg = "";
      this.ended = false;
      this.chunks = [];
      this.strm = new ZStream();
      this.strm.avail_out = 0;
      let status = zlib_inflate.inflateInit2(
        this.strm,
        opt.windowBits
      );
      if (status !== Z_OK) {
        throw new Error(msg[status]);
      }
      this.header = new GZheader();
      zlib_inflate.inflateGetHeader(this.strm, this.header);
      if (opt.dictionary) {
        if (typeof opt.dictionary === "string") {
          opt.dictionary = strings.string2buf(opt.dictionary);
        } else if (toString.call(opt.dictionary) === "[object ArrayBuffer]") {
          opt.dictionary = new Uint8Array(opt.dictionary);
        }
        if (opt.raw) {
          status = zlib_inflate.inflateSetDictionary(this.strm, opt.dictionary);
          if (status !== Z_OK) {
            throw new Error(msg[status]);
          }
        }
      }
    }
    Inflate.prototype.push = function(data, flush_mode) {
      const strm = this.strm;
      const chunkSize = this.options.chunkSize;
      const dictionary = this.options.dictionary;
      let status, _flush_mode, last_avail_out;
      if (this.ended)
        return false;
      if (flush_mode === ~~flush_mode)
        _flush_mode = flush_mode;
      else
        _flush_mode = flush_mode === true ? Z_FINISH : Z_NO_FLUSH;
      if (toString.call(data) === "[object ArrayBuffer]") {
        strm.input = new Uint8Array(data);
      } else {
        strm.input = data;
      }
      strm.next_in = 0;
      strm.avail_in = strm.input.length;
      for (; ; ) {
        if (strm.avail_out === 0) {
          strm.output = new Uint8Array(chunkSize);
          strm.next_out = 0;
          strm.avail_out = chunkSize;
        }
        status = zlib_inflate.inflate(strm, _flush_mode);
        if (status === Z_NEED_DICT && dictionary) {
          status = zlib_inflate.inflateSetDictionary(strm, dictionary);
          if (status === Z_OK) {
            status = zlib_inflate.inflate(strm, _flush_mode);
          } else if (status === Z_DATA_ERROR) {
            status = Z_NEED_DICT;
          }
        }
        while (strm.avail_in > 0 && status === Z_STREAM_END && strm.state.wrap > 0 && data[strm.next_in] !== 0) {
          zlib_inflate.inflateReset(strm);
          status = zlib_inflate.inflate(strm, _flush_mode);
        }
        switch (status) {
          case Z_STREAM_ERROR:
          case Z_DATA_ERROR:
          case Z_NEED_DICT:
          case Z_MEM_ERROR:
            this.onEnd(status);
            this.ended = true;
            return false;
        }
        last_avail_out = strm.avail_out;
        if (strm.next_out) {
          if (strm.avail_out === 0 || status === Z_STREAM_END) {
            if (this.options.to === "string") {
              let next_out_utf8 = strings.utf8border(strm.output, strm.next_out);
              let tail = strm.next_out - next_out_utf8;
              let utf8str = strings.buf2string(strm.output, next_out_utf8);
              strm.next_out = tail;
              strm.avail_out = chunkSize - tail;
              if (tail)
                strm.output.set(strm.output.subarray(next_out_utf8, next_out_utf8 + tail), 0);
              this.onData(utf8str);
            } else {
              this.onData(strm.output.length === strm.next_out ? strm.output : strm.output.subarray(0, strm.next_out));
            }
          }
        }
        if (status === Z_OK && last_avail_out === 0)
          continue;
        if (status === Z_STREAM_END) {
          status = zlib_inflate.inflateEnd(this.strm);
          this.onEnd(status);
          this.ended = true;
          return true;
        }
        if (strm.avail_in === 0)
          break;
      }
      return true;
    };
    Inflate.prototype.onData = function(chunk) {
      this.chunks.push(chunk);
    };
    Inflate.prototype.onEnd = function(status) {
      if (status === Z_OK) {
        if (this.options.to === "string") {
          this.result = this.chunks.join("");
        } else {
          this.result = utils.flattenChunks(this.chunks);
        }
      }
      this.chunks = [];
      this.err = status;
      this.msg = this.strm.msg;
    };
    function inflate(input, options) {
      const inflator = new Inflate(options);
      inflator.push(input);
      if (inflator.err)
        throw inflator.msg || msg[inflator.err];
      return inflator.result;
    }
    function inflateRaw(input, options) {
      options = options || {};
      options.raw = true;
      return inflate(input, options);
    }
    module2.exports.Inflate = Inflate;
    module2.exports.inflate = inflate;
    module2.exports.inflateRaw = inflateRaw;
    module2.exports.ungzip = inflate;
    module2.exports.constants = require_constants();
  }
});

// node_modules/pako/index.js
var require_pako = __commonJS({
  "node_modules/pako/index.js"(exports2, module2) {
    "use strict";
    var { Deflate, deflate, deflateRaw, gzip } = require_deflate2();
    var { Inflate, inflate, inflateRaw, ungzip } = require_inflate2();
    var constants = require_constants();
    module2.exports.Deflate = Deflate;
    module2.exports.deflate = deflate;
    module2.exports.deflateRaw = deflateRaw;
    module2.exports.gzip = gzip;
    module2.exports.Inflate = Inflate;
    module2.exports.inflate = inflate;
    module2.exports.inflateRaw = inflateRaw;
    module2.exports.ungzip = ungzip;
    module2.exports.constants = constants;
  }
});

// node_modules/proxy-from-env/index.js
var require_proxy_from_env = __commonJS({
  "node_modules/proxy-from-env/index.js"(exports2) {
    "use strict";
    var parseUrl = require("url").parse;
    var DEFAULT_PORTS = {
      ftp: 21,
      gopher: 70,
      http: 80,
      https: 443,
      ws: 80,
      wss: 443
    };
    var stringEndsWith = String.prototype.endsWith || function(s) {
      return s.length <= this.length && this.indexOf(s, this.length - s.length) !== -1;
    };
    function getProxyForUrl(url) {
      var parsedUrl = typeof url === "string" ? parseUrl(url) : url || {};
      var proto = parsedUrl.protocol;
      var hostname = parsedUrl.host;
      var port = parsedUrl.port;
      if (typeof hostname !== "string" || !hostname || typeof proto !== "string") {
        return "";
      }
      proto = proto.split(":", 1)[0];
      hostname = hostname.replace(/:\d*$/, "");
      port = parseInt(port) || DEFAULT_PORTS[proto] || 0;
      if (!shouldProxy(hostname, port)) {
        return "";
      }
      var proxy = getEnv("npm_config_" + proto + "_proxy") || getEnv(proto + "_proxy") || getEnv("npm_config_proxy") || getEnv("all_proxy");
      if (proxy && proxy.indexOf("://") === -1) {
        proxy = proto + "://" + proxy;
      }
      return proxy;
    }
    function shouldProxy(hostname, port) {
      var NO_PROXY = (getEnv("npm_config_no_proxy") || getEnv("no_proxy")).toLowerCase();
      if (!NO_PROXY) {
        return true;
      }
      if (NO_PROXY === "*") {
        return false;
      }
      return NO_PROXY.split(/[,\s]/).every(function(proxy) {
        if (!proxy) {
          return true;
        }
        var parsedProxy = proxy.match(/^(.+):(\d+)$/);
        var parsedProxyHostname = parsedProxy ? parsedProxy[1] : proxy;
        var parsedProxyPort = parsedProxy ? parseInt(parsedProxy[2]) : 0;
        if (parsedProxyPort && parsedProxyPort !== port) {
          return true;
        }
        if (!/^[.*]/.test(parsedProxyHostname)) {
          return hostname !== parsedProxyHostname;
        }
        if (parsedProxyHostname.charAt(0) === "*") {
          parsedProxyHostname = parsedProxyHostname.slice(1);
        }
        return !stringEndsWith.call(hostname, parsedProxyHostname);
      });
    }
    function getEnv(key) {
      return process.env[key.toLowerCase()] || process.env[key.toUpperCase()] || "";
    }
    exports2.getProxyForUrl = getProxyForUrl;
  }
});

// node_modules/ms/index.js
var require_ms = __commonJS({
  "node_modules/ms/index.js"(exports2, module2) {
    var s = 1e3;
    var m = s * 60;
    var h = m * 60;
    var d = h * 24;
    var w = d * 7;
    var y = d * 365.25;
    module2.exports = function(val, options) {
      options = options || {};
      var type = typeof val;
      if (type === "string" && val.length > 0) {
        return parse(val);
      } else if (type === "number" && isFinite(val)) {
        return options.long ? fmtLong(val) : fmtShort(val);
      }
      throw new Error(
        "val is not a non-empty string or a valid number. val=" + JSON.stringify(val)
      );
    };
    function parse(str2) {
      str2 = String(str2);
      if (str2.length > 100) {
        return;
      }
      var match = /^(-?(?:\d+)?\.?\d+) *(milliseconds?|msecs?|ms|seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d|weeks?|w|years?|yrs?|y)?$/i.exec(
        str2
      );
      if (!match) {
        return;
      }
      var n = parseFloat(match[1]);
      var type = (match[2] || "ms").toLowerCase();
      switch (type) {
        case "years":
        case "year":
        case "yrs":
        case "yr":
        case "y":
          return n * y;
        case "weeks":
        case "week":
        case "w":
          return n * w;
        case "days":
        case "day":
        case "d":
          return n * d;
        case "hours":
        case "hour":
        case "hrs":
        case "hr":
        case "h":
          return n * h;
        case "minutes":
        case "minute":
        case "mins":
        case "min":
        case "m":
          return n * m;
        case "seconds":
        case "second":
        case "secs":
        case "sec":
        case "s":
          return n * s;
        case "milliseconds":
        case "millisecond":
        case "msecs":
        case "msec":
        case "ms":
          return n;
        default:
          return void 0;
      }
    }
    function fmtShort(ms) {
      var msAbs = Math.abs(ms);
      if (msAbs >= d) {
        return Math.round(ms / d) + "d";
      }
      if (msAbs >= h) {
        return Math.round(ms / h) + "h";
      }
      if (msAbs >= m) {
        return Math.round(ms / m) + "m";
      }
      if (msAbs >= s) {
        return Math.round(ms / s) + "s";
      }
      return ms + "ms";
    }
    function fmtLong(ms) {
      var msAbs = Math.abs(ms);
      if (msAbs >= d) {
        return plural(ms, msAbs, d, "day");
      }
      if (msAbs >= h) {
        return plural(ms, msAbs, h, "hour");
      }
      if (msAbs >= m) {
        return plural(ms, msAbs, m, "minute");
      }
      if (msAbs >= s) {
        return plural(ms, msAbs, s, "second");
      }
      return ms + " ms";
    }
    function plural(ms, msAbs, n, name) {
      var isPlural = msAbs >= n * 1.5;
      return Math.round(ms / n) + " " + name + (isPlural ? "s" : "");
    }
  }
});

// node_modules/debug/src/common.js
var require_common2 = __commonJS({
  "node_modules/debug/src/common.js"(exports2, module2) {
    function setup(env) {
      createDebug.debug = createDebug;
      createDebug.default = createDebug;
      createDebug.coerce = coerce;
      createDebug.disable = disable;
      createDebug.enable = enable;
      createDebug.enabled = enabled;
      createDebug.humanize = require_ms();
      createDebug.destroy = destroy;
      Object.keys(env).forEach((key) => {
        createDebug[key] = env[key];
      });
      createDebug.names = [];
      createDebug.skips = [];
      createDebug.formatters = {};
      function selectColor(namespace) {
        let hash = 0;
        for (let i = 0; i < namespace.length; i++) {
          hash = (hash << 5) - hash + namespace.charCodeAt(i);
          hash |= 0;
        }
        return createDebug.colors[Math.abs(hash) % createDebug.colors.length];
      }
      createDebug.selectColor = selectColor;
      function createDebug(namespace) {
        let prevTime;
        let enableOverride = null;
        let namespacesCache;
        let enabledCache;
        function debug(...args2) {
          if (!debug.enabled) {
            return;
          }
          const self2 = debug;
          const curr = Number(/* @__PURE__ */ new Date());
          const ms = curr - (prevTime || curr);
          self2.diff = ms;
          self2.prev = prevTime;
          self2.curr = curr;
          prevTime = curr;
          args2[0] = createDebug.coerce(args2[0]);
          if (typeof args2[0] !== "string") {
            args2.unshift("%O");
          }
          let index = 0;
          args2[0] = args2[0].replace(/%([a-zA-Z%])/g, (match, format) => {
            if (match === "%%") {
              return "%";
            }
            index++;
            const formatter = createDebug.formatters[format];
            if (typeof formatter === "function") {
              const val = args2[index];
              match = formatter.call(self2, val);
              args2.splice(index, 1);
              index--;
            }
            return match;
          });
          createDebug.formatArgs.call(self2, args2);
          const logFn = self2.log || createDebug.log;
          logFn.apply(self2, args2);
        }
        debug.namespace = namespace;
        debug.useColors = createDebug.useColors();
        debug.color = createDebug.selectColor(namespace);
        debug.extend = extend;
        debug.destroy = createDebug.destroy;
        Object.defineProperty(debug, "enabled", {
          enumerable: true,
          configurable: false,
          get: () => {
            if (enableOverride !== null) {
              return enableOverride;
            }
            if (namespacesCache !== createDebug.namespaces) {
              namespacesCache = createDebug.namespaces;
              enabledCache = createDebug.enabled(namespace);
            }
            return enabledCache;
          },
          set: (v) => {
            enableOverride = v;
          }
        });
        if (typeof createDebug.init === "function") {
          createDebug.init(debug);
        }
        return debug;
      }
      function extend(namespace, delimiter) {
        const newDebug = createDebug(this.namespace + (typeof delimiter === "undefined" ? ":" : delimiter) + namespace);
        newDebug.log = this.log;
        return newDebug;
      }
      function enable(namespaces) {
        createDebug.save(namespaces);
        createDebug.namespaces = namespaces;
        createDebug.names = [];
        createDebug.skips = [];
        const split = (typeof namespaces === "string" ? namespaces : "").trim().replace(/\s+/g, ",").split(",").filter(Boolean);
        for (const ns of split) {
          if (ns[0] === "-") {
            createDebug.skips.push(ns.slice(1));
          } else {
            createDebug.names.push(ns);
          }
        }
      }
      function matchesTemplate(search, template) {
        let searchIndex = 0;
        let templateIndex = 0;
        let starIndex = -1;
        let matchIndex = 0;
        while (searchIndex < search.length) {
          if (templateIndex < template.length && (template[templateIndex] === search[searchIndex] || template[templateIndex] === "*")) {
            if (template[templateIndex] === "*") {
              starIndex = templateIndex;
              matchIndex = searchIndex;
              templateIndex++;
            } else {
              searchIndex++;
              templateIndex++;
            }
          } else if (starIndex !== -1) {
            templateIndex = starIndex + 1;
            matchIndex++;
            searchIndex = matchIndex;
          } else {
            return false;
          }
        }
        while (templateIndex < template.length && template[templateIndex] === "*") {
          templateIndex++;
        }
        return templateIndex === template.length;
      }
      function disable() {
        const namespaces = [
          ...createDebug.names,
          ...createDebug.skips.map((namespace) => "-" + namespace)
        ].join(",");
        createDebug.enable("");
        return namespaces;
      }
      function enabled(name) {
        for (const skip of createDebug.skips) {
          if (matchesTemplate(name, skip)) {
            return false;
          }
        }
        for (const ns of createDebug.names) {
          if (matchesTemplate(name, ns)) {
            return true;
          }
        }
        return false;
      }
      function coerce(val) {
        if (val instanceof Error) {
          return val.stack || val.message;
        }
        return val;
      }
      function destroy() {
        console.warn("Instance method `debug.destroy()` is deprecated and no longer does anything. It will be removed in the next major version of `debug`.");
      }
      createDebug.enable(createDebug.load());
      return createDebug;
    }
    module2.exports = setup;
  }
});

// node_modules/debug/src/browser.js
var require_browser = __commonJS({
  "node_modules/debug/src/browser.js"(exports2, module2) {
    exports2.formatArgs = formatArgs;
    exports2.save = save;
    exports2.load = load;
    exports2.useColors = useColors;
    exports2.storage = localstorage();
    exports2.destroy = /* @__PURE__ */ (() => {
      let warned = false;
      return () => {
        if (!warned) {
          warned = true;
          console.warn("Instance method `debug.destroy()` is deprecated and no longer does anything. It will be removed in the next major version of `debug`.");
        }
      };
    })();
    exports2.colors = [
      "#0000CC",
      "#0000FF",
      "#0033CC",
      "#0033FF",
      "#0066CC",
      "#0066FF",
      "#0099CC",
      "#0099FF",
      "#00CC00",
      "#00CC33",
      "#00CC66",
      "#00CC99",
      "#00CCCC",
      "#00CCFF",
      "#3300CC",
      "#3300FF",
      "#3333CC",
      "#3333FF",
      "#3366CC",
      "#3366FF",
      "#3399CC",
      "#3399FF",
      "#33CC00",
      "#33CC33",
      "#33CC66",
      "#33CC99",
      "#33CCCC",
      "#33CCFF",
      "#6600CC",
      "#6600FF",
      "#6633CC",
      "#6633FF",
      "#66CC00",
      "#66CC33",
      "#9900CC",
      "#9900FF",
      "#9933CC",
      "#9933FF",
      "#99CC00",
      "#99CC33",
      "#CC0000",
      "#CC0033",
      "#CC0066",
      "#CC0099",
      "#CC00CC",
      "#CC00FF",
      "#CC3300",
      "#CC3333",
      "#CC3366",
      "#CC3399",
      "#CC33CC",
      "#CC33FF",
      "#CC6600",
      "#CC6633",
      "#CC9900",
      "#CC9933",
      "#CCCC00",
      "#CCCC33",
      "#FF0000",
      "#FF0033",
      "#FF0066",
      "#FF0099",
      "#FF00CC",
      "#FF00FF",
      "#FF3300",
      "#FF3333",
      "#FF3366",
      "#FF3399",
      "#FF33CC",
      "#FF33FF",
      "#FF6600",
      "#FF6633",
      "#FF9900",
      "#FF9933",
      "#FFCC00",
      "#FFCC33"
    ];
    function useColors() {
      if (typeof window !== "undefined" && window.process && (window.process.type === "renderer" || window.process.__nwjs)) {
        return true;
      }
      if (typeof navigator !== "undefined" && navigator.userAgent && navigator.userAgent.toLowerCase().match(/(edge|trident)\/(\d+)/)) {
        return false;
      }
      let m;
      return typeof document !== "undefined" && document.documentElement && document.documentElement.style && document.documentElement.style.WebkitAppearance || // Is firebug? http://stackoverflow.com/a/398120/376773
      typeof window !== "undefined" && window.console && (window.console.firebug || window.console.exception && window.console.table) || // Is firefox >= v31?
      // https://developer.mozilla.org/en-US/docs/Tools/Web_Console#Styling_messages
      typeof navigator !== "undefined" && navigator.userAgent && (m = navigator.userAgent.toLowerCase().match(/firefox\/(\d+)/)) && parseInt(m[1], 10) >= 31 || // Double check webkit in userAgent just in case we are in a worker
      typeof navigator !== "undefined" && navigator.userAgent && navigator.userAgent.toLowerCase().match(/applewebkit\/(\d+)/);
    }
    function formatArgs(args2) {
      args2[0] = (this.useColors ? "%c" : "") + this.namespace + (this.useColors ? " %c" : " ") + args2[0] + (this.useColors ? "%c " : " ") + "+" + module2.exports.humanize(this.diff);
      if (!this.useColors) {
        return;
      }
      const c = "color: " + this.color;
      args2.splice(1, 0, c, "color: inherit");
      let index = 0;
      let lastC = 0;
      args2[0].replace(/%[a-zA-Z%]/g, (match) => {
        if (match === "%%") {
          return;
        }
        index++;
        if (match === "%c") {
          lastC = index;
        }
      });
      args2.splice(lastC, 0, c);
    }
    exports2.log = console.debug || console.log || (() => {
    });
    function save(namespaces) {
      try {
        if (namespaces) {
          exports2.storage.setItem("debug", namespaces);
        } else {
          exports2.storage.removeItem("debug");
        }
      } catch (error) {
      }
    }
    function load() {
      let r;
      try {
        r = exports2.storage.getItem("debug") || exports2.storage.getItem("DEBUG");
      } catch (error) {
      }
      if (!r && typeof process !== "undefined" && "env" in process) {
        r = process.env.DEBUG;
      }
      return r;
    }
    function localstorage() {
      try {
        return localStorage;
      } catch (error) {
      }
    }
    module2.exports = require_common2()(exports2);
    var { formatters } = module2.exports;
    formatters.j = function(v) {
      try {
        return JSON.stringify(v);
      } catch (error) {
        return "[UnexpectedJSONParseError]: " + error.message;
      }
    };
  }
});

// node_modules/has-flag/index.js
var require_has_flag = __commonJS({
  "node_modules/has-flag/index.js"(exports2, module2) {
    "use strict";
    module2.exports = (flag, argv = process.argv) => {
      const prefix = flag.startsWith("-") ? "" : flag.length === 1 ? "-" : "--";
      const position = argv.indexOf(prefix + flag);
      const terminatorPosition = argv.indexOf("--");
      return position !== -1 && (terminatorPosition === -1 || position < terminatorPosition);
    };
  }
});

// node_modules/supports-color/index.js
var require_supports_color = __commonJS({
  "node_modules/supports-color/index.js"(exports2, module2) {
    "use strict";
    var os = require("os");
    var tty = require("tty");
    var hasFlag = require_has_flag();
    var { env } = process;
    var forceColor;
    if (hasFlag("no-color") || hasFlag("no-colors") || hasFlag("color=false") || hasFlag("color=never")) {
      forceColor = 0;
    } else if (hasFlag("color") || hasFlag("colors") || hasFlag("color=true") || hasFlag("color=always")) {
      forceColor = 1;
    }
    if ("FORCE_COLOR" in env) {
      if (env.FORCE_COLOR === "true") {
        forceColor = 1;
      } else if (env.FORCE_COLOR === "false") {
        forceColor = 0;
      } else {
        forceColor = env.FORCE_COLOR.length === 0 ? 1 : Math.min(parseInt(env.FORCE_COLOR, 10), 3);
      }
    }
    function translateLevel(level) {
      if (level === 0) {
        return false;
      }
      return {
        level,
        hasBasic: true,
        has256: level >= 2,
        has16m: level >= 3
      };
    }
    function supportsColor(haveStream, streamIsTTY) {
      if (forceColor === 0) {
        return 0;
      }
      if (hasFlag("color=16m") || hasFlag("color=full") || hasFlag("color=truecolor")) {
        return 3;
      }
      if (hasFlag("color=256")) {
        return 2;
      }
      if (haveStream && !streamIsTTY && forceColor === void 0) {
        return 0;
      }
      const min = forceColor || 0;
      if (env.TERM === "dumb") {
        return min;
      }
      if (process.platform === "win32") {
        const osRelease = os.release().split(".");
        if (Number(osRelease[0]) >= 10 && Number(osRelease[2]) >= 10586) {
          return Number(osRelease[2]) >= 14931 ? 3 : 2;
        }
        return 1;
      }
      if ("CI" in env) {
        if (["TRAVIS", "CIRCLECI", "APPVEYOR", "GITLAB_CI", "GITHUB_ACTIONS", "BUILDKITE"].some((sign) => sign in env) || env.CI_NAME === "codeship") {
          return 1;
        }
        return min;
      }
      if ("TEAMCITY_VERSION" in env) {
        return /^(9\.(0*[1-9]\d*)\.|\d{2,}\.)/.test(env.TEAMCITY_VERSION) ? 1 : 0;
      }
      if (env.COLORTERM === "truecolor") {
        return 3;
      }
      if ("TERM_PROGRAM" in env) {
        const version = parseInt((env.TERM_PROGRAM_VERSION || "").split(".")[0], 10);
        switch (env.TERM_PROGRAM) {
          case "iTerm.app":
            return version >= 3 ? 3 : 2;
          case "Apple_Terminal":
            return 2;
        }
      }
      if (/-256(color)?$/i.test(env.TERM)) {
        return 2;
      }
      if (/^screen|^xterm|^vt100|^vt220|^rxvt|color|ansi|cygwin|linux/i.test(env.TERM)) {
        return 1;
      }
      if ("COLORTERM" in env) {
        return 1;
      }
      return min;
    }
    function getSupportLevel(stream) {
      const level = supportsColor(stream, stream && stream.isTTY);
      return translateLevel(level);
    }
    module2.exports = {
      supportsColor: getSupportLevel,
      stdout: translateLevel(supportsColor(true, tty.isatty(1))),
      stderr: translateLevel(supportsColor(true, tty.isatty(2)))
    };
  }
});

// node_modules/debug/src/node.js
var require_node = __commonJS({
  "node_modules/debug/src/node.js"(exports2, module2) {
    var tty = require("tty");
    var util = require("util");
    exports2.init = init;
    exports2.log = log;
    exports2.formatArgs = formatArgs;
    exports2.save = save;
    exports2.load = load;
    exports2.useColors = useColors;
    exports2.destroy = util.deprecate(
      () => {
      },
      "Instance method `debug.destroy()` is deprecated and no longer does anything. It will be removed in the next major version of `debug`."
    );
    exports2.colors = [6, 2, 3, 4, 5, 1];
    try {
      const supportsColor = require_supports_color();
      if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) {
        exports2.colors = [
          20,
          21,
          26,
          27,
          32,
          33,
          38,
          39,
          40,
          41,
          42,
          43,
          44,
          45,
          56,
          57,
          62,
          63,
          68,
          69,
          74,
          75,
          76,
          77,
          78,
          79,
          80,
          81,
          92,
          93,
          98,
          99,
          112,
          113,
          128,
          129,
          134,
          135,
          148,
          149,
          160,
          161,
          162,
          163,
          164,
          165,
          166,
          167,
          168,
          169,
          170,
          171,
          172,
          173,
          178,
          179,
          184,
          185,
          196,
          197,
          198,
          199,
          200,
          201,
          202,
          203,
          204,
          205,
          206,
          207,
          208,
          209,
          214,
          215,
          220,
          221
        ];
      }
    } catch (error) {
    }
    exports2.inspectOpts = Object.keys(process.env).filter((key) => {
      return /^debug_/i.test(key);
    }).reduce((obj, key) => {
      const prop = key.substring(6).toLowerCase().replace(/_([a-z])/g, (_, k) => {
        return k.toUpperCase();
      });
      let val = process.env[key];
      if (/^(yes|on|true|enabled)$/i.test(val)) {
        val = true;
      } else if (/^(no|off|false|disabled)$/i.test(val)) {
        val = false;
      } else if (val === "null") {
        val = null;
      } else {
        val = Number(val);
      }
      obj[prop] = val;
      return obj;
    }, {});
    function useColors() {
      return "colors" in exports2.inspectOpts ? Boolean(exports2.inspectOpts.colors) : tty.isatty(process.stderr.fd);
    }
    function formatArgs(args2) {
      const { namespace: name, useColors: useColors2 } = this;
      if (useColors2) {
        const c = this.color;
        const colorCode = "\x1B[3" + (c < 8 ? c : "8;5;" + c);
        const prefix = `  ${colorCode};1m${name} \x1B[0m`;
        args2[0] = prefix + args2[0].split("\n").join("\n" + prefix);
        args2.push(colorCode + "m+" + module2.exports.humanize(this.diff) + "\x1B[0m");
      } else {
        args2[0] = getDate() + name + " " + args2[0];
      }
    }
    function getDate() {
      if (exports2.inspectOpts.hideDate) {
        return "";
      }
      return (/* @__PURE__ */ new Date()).toISOString() + " ";
    }
    function log(...args2) {
      return process.stderr.write(util.formatWithOptions(exports2.inspectOpts, ...args2) + "\n");
    }
    function save(namespaces) {
      if (namespaces) {
        process.env.DEBUG = namespaces;
      } else {
        delete process.env.DEBUG;
      }
    }
    function load() {
      return process.env.DEBUG;
    }
    function init(debug) {
      debug.inspectOpts = {};
      const keys = Object.keys(exports2.inspectOpts);
      for (let i = 0; i < keys.length; i++) {
        debug.inspectOpts[keys[i]] = exports2.inspectOpts[keys[i]];
      }
    }
    module2.exports = require_common2()(exports2);
    var { formatters } = module2.exports;
    formatters.o = function(v) {
      this.inspectOpts.colors = this.useColors;
      return util.inspect(v, this.inspectOpts).split("\n").map((str2) => str2.trim()).join(" ");
    };
    formatters.O = function(v) {
      this.inspectOpts.colors = this.useColors;
      return util.inspect(v, this.inspectOpts);
    };
  }
});

// node_modules/debug/src/index.js
var require_src = __commonJS({
  "node_modules/debug/src/index.js"(exports2, module2) {
    if (typeof process === "undefined" || process.type === "renderer" || process.browser === true || process.__nwjs) {
      module2.exports = require_browser();
    } else {
      module2.exports = require_node();
    }
  }
});

// node_modules/follow-redirects/debug.js
var require_debug = __commonJS({
  "node_modules/follow-redirects/debug.js"(exports2, module2) {
    var debug;
    module2.exports = function() {
      if (!debug) {
        try {
          debug = require_src()("follow-redirects");
        } catch (error) {
        }
        if (typeof debug !== "function") {
          debug = function() {
          };
        }
      }
      debug.apply(null, arguments);
    };
  }
});

// node_modules/follow-redirects/index.js
var require_follow_redirects = __commonJS({
  "node_modules/follow-redirects/index.js"(exports2, module2) {
    var url = require("url");
    var URL2 = url.URL;
    var http = require("http");
    var https = require("https");
    var Writable = require("stream").Writable;
    var assert = require("assert");
    var debug = require_debug();
    (function detectUnsupportedEnvironment() {
      var looksLikeNode = typeof process !== "undefined";
      var looksLikeBrowser = typeof window !== "undefined" && typeof document !== "undefined";
      var looksLikeV8 = isFunction(Error.captureStackTrace);
      if (!looksLikeNode && (looksLikeBrowser || !looksLikeV8)) {
        console.warn("The follow-redirects package should be excluded from browser builds.");
      }
    })();
    var useNativeURL = false;
    try {
      assert(new URL2(""));
    } catch (error) {
      useNativeURL = error.code === "ERR_INVALID_URL";
    }
    var preservedUrlFields = [
      "auth",
      "host",
      "hostname",
      "href",
      "path",
      "pathname",
      "port",
      "protocol",
      "query",
      "search",
      "hash"
    ];
    var events = ["abort", "aborted", "connect", "error", "socket", "timeout"];
    var eventHandlers = /* @__PURE__ */ Object.create(null);
    events.forEach(function(event) {
      eventHandlers[event] = function(arg1, arg2, arg3) {
        this._redirectable.emit(event, arg1, arg2, arg3);
      };
    });
    var InvalidUrlError = createErrorType(
      "ERR_INVALID_URL",
      "Invalid URL",
      TypeError
    );
    var RedirectionError = createErrorType(
      "ERR_FR_REDIRECTION_FAILURE",
      "Redirected request failed"
    );
    var TooManyRedirectsError = createErrorType(
      "ERR_FR_TOO_MANY_REDIRECTS",
      "Maximum number of redirects exceeded",
      RedirectionError
    );
    var MaxBodyLengthExceededError = createErrorType(
      "ERR_FR_MAX_BODY_LENGTH_EXCEEDED",
      "Request body larger than maxBodyLength limit"
    );
    var WriteAfterEndError = createErrorType(
      "ERR_STREAM_WRITE_AFTER_END",
      "write after end"
    );
    var destroy = Writable.prototype.destroy || noop;
    function RedirectableRequest(options, responseCallback) {
      Writable.call(this);
      this._sanitizeOptions(options);
      this._options = options;
      this._ended = false;
      this._ending = false;
      this._redirectCount = 0;
      this._redirects = [];
      this._requestBodyLength = 0;
      this._requestBodyBuffers = [];
      if (responseCallback) {
        this.on("response", responseCallback);
      }
      var self2 = this;
      this._onNativeResponse = function(response) {
        try {
          self2._processResponse(response);
        } catch (cause) {
          self2.emit("error", cause instanceof RedirectionError ? cause : new RedirectionError({ cause }));
        }
      };
      this._performRequest();
    }
    RedirectableRequest.prototype = Object.create(Writable.prototype);
    RedirectableRequest.prototype.abort = function() {
      destroyRequest(this._currentRequest);
      this._currentRequest.abort();
      this.emit("abort");
    };
    RedirectableRequest.prototype.destroy = function(error) {
      destroyRequest(this._currentRequest, error);
      destroy.call(this, error);
      return this;
    };
    RedirectableRequest.prototype.write = function(data, encoding, callback) {
      if (this._ending) {
        throw new WriteAfterEndError();
      }
      if (!isString(data) && !isBuffer(data)) {
        throw new TypeError("data should be a string, Buffer or Uint8Array");
      }
      if (isFunction(encoding)) {
        callback = encoding;
        encoding = null;
      }
      if (data.length === 0) {
        if (callback) {
          callback();
        }
        return;
      }
      if (this._requestBodyLength + data.length <= this._options.maxBodyLength) {
        this._requestBodyLength += data.length;
        this._requestBodyBuffers.push({ data, encoding });
        this._currentRequest.write(data, encoding, callback);
      } else {
        this.emit("error", new MaxBodyLengthExceededError());
        this.abort();
      }
    };
    RedirectableRequest.prototype.end = function(data, encoding, callback) {
      if (isFunction(data)) {
        callback = data;
        data = encoding = null;
      } else if (isFunction(encoding)) {
        callback = encoding;
        encoding = null;
      }
      if (!data) {
        this._ended = this._ending = true;
        this._currentRequest.end(null, null, callback);
      } else {
        var self2 = this;
        var currentRequest = this._currentRequest;
        this.write(data, encoding, function() {
          self2._ended = true;
          currentRequest.end(null, null, callback);
        });
        this._ending = true;
      }
    };
    RedirectableRequest.prototype.setHeader = function(name, value) {
      this._options.headers[name] = value;
      this._currentRequest.setHeader(name, value);
    };
    RedirectableRequest.prototype.removeHeader = function(name) {
      delete this._options.headers[name];
      this._currentRequest.removeHeader(name);
    };
    RedirectableRequest.prototype.setTimeout = function(msecs, callback) {
      var self2 = this;
      function destroyOnTimeout(socket) {
        socket.setTimeout(msecs);
        socket.removeListener("timeout", socket.destroy);
        socket.addListener("timeout", socket.destroy);
      }
      function startTimer(socket) {
        if (self2._timeout) {
          clearTimeout(self2._timeout);
        }
        self2._timeout = setTimeout(function() {
          self2.emit("timeout");
          clearTimer();
        }, msecs);
        destroyOnTimeout(socket);
      }
      function clearTimer() {
        if (self2._timeout) {
          clearTimeout(self2._timeout);
          self2._timeout = null;
        }
        self2.removeListener("abort", clearTimer);
        self2.removeListener("error", clearTimer);
        self2.removeListener("response", clearTimer);
        self2.removeListener("close", clearTimer);
        if (callback) {
          self2.removeListener("timeout", callback);
        }
        if (!self2.socket) {
          self2._currentRequest.removeListener("socket", startTimer);
        }
      }
      if (callback) {
        this.on("timeout", callback);
      }
      if (this.socket) {
        startTimer(this.socket);
      } else {
        this._currentRequest.once("socket", startTimer);
      }
      this.on("socket", destroyOnTimeout);
      this.on("abort", clearTimer);
      this.on("error", clearTimer);
      this.on("response", clearTimer);
      this.on("close", clearTimer);
      return this;
    };
    [
      "flushHeaders",
      "getHeader",
      "setNoDelay",
      "setSocketKeepAlive"
    ].forEach(function(method) {
      RedirectableRequest.prototype[method] = function(a, b) {
        return this._currentRequest[method](a, b);
      };
    });
    ["aborted", "connection", "socket"].forEach(function(property) {
      Object.defineProperty(RedirectableRequest.prototype, property, {
        get: function() {
          return this._currentRequest[property];
        }
      });
    });
    RedirectableRequest.prototype._sanitizeOptions = function(options) {
      if (!options.headers) {
        options.headers = {};
      }
      if (options.host) {
        if (!options.hostname) {
          options.hostname = options.host;
        }
        delete options.host;
      }
      if (!options.pathname && options.path) {
        var searchPos = options.path.indexOf("?");
        if (searchPos < 0) {
          options.pathname = options.path;
        } else {
          options.pathname = options.path.substring(0, searchPos);
          options.search = options.path.substring(searchPos);
        }
      }
    };
    RedirectableRequest.prototype._performRequest = function() {
      var protocol = this._options.protocol;
      var nativeProtocol = this._options.nativeProtocols[protocol];
      if (!nativeProtocol) {
        throw new TypeError("Unsupported protocol " + protocol);
      }
      if (this._options.agents) {
        var scheme = protocol.slice(0, -1);
        this._options.agent = this._options.agents[scheme];
      }
      var request = this._currentRequest = nativeProtocol.request(this._options, this._onNativeResponse);
      request._redirectable = this;
      for (var event of events) {
        request.on(event, eventHandlers[event]);
      }
      this._currentUrl = /^\//.test(this._options.path) ? url.format(this._options) : (
        // When making a request to a proxy, […]
        // a client MUST send the target URI in absolute-form […].
        this._options.path
      );
      if (this._isRedirect) {
        var i = 0;
        var self2 = this;
        var buffers = this._requestBodyBuffers;
        (function writeNext(error) {
          if (request === self2._currentRequest) {
            if (error) {
              self2.emit("error", error);
            } else if (i < buffers.length) {
              var buffer = buffers[i++];
              if (!request.finished) {
                request.write(buffer.data, buffer.encoding, writeNext);
              }
            } else if (self2._ended) {
              request.end();
            }
          }
        })();
      }
    };
    RedirectableRequest.prototype._processResponse = function(response) {
      var statusCode = response.statusCode;
      if (this._options.trackRedirects) {
        this._redirects.push({
          url: this._currentUrl,
          headers: response.headers,
          statusCode
        });
      }
      var location = response.headers.location;
      if (!location || this._options.followRedirects === false || statusCode < 300 || statusCode >= 400) {
        response.responseUrl = this._currentUrl;
        response.redirects = this._redirects;
        this.emit("response", response);
        this._requestBodyBuffers = [];
        return;
      }
      destroyRequest(this._currentRequest);
      response.destroy();
      if (++this._redirectCount > this._options.maxRedirects) {
        throw new TooManyRedirectsError();
      }
      var requestHeaders;
      var beforeRedirect = this._options.beforeRedirect;
      if (beforeRedirect) {
        requestHeaders = Object.assign({
          // The Host header was set by nativeProtocol.request
          Host: response.req.getHeader("host")
        }, this._options.headers);
      }
      var method = this._options.method;
      if ((statusCode === 301 || statusCode === 302) && this._options.method === "POST" || // RFC7231§6.4.4: The 303 (See Other) status code indicates that
      // the server is redirecting the user agent to a different resource […]
      // A user agent can perform a retrieval request targeting that URI
      // (a GET or HEAD request if using HTTP) […]
      statusCode === 303 && !/^(?:GET|HEAD)$/.test(this._options.method)) {
        this._options.method = "GET";
        this._requestBodyBuffers = [];
        removeMatchingHeaders(/^content-/i, this._options.headers);
      }
      var currentHostHeader = removeMatchingHeaders(/^host$/i, this._options.headers);
      var currentUrlParts = parseUrl(this._currentUrl);
      var currentHost = currentHostHeader || currentUrlParts.host;
      var currentUrl = /^\w+:/.test(location) ? this._currentUrl : url.format(Object.assign(currentUrlParts, { host: currentHost }));
      var redirectUrl = resolveUrl(location, currentUrl);
      debug("redirecting to", redirectUrl.href);
      this._isRedirect = true;
      spreadUrlObject(redirectUrl, this._options);
      if (redirectUrl.protocol !== currentUrlParts.protocol && redirectUrl.protocol !== "https:" || redirectUrl.host !== currentHost && !isSubdomain(redirectUrl.host, currentHost)) {
        removeMatchingHeaders(/^(?:(?:proxy-)?authorization|cookie)$/i, this._options.headers);
      }
      if (isFunction(beforeRedirect)) {
        var responseDetails = {
          headers: response.headers,
          statusCode
        };
        var requestDetails = {
          url: currentUrl,
          method,
          headers: requestHeaders
        };
        beforeRedirect(this._options, responseDetails, requestDetails);
        this._sanitizeOptions(this._options);
      }
      this._performRequest();
    };
    function wrap(protocols) {
      var exports3 = {
        maxRedirects: 21,
        maxBodyLength: 10 * 1024 * 1024
      };
      var nativeProtocols = {};
      Object.keys(protocols).forEach(function(scheme) {
        var protocol = scheme + ":";
        var nativeProtocol = nativeProtocols[protocol] = protocols[scheme];
        var wrappedProtocol = exports3[scheme] = Object.create(nativeProtocol);
        function request(input, options, callback) {
          if (isURL(input)) {
            input = spreadUrlObject(input);
          } else if (isString(input)) {
            input = spreadUrlObject(parseUrl(input));
          } else {
            callback = options;
            options = validateUrl(input);
            input = { protocol };
          }
          if (isFunction(options)) {
            callback = options;
            options = null;
          }
          options = Object.assign({
            maxRedirects: exports3.maxRedirects,
            maxBodyLength: exports3.maxBodyLength
          }, input, options);
          options.nativeProtocols = nativeProtocols;
          if (!isString(options.host) && !isString(options.hostname)) {
            options.hostname = "::1";
          }
          assert.equal(options.protocol, protocol, "protocol mismatch");
          debug("options", options);
          return new RedirectableRequest(options, callback);
        }
        function get(input, options, callback) {
          var wrappedRequest = wrappedProtocol.request(input, options, callback);
          wrappedRequest.end();
          return wrappedRequest;
        }
        Object.defineProperties(wrappedProtocol, {
          request: { value: request, configurable: true, enumerable: true, writable: true },
          get: { value: get, configurable: true, enumerable: true, writable: true }
        });
      });
      return exports3;
    }
    function noop() {
    }
    function parseUrl(input) {
      var parsed;
      if (useNativeURL) {
        parsed = new URL2(input);
      } else {
        parsed = validateUrl(url.parse(input));
        if (!isString(parsed.protocol)) {
          throw new InvalidUrlError({ input });
        }
      }
      return parsed;
    }
    function resolveUrl(relative, base) {
      return useNativeURL ? new URL2(relative, base) : parseUrl(url.resolve(base, relative));
    }
    function validateUrl(input) {
      if (/^\[/.test(input.hostname) && !/^\[[:0-9a-f]+\]$/i.test(input.hostname)) {
        throw new InvalidUrlError({ input: input.href || input });
      }
      if (/^\[/.test(input.host) && !/^\[[:0-9a-f]+\](:\d+)?$/i.test(input.host)) {
        throw new InvalidUrlError({ input: input.href || input });
      }
      return input;
    }
    function spreadUrlObject(urlObject, target) {
      var spread = target || {};
      for (var key of preservedUrlFields) {
        spread[key] = urlObject[key];
      }
      if (spread.hostname.startsWith("[")) {
        spread.hostname = spread.hostname.slice(1, -1);
      }
      if (spread.port !== "") {
        spread.port = Number(spread.port);
      }
      spread.path = spread.search ? spread.pathname + spread.search : spread.pathname;
      return spread;
    }
    function removeMatchingHeaders(regex, headers) {
      var lastValue;
      for (var header in headers) {
        if (regex.test(header)) {
          lastValue = headers[header];
          delete headers[header];
        }
      }
      return lastValue === null || typeof lastValue === "undefined" ? void 0 : String(lastValue).trim();
    }
    function createErrorType(code, message, baseClass) {
      function CustomError(properties) {
        if (isFunction(Error.captureStackTrace)) {
          Error.captureStackTrace(this, this.constructor);
        }
        Object.assign(this, properties || {});
        this.code = code;
        this.message = this.cause ? message + ": " + this.cause.message : message;
      }
      CustomError.prototype = new (baseClass || Error)();
      Object.defineProperties(CustomError.prototype, {
        constructor: {
          value: CustomError,
          enumerable: false
        },
        name: {
          value: "Error [" + code + "]",
          enumerable: false
        }
      });
      return CustomError;
    }
    function destroyRequest(request, error) {
      for (var event of events) {
        request.removeListener(event, eventHandlers[event]);
      }
      request.on("error", noop);
      request.destroy(error);
    }
    function isSubdomain(subdomain, domain) {
      assert(isString(subdomain) && isString(domain));
      var dot = subdomain.length - domain.length - 1;
      return dot > 0 && subdomain[dot] === "." && subdomain.endsWith(domain);
    }
    function isString(value) {
      return typeof value === "string" || value instanceof String;
    }
    function isFunction(value) {
      return typeof value === "function";
    }
    function isBuffer(value) {
      return typeof value === "object" && "length" in value;
    }
    function isURL(value) {
      return URL2 && value instanceof URL2;
    }
    module2.exports = wrap({ http, https });
    module2.exports.wrap = wrap;
  }
});

// node_modules/axios/dist/node/axios.cjs
var require_axios = __commonJS({
  "node_modules/axios/dist/node/axios.cjs"(exports2, module2) {
    "use strict";
    var FormData$1 = require_form_data();
    var crypto = require("crypto");
    var url = require("url");
    var proxyFromEnv = require_proxy_from_env();
    var http = require("http");
    var https = require("https");
    var http2 = require("http2");
    var util = require("util");
    var followRedirects = require_follow_redirects();
    var zlib = require("zlib");
    var stream = require("stream");
    var events = require("events");
    function _interopDefaultLegacy(e) {
      return e && typeof e === "object" && "default" in e ? e : { "default": e };
    }
    var FormData__default = /* @__PURE__ */ _interopDefaultLegacy(FormData$1);
    var crypto__default = /* @__PURE__ */ _interopDefaultLegacy(crypto);
    var url__default = /* @__PURE__ */ _interopDefaultLegacy(url);
    var proxyFromEnv__default = /* @__PURE__ */ _interopDefaultLegacy(proxyFromEnv);
    var http__default = /* @__PURE__ */ _interopDefaultLegacy(http);
    var https__default = /* @__PURE__ */ _interopDefaultLegacy(https);
    var http2__default = /* @__PURE__ */ _interopDefaultLegacy(http2);
    var util__default = /* @__PURE__ */ _interopDefaultLegacy(util);
    var followRedirects__default = /* @__PURE__ */ _interopDefaultLegacy(followRedirects);
    var zlib__default = /* @__PURE__ */ _interopDefaultLegacy(zlib);
    var stream__default = /* @__PURE__ */ _interopDefaultLegacy(stream);
    function bind(fn, thisArg) {
      return function wrap() {
        return fn.apply(thisArg, arguments);
      };
    }
    var { toString } = Object.prototype;
    var { getPrototypeOf } = Object;
    var { iterator, toStringTag } = Symbol;
    var kindOf = /* @__PURE__ */ ((cache) => (thing) => {
      const str2 = toString.call(thing);
      return cache[str2] || (cache[str2] = str2.slice(8, -1).toLowerCase());
    })(/* @__PURE__ */ Object.create(null));
    var kindOfTest = (type) => {
      type = type.toLowerCase();
      return (thing) => kindOf(thing) === type;
    };
    var typeOfTest = (type) => (thing) => typeof thing === type;
    var { isArray } = Array;
    var isUndefined = typeOfTest("undefined");
    function isBuffer(val) {
      return val !== null && !isUndefined(val) && val.constructor !== null && !isUndefined(val.constructor) && isFunction$1(val.constructor.isBuffer) && val.constructor.isBuffer(val);
    }
    var isArrayBuffer = kindOfTest("ArrayBuffer");
    function isArrayBufferView(val) {
      let result;
      if (typeof ArrayBuffer !== "undefined" && ArrayBuffer.isView) {
        result = ArrayBuffer.isView(val);
      } else {
        result = val && val.buffer && isArrayBuffer(val.buffer);
      }
      return result;
    }
    var isString = typeOfTest("string");
    var isFunction$1 = typeOfTest("function");
    var isNumber = typeOfTest("number");
    var isObject = (thing) => thing !== null && typeof thing === "object";
    var isBoolean = (thing) => thing === true || thing === false;
    var isPlainObject = (val) => {
      if (kindOf(val) !== "object") {
        return false;
      }
      const prototype2 = getPrototypeOf(val);
      return (prototype2 === null || prototype2 === Object.prototype || Object.getPrototypeOf(prototype2) === null) && !(toStringTag in val) && !(iterator in val);
    };
    var isEmptyObject = (val) => {
      if (!isObject(val) || isBuffer(val)) {
        return false;
      }
      try {
        return Object.keys(val).length === 0 && Object.getPrototypeOf(val) === Object.prototype;
      } catch (e) {
        return false;
      }
    };
    var isDate = kindOfTest("Date");
    var isFile = kindOfTest("File");
    var isBlob = kindOfTest("Blob");
    var isFileList = kindOfTest("FileList");
    var isStream = (val) => isObject(val) && isFunction$1(val.pipe);
    var isFormData = (thing) => {
      let kind;
      return thing && (typeof FormData === "function" && thing instanceof FormData || isFunction$1(thing.append) && ((kind = kindOf(thing)) === "formdata" || // detect form-data instance
      kind === "object" && isFunction$1(thing.toString) && thing.toString() === "[object FormData]"));
    };
    var isURLSearchParams = kindOfTest("URLSearchParams");
    var [isReadableStream, isRequest, isResponse, isHeaders] = ["ReadableStream", "Request", "Response", "Headers"].map(kindOfTest);
    var trim = (str2) => str2.trim ? str2.trim() : str2.replace(/^[\s\uFEFF\xA0]+|[\s\uFEFF\xA0]+$/g, "");
    function forEach(obj, fn, { allOwnKeys = false } = {}) {
      if (obj === null || typeof obj === "undefined") {
        return;
      }
      let i;
      let l;
      if (typeof obj !== "object") {
        obj = [obj];
      }
      if (isArray(obj)) {
        for (i = 0, l = obj.length; i < l; i++) {
          fn.call(null, obj[i], i, obj);
        }
      } else {
        if (isBuffer(obj)) {
          return;
        }
        const keys = allOwnKeys ? Object.getOwnPropertyNames(obj) : Object.keys(obj);
        const len = keys.length;
        let key;
        for (i = 0; i < len; i++) {
          key = keys[i];
          fn.call(null, obj[key], key, obj);
        }
      }
    }
    function findKey(obj, key) {
      if (isBuffer(obj)) {
        return null;
      }
      key = key.toLowerCase();
      const keys = Object.keys(obj);
      let i = keys.length;
      let _key;
      while (i-- > 0) {
        _key = keys[i];
        if (key === _key.toLowerCase()) {
          return _key;
        }
      }
      return null;
    }
    var _global = (() => {
      if (typeof globalThis !== "undefined")
        return globalThis;
      return typeof self !== "undefined" ? self : typeof window !== "undefined" ? window : global;
    })();
    var isContextDefined = (context) => !isUndefined(context) && context !== _global;
    function merge() {
      const { caseless, skipUndefined } = isContextDefined(this) && this || {};
      const result = {};
      const assignValue = (val, key) => {
        const targetKey = caseless && findKey(result, key) || key;
        if (isPlainObject(result[targetKey]) && isPlainObject(val)) {
          result[targetKey] = merge(result[targetKey], val);
        } else if (isPlainObject(val)) {
          result[targetKey] = merge({}, val);
        } else if (isArray(val)) {
          result[targetKey] = val.slice();
        } else if (!skipUndefined || !isUndefined(val)) {
          result[targetKey] = val;
        }
      };
      for (let i = 0, l = arguments.length; i < l; i++) {
        arguments[i] && forEach(arguments[i], assignValue);
      }
      return result;
    }
    var extend = (a, b, thisArg, { allOwnKeys } = {}) => {
      forEach(b, (val, key) => {
        if (thisArg && isFunction$1(val)) {
          a[key] = bind(val, thisArg);
        } else {
          a[key] = val;
        }
      }, { allOwnKeys });
      return a;
    };
    var stripBOM = (content) => {
      if (content.charCodeAt(0) === 65279) {
        content = content.slice(1);
      }
      return content;
    };
    var inherits = (constructor, superConstructor, props, descriptors2) => {
      constructor.prototype = Object.create(superConstructor.prototype, descriptors2);
      constructor.prototype.constructor = constructor;
      Object.defineProperty(constructor, "super", {
        value: superConstructor.prototype
      });
      props && Object.assign(constructor.prototype, props);
    };
    var toFlatObject = (sourceObj, destObj, filter, propFilter) => {
      let props;
      let i;
      let prop;
      const merged = {};
      destObj = destObj || {};
      if (sourceObj == null)
        return destObj;
      do {
        props = Object.getOwnPropertyNames(sourceObj);
        i = props.length;
        while (i-- > 0) {
          prop = props[i];
          if ((!propFilter || propFilter(prop, sourceObj, destObj)) && !merged[prop]) {
            destObj[prop] = sourceObj[prop];
            merged[prop] = true;
          }
        }
        sourceObj = filter !== false && getPrototypeOf(sourceObj);
      } while (sourceObj && (!filter || filter(sourceObj, destObj)) && sourceObj !== Object.prototype);
      return destObj;
    };
    var endsWith = (str2, searchString, position) => {
      str2 = String(str2);
      if (position === void 0 || position > str2.length) {
        position = str2.length;
      }
      position -= searchString.length;
      const lastIndex = str2.indexOf(searchString, position);
      return lastIndex !== -1 && lastIndex === position;
    };
    var toArray = (thing) => {
      if (!thing)
        return null;
      if (isArray(thing))
        return thing;
      let i = thing.length;
      if (!isNumber(i))
        return null;
      const arr = new Array(i);
      while (i-- > 0) {
        arr[i] = thing[i];
      }
      return arr;
    };
    var isTypedArray = /* @__PURE__ */ ((TypedArray) => {
      return (thing) => {
        return TypedArray && thing instanceof TypedArray;
      };
    })(typeof Uint8Array !== "undefined" && getPrototypeOf(Uint8Array));
    var forEachEntry = (obj, fn) => {
      const generator = obj && obj[iterator];
      const _iterator = generator.call(obj);
      let result;
      while ((result = _iterator.next()) && !result.done) {
        const pair = result.value;
        fn.call(obj, pair[0], pair[1]);
      }
    };
    var matchAll = (regExp, str2) => {
      let matches;
      const arr = [];
      while ((matches = regExp.exec(str2)) !== null) {
        arr.push(matches);
      }
      return arr;
    };
    var isHTMLForm = kindOfTest("HTMLFormElement");
    var toCamelCase = (str2) => {
      return str2.toLowerCase().replace(
        /[-_\s]([a-z\d])(\w*)/g,
        function replacer(m, p1, p2) {
          return p1.toUpperCase() + p2;
        }
      );
    };
    var hasOwnProperty = (({ hasOwnProperty: hasOwnProperty2 }) => (obj, prop) => hasOwnProperty2.call(obj, prop))(Object.prototype);
    var isRegExp = kindOfTest("RegExp");
    var reduceDescriptors = (obj, reducer) => {
      const descriptors2 = Object.getOwnPropertyDescriptors(obj);
      const reducedDescriptors = {};
      forEach(descriptors2, (descriptor, name) => {
        let ret;
        if ((ret = reducer(descriptor, name, obj)) !== false) {
          reducedDescriptors[name] = ret || descriptor;
        }
      });
      Object.defineProperties(obj, reducedDescriptors);
    };
    var freezeMethods = (obj) => {
      reduceDescriptors(obj, (descriptor, name) => {
        if (isFunction$1(obj) && ["arguments", "caller", "callee"].indexOf(name) !== -1) {
          return false;
        }
        const value = obj[name];
        if (!isFunction$1(value))
          return;
        descriptor.enumerable = false;
        if ("writable" in descriptor) {
          descriptor.writable = false;
          return;
        }
        if (!descriptor.set) {
          descriptor.set = () => {
            throw Error("Can not rewrite read-only method '" + name + "'");
          };
        }
      });
    };
    var toObjectSet = (arrayOrString, delimiter) => {
      const obj = {};
      const define = (arr) => {
        arr.forEach((value) => {
          obj[value] = true;
        });
      };
      isArray(arrayOrString) ? define(arrayOrString) : define(String(arrayOrString).split(delimiter));
      return obj;
    };
    var noop = () => {
    };
    var toFiniteNumber = (value, defaultValue) => {
      return value != null && Number.isFinite(value = +value) ? value : defaultValue;
    };
    function isSpecCompliantForm(thing) {
      return !!(thing && isFunction$1(thing.append) && thing[toStringTag] === "FormData" && thing[iterator]);
    }
    var toJSONObject = (obj) => {
      const stack = new Array(10);
      const visit = (source, i) => {
        if (isObject(source)) {
          if (stack.indexOf(source) >= 0) {
            return;
          }
          if (isBuffer(source)) {
            return source;
          }
          if (!("toJSON" in source)) {
            stack[i] = source;
            const target = isArray(source) ? [] : {};
            forEach(source, (value, key) => {
              const reducedValue = visit(value, i + 1);
              !isUndefined(reducedValue) && (target[key] = reducedValue);
            });
            stack[i] = void 0;
            return target;
          }
        }
        return source;
      };
      return visit(obj, 0);
    };
    var isAsyncFn = kindOfTest("AsyncFunction");
    var isThenable = (thing) => thing && (isObject(thing) || isFunction$1(thing)) && isFunction$1(thing.then) && isFunction$1(thing.catch);
    var _setImmediate = ((setImmediateSupported, postMessageSupported) => {
      if (setImmediateSupported) {
        return setImmediate;
      }
      return postMessageSupported ? ((token, callbacks) => {
        _global.addEventListener("message", ({ source, data }) => {
          if (source === _global && data === token) {
            callbacks.length && callbacks.shift()();
          }
        }, false);
        return (cb) => {
          callbacks.push(cb);
          _global.postMessage(token, "*");
        };
      })(`axios@${Math.random()}`, []) : (cb) => setTimeout(cb);
    })(
      typeof setImmediate === "function",
      isFunction$1(_global.postMessage)
    );
    var asap = typeof queueMicrotask !== "undefined" ? queueMicrotask.bind(_global) : typeof process !== "undefined" && process.nextTick || _setImmediate;
    var isIterable = (thing) => thing != null && isFunction$1(thing[iterator]);
    var utils$1 = {
      isArray,
      isArrayBuffer,
      isBuffer,
      isFormData,
      isArrayBufferView,
      isString,
      isNumber,
      isBoolean,
      isObject,
      isPlainObject,
      isEmptyObject,
      isReadableStream,
      isRequest,
      isResponse,
      isHeaders,
      isUndefined,
      isDate,
      isFile,
      isBlob,
      isRegExp,
      isFunction: isFunction$1,
      isStream,
      isURLSearchParams,
      isTypedArray,
      isFileList,
      forEach,
      merge,
      extend,
      trim,
      stripBOM,
      inherits,
      toFlatObject,
      kindOf,
      kindOfTest,
      endsWith,
      toArray,
      forEachEntry,
      matchAll,
      isHTMLForm,
      hasOwnProperty,
      hasOwnProp: hasOwnProperty,
      // an alias to avoid ESLint no-prototype-builtins detection
      reduceDescriptors,
      freezeMethods,
      toObjectSet,
      toCamelCase,
      noop,
      toFiniteNumber,
      findKey,
      global: _global,
      isContextDefined,
      isSpecCompliantForm,
      toJSONObject,
      isAsyncFn,
      isThenable,
      setImmediate: _setImmediate,
      asap,
      isIterable
    };
    function AxiosError(message, code, config, request, response) {
      Error.call(this);
      if (Error.captureStackTrace) {
        Error.captureStackTrace(this, this.constructor);
      } else {
        this.stack = new Error().stack;
      }
      this.message = message;
      this.name = "AxiosError";
      code && (this.code = code);
      config && (this.config = config);
      request && (this.request = request);
      if (response) {
        this.response = response;
        this.status = response.status ? response.status : null;
      }
    }
    utils$1.inherits(AxiosError, Error, {
      toJSON: function toJSON() {
        return {
          // Standard
          message: this.message,
          name: this.name,
          // Microsoft
          description: this.description,
          number: this.number,
          // Mozilla
          fileName: this.fileName,
          lineNumber: this.lineNumber,
          columnNumber: this.columnNumber,
          stack: this.stack,
          // Axios
          config: utils$1.toJSONObject(this.config),
          code: this.code,
          status: this.status
        };
      }
    });
    var prototype$1 = AxiosError.prototype;
    var descriptors = {};
    [
      "ERR_BAD_OPTION_VALUE",
      "ERR_BAD_OPTION",
      "ECONNABORTED",
      "ETIMEDOUT",
      "ERR_NETWORK",
      "ERR_FR_TOO_MANY_REDIRECTS",
      "ERR_DEPRECATED",
      "ERR_BAD_RESPONSE",
      "ERR_BAD_REQUEST",
      "ERR_CANCELED",
      "ERR_NOT_SUPPORT",
      "ERR_INVALID_URL"
      // eslint-disable-next-line func-names
    ].forEach((code) => {
      descriptors[code] = { value: code };
    });
    Object.defineProperties(AxiosError, descriptors);
    Object.defineProperty(prototype$1, "isAxiosError", { value: true });
    AxiosError.from = (error, code, config, request, response, customProps) => {
      const axiosError = Object.create(prototype$1);
      utils$1.toFlatObject(error, axiosError, function filter(obj) {
        return obj !== Error.prototype;
      }, (prop) => {
        return prop !== "isAxiosError";
      });
      const msg = error && error.message ? error.message : "Error";
      const errCode = code == null && error ? error.code : code;
      AxiosError.call(axiosError, msg, errCode, config, request, response);
      if (error && axiosError.cause == null) {
        Object.defineProperty(axiosError, "cause", { value: error, configurable: true });
      }
      axiosError.name = error && error.name || "Error";
      customProps && Object.assign(axiosError, customProps);
      return axiosError;
    };
    function isVisitable(thing) {
      return utils$1.isPlainObject(thing) || utils$1.isArray(thing);
    }
    function removeBrackets(key) {
      return utils$1.endsWith(key, "[]") ? key.slice(0, -2) : key;
    }
    function renderKey(path, key, dots) {
      if (!path)
        return key;
      return path.concat(key).map(function each(token, i) {
        token = removeBrackets(token);
        return !dots && i ? "[" + token + "]" : token;
      }).join(dots ? "." : "");
    }
    function isFlatArray(arr) {
      return utils$1.isArray(arr) && !arr.some(isVisitable);
    }
    var predicates = utils$1.toFlatObject(utils$1, {}, null, function filter(prop) {
      return /^is[A-Z]/.test(prop);
    });
    function toFormData(obj, formData, options) {
      if (!utils$1.isObject(obj)) {
        throw new TypeError("target must be an object");
      }
      formData = formData || new (FormData__default["default"] || FormData)();
      options = utils$1.toFlatObject(options, {
        metaTokens: true,
        dots: false,
        indexes: false
      }, false, function defined(option, source) {
        return !utils$1.isUndefined(source[option]);
      });
      const metaTokens = options.metaTokens;
      const visitor = options.visitor || defaultVisitor;
      const dots = options.dots;
      const indexes = options.indexes;
      const _Blob = options.Blob || typeof Blob !== "undefined" && Blob;
      const useBlob = _Blob && utils$1.isSpecCompliantForm(formData);
      if (!utils$1.isFunction(visitor)) {
        throw new TypeError("visitor must be a function");
      }
      function convertValue(value) {
        if (value === null)
          return "";
        if (utils$1.isDate(value)) {
          return value.toISOString();
        }
        if (utils$1.isBoolean(value)) {
          return value.toString();
        }
        if (!useBlob && utils$1.isBlob(value)) {
          throw new AxiosError("Blob is not supported. Use a Buffer instead.");
        }
        if (utils$1.isArrayBuffer(value) || utils$1.isTypedArray(value)) {
          return useBlob && typeof Blob === "function" ? new Blob([value]) : Buffer.from(value);
        }
        return value;
      }
      function defaultVisitor(value, key, path) {
        let arr = value;
        if (value && !path && typeof value === "object") {
          if (utils$1.endsWith(key, "{}")) {
            key = metaTokens ? key : key.slice(0, -2);
            value = JSON.stringify(value);
          } else if (utils$1.isArray(value) && isFlatArray(value) || (utils$1.isFileList(value) || utils$1.endsWith(key, "[]")) && (arr = utils$1.toArray(value))) {
            key = removeBrackets(key);
            arr.forEach(function each(el, index) {
              !(utils$1.isUndefined(el) || el === null) && formData.append(
                // eslint-disable-next-line no-nested-ternary
                indexes === true ? renderKey([key], index, dots) : indexes === null ? key : key + "[]",
                convertValue(el)
              );
            });
            return false;
          }
        }
        if (isVisitable(value)) {
          return true;
        }
        formData.append(renderKey(path, key, dots), convertValue(value));
        return false;
      }
      const stack = [];
      const exposedHelpers = Object.assign(predicates, {
        defaultVisitor,
        convertValue,
        isVisitable
      });
      function build(value, path) {
        if (utils$1.isUndefined(value))
          return;
        if (stack.indexOf(value) !== -1) {
          throw Error("Circular reference detected in " + path.join("."));
        }
        stack.push(value);
        utils$1.forEach(value, function each(el, key) {
          const result = !(utils$1.isUndefined(el) || el === null) && visitor.call(
            formData,
            el,
            utils$1.isString(key) ? key.trim() : key,
            path,
            exposedHelpers
          );
          if (result === true) {
            build(el, path ? path.concat(key) : [key]);
          }
        });
        stack.pop();
      }
      if (!utils$1.isObject(obj)) {
        throw new TypeError("data must be an object");
      }
      build(obj);
      return formData;
    }
    function encode$1(str2) {
      const charMap = {
        "!": "%21",
        "'": "%27",
        "(": "%28",
        ")": "%29",
        "~": "%7E",
        "%20": "+",
        "%00": "\0"
      };
      return encodeURIComponent(str2).replace(/[!'()~]|%20|%00/g, function replacer(match) {
        return charMap[match];
      });
    }
    function AxiosURLSearchParams(params, options) {
      this._pairs = [];
      params && toFormData(params, this, options);
    }
    var prototype = AxiosURLSearchParams.prototype;
    prototype.append = function append(name, value) {
      this._pairs.push([name, value]);
    };
    prototype.toString = function toString2(encoder) {
      const _encode = encoder ? function(value) {
        return encoder.call(this, value, encode$1);
      } : encode$1;
      return this._pairs.map(function each(pair) {
        return _encode(pair[0]) + "=" + _encode(pair[1]);
      }, "").join("&");
    };
    function encode(val) {
      return encodeURIComponent(val).replace(/%3A/gi, ":").replace(/%24/g, "$").replace(/%2C/gi, ",").replace(/%20/g, "+");
    }
    function buildURL(url2, params, options) {
      if (!params) {
        return url2;
      }
      const _encode = options && options.encode || encode;
      if (utils$1.isFunction(options)) {
        options = {
          serialize: options
        };
      }
      const serializeFn = options && options.serialize;
      let serializedParams;
      if (serializeFn) {
        serializedParams = serializeFn(params, options);
      } else {
        serializedParams = utils$1.isURLSearchParams(params) ? params.toString() : new AxiosURLSearchParams(params, options).toString(_encode);
      }
      if (serializedParams) {
        const hashmarkIndex = url2.indexOf("#");
        if (hashmarkIndex !== -1) {
          url2 = url2.slice(0, hashmarkIndex);
        }
        url2 += (url2.indexOf("?") === -1 ? "?" : "&") + serializedParams;
      }
      return url2;
    }
    var InterceptorManager = class {
      constructor() {
        this.handlers = [];
      }
      /**
       * Add a new interceptor to the stack
       *
       * @param {Function} fulfilled The function to handle `then` for a `Promise`
       * @param {Function} rejected The function to handle `reject` for a `Promise`
       *
       * @return {Number} An ID used to remove interceptor later
       */
      use(fulfilled, rejected, options) {
        this.handlers.push({
          fulfilled,
          rejected,
          synchronous: options ? options.synchronous : false,
          runWhen: options ? options.runWhen : null
        });
        return this.handlers.length - 1;
      }
      /**
       * Remove an interceptor from the stack
       *
       * @param {Number} id The ID that was returned by `use`
       *
       * @returns {void}
       */
      eject(id) {
        if (this.handlers[id]) {
          this.handlers[id] = null;
        }
      }
      /**
       * Clear all interceptors from the stack
       *
       * @returns {void}
       */
      clear() {
        if (this.handlers) {
          this.handlers = [];
        }
      }
      /**
       * Iterate over all the registered interceptors
       *
       * This method is particularly useful for skipping over any
       * interceptors that may have become `null` calling `eject`.
       *
       * @param {Function} fn The function to call for each interceptor
       *
       * @returns {void}
       */
      forEach(fn) {
        utils$1.forEach(this.handlers, function forEachHandler(h) {
          if (h !== null) {
            fn(h);
          }
        });
      }
    };
    var InterceptorManager$1 = InterceptorManager;
    var transitionalDefaults = {
      silentJSONParsing: true,
      forcedJSONParsing: true,
      clarifyTimeoutError: false
    };
    var URLSearchParams = url__default["default"].URLSearchParams;
    var ALPHA = "abcdefghijklmnopqrstuvwxyz";
    var DIGIT = "0123456789";
    var ALPHABET = {
      DIGIT,
      ALPHA,
      ALPHA_DIGIT: ALPHA + ALPHA.toUpperCase() + DIGIT
    };
    var generateString = (size = 16, alphabet = ALPHABET.ALPHA_DIGIT) => {
      let str2 = "";
      const { length } = alphabet;
      const randomValues = new Uint32Array(size);
      crypto__default["default"].randomFillSync(randomValues);
      for (let i = 0; i < size; i++) {
        str2 += alphabet[randomValues[i] % length];
      }
      return str2;
    };
    var platform$1 = {
      isNode: true,
      classes: {
        URLSearchParams,
        FormData: FormData__default["default"],
        Blob: typeof Blob !== "undefined" && Blob || null
      },
      ALPHABET,
      generateString,
      protocols: ["http", "https", "file", "data"]
    };
    var hasBrowserEnv = typeof window !== "undefined" && typeof document !== "undefined";
    var _navigator = typeof navigator === "object" && navigator || void 0;
    var hasStandardBrowserEnv = hasBrowserEnv && (!_navigator || ["ReactNative", "NativeScript", "NS"].indexOf(_navigator.product) < 0);
    var hasStandardBrowserWebWorkerEnv = (() => {
      return typeof WorkerGlobalScope !== "undefined" && // eslint-disable-next-line no-undef
      self instanceof WorkerGlobalScope && typeof self.importScripts === "function";
    })();
    var origin = hasBrowserEnv && window.location.href || "http://localhost";
    var utils = /* @__PURE__ */ Object.freeze({
      __proto__: null,
      hasBrowserEnv,
      hasStandardBrowserWebWorkerEnv,
      hasStandardBrowserEnv,
      navigator: _navigator,
      origin
    });
    var platform = {
      ...utils,
      ...platform$1
    };
    function toURLEncodedForm(data, options) {
      return toFormData(data, new platform.classes.URLSearchParams(), {
        visitor: function(value, key, path, helpers) {
          if (platform.isNode && utils$1.isBuffer(value)) {
            this.append(key, value.toString("base64"));
            return false;
          }
          return helpers.defaultVisitor.apply(this, arguments);
        },
        ...options
      });
    }
    function parsePropPath(name) {
      return utils$1.matchAll(/\w+|\[(\w*)]/g, name).map((match) => {
        return match[0] === "[]" ? "" : match[1] || match[0];
      });
    }
    function arrayToObject(arr) {
      const obj = {};
      const keys = Object.keys(arr);
      let i;
      const len = keys.length;
      let key;
      for (i = 0; i < len; i++) {
        key = keys[i];
        obj[key] = arr[key];
      }
      return obj;
    }
    function formDataToJSON(formData) {
      function buildPath(path, value, target, index) {
        let name = path[index++];
        if (name === "__proto__")
          return true;
        const isNumericKey = Number.isFinite(+name);
        const isLast = index >= path.length;
        name = !name && utils$1.isArray(target) ? target.length : name;
        if (isLast) {
          if (utils$1.hasOwnProp(target, name)) {
            target[name] = [target[name], value];
          } else {
            target[name] = value;
          }
          return !isNumericKey;
        }
        if (!target[name] || !utils$1.isObject(target[name])) {
          target[name] = [];
        }
        const result = buildPath(path, value, target[name], index);
        if (result && utils$1.isArray(target[name])) {
          target[name] = arrayToObject(target[name]);
        }
        return !isNumericKey;
      }
      if (utils$1.isFormData(formData) && utils$1.isFunction(formData.entries)) {
        const obj = {};
        utils$1.forEachEntry(formData, (name, value) => {
          buildPath(parsePropPath(name), value, obj, 0);
        });
        return obj;
      }
      return null;
    }
    function stringifySafely(rawValue, parser, encoder) {
      if (utils$1.isString(rawValue)) {
        try {
          (parser || JSON.parse)(rawValue);
          return utils$1.trim(rawValue);
        } catch (e) {
          if (e.name !== "SyntaxError") {
            throw e;
          }
        }
      }
      return (encoder || JSON.stringify)(rawValue);
    }
    var defaults = {
      transitional: transitionalDefaults,
      adapter: ["xhr", "http", "fetch"],
      transformRequest: [function transformRequest(data, headers) {
        const contentType = headers.getContentType() || "";
        const hasJSONContentType = contentType.indexOf("application/json") > -1;
        const isObjectPayload = utils$1.isObject(data);
        if (isObjectPayload && utils$1.isHTMLForm(data)) {
          data = new FormData(data);
        }
        const isFormData2 = utils$1.isFormData(data);
        if (isFormData2) {
          return hasJSONContentType ? JSON.stringify(formDataToJSON(data)) : data;
        }
        if (utils$1.isArrayBuffer(data) || utils$1.isBuffer(data) || utils$1.isStream(data) || utils$1.isFile(data) || utils$1.isBlob(data) || utils$1.isReadableStream(data)) {
          return data;
        }
        if (utils$1.isArrayBufferView(data)) {
          return data.buffer;
        }
        if (utils$1.isURLSearchParams(data)) {
          headers.setContentType("application/x-www-form-urlencoded;charset=utf-8", false);
          return data.toString();
        }
        let isFileList2;
        if (isObjectPayload) {
          if (contentType.indexOf("application/x-www-form-urlencoded") > -1) {
            return toURLEncodedForm(data, this.formSerializer).toString();
          }
          if ((isFileList2 = utils$1.isFileList(data)) || contentType.indexOf("multipart/form-data") > -1) {
            const _FormData = this.env && this.env.FormData;
            return toFormData(
              isFileList2 ? { "files[]": data } : data,
              _FormData && new _FormData(),
              this.formSerializer
            );
          }
        }
        if (isObjectPayload || hasJSONContentType) {
          headers.setContentType("application/json", false);
          return stringifySafely(data);
        }
        return data;
      }],
      transformResponse: [function transformResponse(data) {
        const transitional = this.transitional || defaults.transitional;
        const forcedJSONParsing = transitional && transitional.forcedJSONParsing;
        const JSONRequested = this.responseType === "json";
        if (utils$1.isResponse(data) || utils$1.isReadableStream(data)) {
          return data;
        }
        if (data && utils$1.isString(data) && (forcedJSONParsing && !this.responseType || JSONRequested)) {
          const silentJSONParsing = transitional && transitional.silentJSONParsing;
          const strictJSONParsing = !silentJSONParsing && JSONRequested;
          try {
            return JSON.parse(data, this.parseReviver);
          } catch (e) {
            if (strictJSONParsing) {
              if (e.name === "SyntaxError") {
                throw AxiosError.from(e, AxiosError.ERR_BAD_RESPONSE, this, null, this.response);
              }
              throw e;
            }
          }
        }
        return data;
      }],
      /**
       * A timeout in milliseconds to abort a request. If set to 0 (default) a
       * timeout is not created.
       */
      timeout: 0,
      xsrfCookieName: "XSRF-TOKEN",
      xsrfHeaderName: "X-XSRF-TOKEN",
      maxContentLength: -1,
      maxBodyLength: -1,
      env: {
        FormData: platform.classes.FormData,
        Blob: platform.classes.Blob
      },
      validateStatus: function validateStatus(status) {
        return status >= 200 && status < 300;
      },
      headers: {
        common: {
          "Accept": "application/json, text/plain, */*",
          "Content-Type": void 0
        }
      }
    };
    utils$1.forEach(["delete", "get", "head", "post", "put", "patch"], (method) => {
      defaults.headers[method] = {};
    });
    var defaults$1 = defaults;
    var ignoreDuplicateOf = utils$1.toObjectSet([
      "age",
      "authorization",
      "content-length",
      "content-type",
      "etag",
      "expires",
      "from",
      "host",
      "if-modified-since",
      "if-unmodified-since",
      "last-modified",
      "location",
      "max-forwards",
      "proxy-authorization",
      "referer",
      "retry-after",
      "user-agent"
    ]);
    var parseHeaders = (rawHeaders) => {
      const parsed = {};
      let key;
      let val;
      let i;
      rawHeaders && rawHeaders.split("\n").forEach(function parser(line) {
        i = line.indexOf(":");
        key = line.substring(0, i).trim().toLowerCase();
        val = line.substring(i + 1).trim();
        if (!key || parsed[key] && ignoreDuplicateOf[key]) {
          return;
        }
        if (key === "set-cookie") {
          if (parsed[key]) {
            parsed[key].push(val);
          } else {
            parsed[key] = [val];
          }
        } else {
          parsed[key] = parsed[key] ? parsed[key] + ", " + val : val;
        }
      });
      return parsed;
    };
    var $internals = Symbol("internals");
    function normalizeHeader(header) {
      return header && String(header).trim().toLowerCase();
    }
    function normalizeValue(value) {
      if (value === false || value == null) {
        return value;
      }
      return utils$1.isArray(value) ? value.map(normalizeValue) : String(value);
    }
    function parseTokens(str2) {
      const tokens = /* @__PURE__ */ Object.create(null);
      const tokensRE = /([^\s,;=]+)\s*(?:=\s*([^,;]+))?/g;
      let match;
      while (match = tokensRE.exec(str2)) {
        tokens[match[1]] = match[2];
      }
      return tokens;
    }
    var isValidHeaderName = (str2) => /^[-_a-zA-Z0-9^`|~,!#$%&'*+.]+$/.test(str2.trim());
    function matchHeaderValue(context, value, header, filter, isHeaderNameFilter) {
      if (utils$1.isFunction(filter)) {
        return filter.call(this, value, header);
      }
      if (isHeaderNameFilter) {
        value = header;
      }
      if (!utils$1.isString(value))
        return;
      if (utils$1.isString(filter)) {
        return value.indexOf(filter) !== -1;
      }
      if (utils$1.isRegExp(filter)) {
        return filter.test(value);
      }
    }
    function formatHeader(header) {
      return header.trim().toLowerCase().replace(/([a-z\d])(\w*)/g, (w, char, str2) => {
        return char.toUpperCase() + str2;
      });
    }
    function buildAccessors(obj, header) {
      const accessorName = utils$1.toCamelCase(" " + header);
      ["get", "set", "has"].forEach((methodName) => {
        Object.defineProperty(obj, methodName + accessorName, {
          value: function(arg1, arg2, arg3) {
            return this[methodName].call(this, header, arg1, arg2, arg3);
          },
          configurable: true
        });
      });
    }
    var AxiosHeaders = class {
      constructor(headers) {
        headers && this.set(headers);
      }
      set(header, valueOrRewrite, rewrite) {
        const self2 = this;
        function setHeader(_value, _header, _rewrite) {
          const lHeader = normalizeHeader(_header);
          if (!lHeader) {
            throw new Error("header name must be a non-empty string");
          }
          const key = utils$1.findKey(self2, lHeader);
          if (!key || self2[key] === void 0 || _rewrite === true || _rewrite === void 0 && self2[key] !== false) {
            self2[key || _header] = normalizeValue(_value);
          }
        }
        const setHeaders = (headers, _rewrite) => utils$1.forEach(headers, (_value, _header) => setHeader(_value, _header, _rewrite));
        if (utils$1.isPlainObject(header) || header instanceof this.constructor) {
          setHeaders(header, valueOrRewrite);
        } else if (utils$1.isString(header) && (header = header.trim()) && !isValidHeaderName(header)) {
          setHeaders(parseHeaders(header), valueOrRewrite);
        } else if (utils$1.isObject(header) && utils$1.isIterable(header)) {
          let obj = {}, dest, key;
          for (const entry of header) {
            if (!utils$1.isArray(entry)) {
              throw TypeError("Object iterator must return a key-value pair");
            }
            obj[key = entry[0]] = (dest = obj[key]) ? utils$1.isArray(dest) ? [...dest, entry[1]] : [dest, entry[1]] : entry[1];
          }
          setHeaders(obj, valueOrRewrite);
        } else {
          header != null && setHeader(valueOrRewrite, header, rewrite);
        }
        return this;
      }
      get(header, parser) {
        header = normalizeHeader(header);
        if (header) {
          const key = utils$1.findKey(this, header);
          if (key) {
            const value = this[key];
            if (!parser) {
              return value;
            }
            if (parser === true) {
              return parseTokens(value);
            }
            if (utils$1.isFunction(parser)) {
              return parser.call(this, value, key);
            }
            if (utils$1.isRegExp(parser)) {
              return parser.exec(value);
            }
            throw new TypeError("parser must be boolean|regexp|function");
          }
        }
      }
      has(header, matcher) {
        header = normalizeHeader(header);
        if (header) {
          const key = utils$1.findKey(this, header);
          return !!(key && this[key] !== void 0 && (!matcher || matchHeaderValue(this, this[key], key, matcher)));
        }
        return false;
      }
      delete(header, matcher) {
        const self2 = this;
        let deleted = false;
        function deleteHeader(_header) {
          _header = normalizeHeader(_header);
          if (_header) {
            const key = utils$1.findKey(self2, _header);
            if (key && (!matcher || matchHeaderValue(self2, self2[key], key, matcher))) {
              delete self2[key];
              deleted = true;
            }
          }
        }
        if (utils$1.isArray(header)) {
          header.forEach(deleteHeader);
        } else {
          deleteHeader(header);
        }
        return deleted;
      }
      clear(matcher) {
        const keys = Object.keys(this);
        let i = keys.length;
        let deleted = false;
        while (i--) {
          const key = keys[i];
          if (!matcher || matchHeaderValue(this, this[key], key, matcher, true)) {
            delete this[key];
            deleted = true;
          }
        }
        return deleted;
      }
      normalize(format) {
        const self2 = this;
        const headers = {};
        utils$1.forEach(this, (value, header) => {
          const key = utils$1.findKey(headers, header);
          if (key) {
            self2[key] = normalizeValue(value);
            delete self2[header];
            return;
          }
          const normalized = format ? formatHeader(header) : String(header).trim();
          if (normalized !== header) {
            delete self2[header];
          }
          self2[normalized] = normalizeValue(value);
          headers[normalized] = true;
        });
        return this;
      }
      concat(...targets) {
        return this.constructor.concat(this, ...targets);
      }
      toJSON(asStrings) {
        const obj = /* @__PURE__ */ Object.create(null);
        utils$1.forEach(this, (value, header) => {
          value != null && value !== false && (obj[header] = asStrings && utils$1.isArray(value) ? value.join(", ") : value);
        });
        return obj;
      }
      [Symbol.iterator]() {
        return Object.entries(this.toJSON())[Symbol.iterator]();
      }
      toString() {
        return Object.entries(this.toJSON()).map(([header, value]) => header + ": " + value).join("\n");
      }
      getSetCookie() {
        return this.get("set-cookie") || [];
      }
      get [Symbol.toStringTag]() {
        return "AxiosHeaders";
      }
      static from(thing) {
        return thing instanceof this ? thing : new this(thing);
      }
      static concat(first, ...targets) {
        const computed = new this(first);
        targets.forEach((target) => computed.set(target));
        return computed;
      }
      static accessor(header) {
        const internals = this[$internals] = this[$internals] = {
          accessors: {}
        };
        const accessors = internals.accessors;
        const prototype2 = this.prototype;
        function defineAccessor(_header) {
          const lHeader = normalizeHeader(_header);
          if (!accessors[lHeader]) {
            buildAccessors(prototype2, _header);
            accessors[lHeader] = true;
          }
        }
        utils$1.isArray(header) ? header.forEach(defineAccessor) : defineAccessor(header);
        return this;
      }
    };
    AxiosHeaders.accessor(["Content-Type", "Content-Length", "Accept", "Accept-Encoding", "User-Agent", "Authorization"]);
    utils$1.reduceDescriptors(AxiosHeaders.prototype, ({ value }, key) => {
      let mapped = key[0].toUpperCase() + key.slice(1);
      return {
        get: () => value,
        set(headerValue) {
          this[mapped] = headerValue;
        }
      };
    });
    utils$1.freezeMethods(AxiosHeaders);
    var AxiosHeaders$1 = AxiosHeaders;
    function transformData(fns, response) {
      const config = this || defaults$1;
      const context = response || config;
      const headers = AxiosHeaders$1.from(context.headers);
      let data = context.data;
      utils$1.forEach(fns, function transform(fn) {
        data = fn.call(config, data, headers.normalize(), response ? response.status : void 0);
      });
      headers.normalize();
      return data;
    }
    function isCancel(value) {
      return !!(value && value.__CANCEL__);
    }
    function CanceledError(message, config, request) {
      AxiosError.call(this, message == null ? "canceled" : message, AxiosError.ERR_CANCELED, config, request);
      this.name = "CanceledError";
    }
    utils$1.inherits(CanceledError, AxiosError, {
      __CANCEL__: true
    });
    function settle(resolve, reject, response) {
      const validateStatus = response.config.validateStatus;
      if (!response.status || !validateStatus || validateStatus(response.status)) {
        resolve(response);
      } else {
        reject(new AxiosError(
          "Request failed with status code " + response.status,
          [AxiosError.ERR_BAD_REQUEST, AxiosError.ERR_BAD_RESPONSE][Math.floor(response.status / 100) - 4],
          response.config,
          response.request,
          response
        ));
      }
    }
    function isAbsoluteURL(url2) {
      return /^([a-z][a-z\d+\-.]*:)?\/\//i.test(url2);
    }
    function combineURLs(baseURL, relativeURL) {
      return relativeURL ? baseURL.replace(/\/?\/$/, "") + "/" + relativeURL.replace(/^\/+/, "") : baseURL;
    }
    function buildFullPath(baseURL, requestedURL, allowAbsoluteUrls) {
      let isRelativeUrl = !isAbsoluteURL(requestedURL);
      if (baseURL && (isRelativeUrl || allowAbsoluteUrls == false)) {
        return combineURLs(baseURL, requestedURL);
      }
      return requestedURL;
    }
    var VERSION = "1.13.2";
    function parseProtocol(url2) {
      const match = /^([-+\w]{1,25})(:?\/\/|:)/.exec(url2);
      return match && match[1] || "";
    }
    var DATA_URL_PATTERN = /^(?:([^;]+);)?(?:[^;]+;)?(base64|),([\s\S]*)$/;
    function fromDataURI(uri, asBlob, options) {
      const _Blob = options && options.Blob || platform.classes.Blob;
      const protocol = parseProtocol(uri);
      if (asBlob === void 0 && _Blob) {
        asBlob = true;
      }
      if (protocol === "data") {
        uri = protocol.length ? uri.slice(protocol.length + 1) : uri;
        const match = DATA_URL_PATTERN.exec(uri);
        if (!match) {
          throw new AxiosError("Invalid URL", AxiosError.ERR_INVALID_URL);
        }
        const mime = match[1];
        const isBase64 = match[2];
        const body = match[3];
        const buffer = Buffer.from(decodeURIComponent(body), isBase64 ? "base64" : "utf8");
        if (asBlob) {
          if (!_Blob) {
            throw new AxiosError("Blob is not supported", AxiosError.ERR_NOT_SUPPORT);
          }
          return new _Blob([buffer], { type: mime });
        }
        return buffer;
      }
      throw new AxiosError("Unsupported protocol " + protocol, AxiosError.ERR_NOT_SUPPORT);
    }
    var kInternals = Symbol("internals");
    var AxiosTransformStream = class extends stream__default["default"].Transform {
      constructor(options) {
        options = utils$1.toFlatObject(options, {
          maxRate: 0,
          chunkSize: 64 * 1024,
          minChunkSize: 100,
          timeWindow: 500,
          ticksRate: 2,
          samplesCount: 15
        }, null, (prop, source) => {
          return !utils$1.isUndefined(source[prop]);
        });
        super({
          readableHighWaterMark: options.chunkSize
        });
        const internals = this[kInternals] = {
          timeWindow: options.timeWindow,
          chunkSize: options.chunkSize,
          maxRate: options.maxRate,
          minChunkSize: options.minChunkSize,
          bytesSeen: 0,
          isCaptured: false,
          notifiedBytesLoaded: 0,
          ts: Date.now(),
          bytes: 0,
          onReadCallback: null
        };
        this.on("newListener", (event) => {
          if (event === "progress") {
            if (!internals.isCaptured) {
              internals.isCaptured = true;
            }
          }
        });
      }
      _read(size) {
        const internals = this[kInternals];
        if (internals.onReadCallback) {
          internals.onReadCallback();
        }
        return super._read(size);
      }
      _transform(chunk, encoding, callback) {
        const internals = this[kInternals];
        const maxRate = internals.maxRate;
        const readableHighWaterMark = this.readableHighWaterMark;
        const timeWindow = internals.timeWindow;
        const divider = 1e3 / timeWindow;
        const bytesThreshold = maxRate / divider;
        const minChunkSize = internals.minChunkSize !== false ? Math.max(internals.minChunkSize, bytesThreshold * 0.01) : 0;
        const pushChunk = (_chunk, _callback) => {
          const bytes = Buffer.byteLength(_chunk);
          internals.bytesSeen += bytes;
          internals.bytes += bytes;
          internals.isCaptured && this.emit("progress", internals.bytesSeen);
          if (this.push(_chunk)) {
            process.nextTick(_callback);
          } else {
            internals.onReadCallback = () => {
              internals.onReadCallback = null;
              process.nextTick(_callback);
            };
          }
        };
        const transformChunk = (_chunk, _callback) => {
          const chunkSize = Buffer.byteLength(_chunk);
          let chunkRemainder = null;
          let maxChunkSize = readableHighWaterMark;
          let bytesLeft;
          let passed = 0;
          if (maxRate) {
            const now = Date.now();
            if (!internals.ts || (passed = now - internals.ts) >= timeWindow) {
              internals.ts = now;
              bytesLeft = bytesThreshold - internals.bytes;
              internals.bytes = bytesLeft < 0 ? -bytesLeft : 0;
              passed = 0;
            }
            bytesLeft = bytesThreshold - internals.bytes;
          }
          if (maxRate) {
            if (bytesLeft <= 0) {
              return setTimeout(() => {
                _callback(null, _chunk);
              }, timeWindow - passed);
            }
            if (bytesLeft < maxChunkSize) {
              maxChunkSize = bytesLeft;
            }
          }
          if (maxChunkSize && chunkSize > maxChunkSize && chunkSize - maxChunkSize > minChunkSize) {
            chunkRemainder = _chunk.subarray(maxChunkSize);
            _chunk = _chunk.subarray(0, maxChunkSize);
          }
          pushChunk(_chunk, chunkRemainder ? () => {
            process.nextTick(_callback, null, chunkRemainder);
          } : _callback);
        };
        transformChunk(chunk, function transformNextChunk(err, _chunk) {
          if (err) {
            return callback(err);
          }
          if (_chunk) {
            transformChunk(_chunk, transformNextChunk);
          } else {
            callback(null);
          }
        });
      }
    };
    var AxiosTransformStream$1 = AxiosTransformStream;
    var { asyncIterator } = Symbol;
    var readBlob = async function* (blob) {
      if (blob.stream) {
        yield* blob.stream();
      } else if (blob.arrayBuffer) {
        yield await blob.arrayBuffer();
      } else if (blob[asyncIterator]) {
        yield* blob[asyncIterator]();
      } else {
        yield blob;
      }
    };
    var readBlob$1 = readBlob;
    var BOUNDARY_ALPHABET = platform.ALPHABET.ALPHA_DIGIT + "-_";
    var textEncoder = typeof TextEncoder === "function" ? new TextEncoder() : new util__default["default"].TextEncoder();
    var CRLF = "\r\n";
    var CRLF_BYTES = textEncoder.encode(CRLF);
    var CRLF_BYTES_COUNT = 2;
    var FormDataPart = class {
      constructor(name, value) {
        const { escapeName } = this.constructor;
        const isStringValue = utils$1.isString(value);
        let headers = `Content-Disposition: form-data; name="${escapeName(name)}"${!isStringValue && value.name ? `; filename="${escapeName(value.name)}"` : ""}${CRLF}`;
        if (isStringValue) {
          value = textEncoder.encode(String(value).replace(/\r?\n|\r\n?/g, CRLF));
        } else {
          headers += `Content-Type: ${value.type || "application/octet-stream"}${CRLF}`;
        }
        this.headers = textEncoder.encode(headers + CRLF);
        this.contentLength = isStringValue ? value.byteLength : value.size;
        this.size = this.headers.byteLength + this.contentLength + CRLF_BYTES_COUNT;
        this.name = name;
        this.value = value;
      }
      async *encode() {
        yield this.headers;
        const { value } = this;
        if (utils$1.isTypedArray(value)) {
          yield value;
        } else {
          yield* readBlob$1(value);
        }
        yield CRLF_BYTES;
      }
      static escapeName(name) {
        return String(name).replace(/[\r\n"]/g, (match) => ({
          "\r": "%0D",
          "\n": "%0A",
          '"': "%22"
        })[match]);
      }
    };
    var formDataToStream = (form, headersHandler, options) => {
      const {
        tag = "form-data-boundary",
        size = 25,
        boundary = tag + "-" + platform.generateString(size, BOUNDARY_ALPHABET)
      } = options || {};
      if (!utils$1.isFormData(form)) {
        throw TypeError("FormData instance required");
      }
      if (boundary.length < 1 || boundary.length > 70) {
        throw Error("boundary must be 10-70 characters long");
      }
      const boundaryBytes = textEncoder.encode("--" + boundary + CRLF);
      const footerBytes = textEncoder.encode("--" + boundary + "--" + CRLF);
      let contentLength = footerBytes.byteLength;
      const parts = Array.from(form.entries()).map(([name, value]) => {
        const part = new FormDataPart(name, value);
        contentLength += part.size;
        return part;
      });
      contentLength += boundaryBytes.byteLength * parts.length;
      contentLength = utils$1.toFiniteNumber(contentLength);
      const computedHeaders = {
        "Content-Type": `multipart/form-data; boundary=${boundary}`
      };
      if (Number.isFinite(contentLength)) {
        computedHeaders["Content-Length"] = contentLength;
      }
      headersHandler && headersHandler(computedHeaders);
      return stream.Readable.from(async function* () {
        for (const part of parts) {
          yield boundaryBytes;
          yield* part.encode();
        }
        yield footerBytes;
      }());
    };
    var formDataToStream$1 = formDataToStream;
    var ZlibHeaderTransformStream = class extends stream__default["default"].Transform {
      __transform(chunk, encoding, callback) {
        this.push(chunk);
        callback();
      }
      _transform(chunk, encoding, callback) {
        if (chunk.length !== 0) {
          this._transform = this.__transform;
          if (chunk[0] !== 120) {
            const header = Buffer.alloc(2);
            header[0] = 120;
            header[1] = 156;
            this.push(header, encoding);
          }
        }
        this.__transform(chunk, encoding, callback);
      }
    };
    var ZlibHeaderTransformStream$1 = ZlibHeaderTransformStream;
    var callbackify = (fn, reducer) => {
      return utils$1.isAsyncFn(fn) ? function(...args2) {
        const cb = args2.pop();
        fn.apply(this, args2).then((value) => {
          try {
            reducer ? cb(null, ...reducer(value)) : cb(null, value);
          } catch (err) {
            cb(err);
          }
        }, cb);
      } : fn;
    };
    var callbackify$1 = callbackify;
    function speedometer(samplesCount, min) {
      samplesCount = samplesCount || 10;
      const bytes = new Array(samplesCount);
      const timestamps = new Array(samplesCount);
      let head = 0;
      let tail = 0;
      let firstSampleTS;
      min = min !== void 0 ? min : 1e3;
      return function push(chunkLength) {
        const now = Date.now();
        const startedAt = timestamps[tail];
        if (!firstSampleTS) {
          firstSampleTS = now;
        }
        bytes[head] = chunkLength;
        timestamps[head] = now;
        let i = tail;
        let bytesCount = 0;
        while (i !== head) {
          bytesCount += bytes[i++];
          i = i % samplesCount;
        }
        head = (head + 1) % samplesCount;
        if (head === tail) {
          tail = (tail + 1) % samplesCount;
        }
        if (now - firstSampleTS < min) {
          return;
        }
        const passed = startedAt && now - startedAt;
        return passed ? Math.round(bytesCount * 1e3 / passed) : void 0;
      };
    }
    function throttle(fn, freq) {
      let timestamp = 0;
      let threshold = 1e3 / freq;
      let lastArgs;
      let timer;
      const invoke = (args2, now = Date.now()) => {
        timestamp = now;
        lastArgs = null;
        if (timer) {
          clearTimeout(timer);
          timer = null;
        }
        fn(...args2);
      };
      const throttled = (...args2) => {
        const now = Date.now();
        const passed = now - timestamp;
        if (passed >= threshold) {
          invoke(args2, now);
        } else {
          lastArgs = args2;
          if (!timer) {
            timer = setTimeout(() => {
              timer = null;
              invoke(lastArgs);
            }, threshold - passed);
          }
        }
      };
      const flush = () => lastArgs && invoke(lastArgs);
      return [throttled, flush];
    }
    var progressEventReducer = (listener, isDownloadStream, freq = 3) => {
      let bytesNotified = 0;
      const _speedometer = speedometer(50, 250);
      return throttle((e) => {
        const loaded = e.loaded;
        const total = e.lengthComputable ? e.total : void 0;
        const progressBytes = loaded - bytesNotified;
        const rate = _speedometer(progressBytes);
        const inRange = loaded <= total;
        bytesNotified = loaded;
        const data = {
          loaded,
          total,
          progress: total ? loaded / total : void 0,
          bytes: progressBytes,
          rate: rate ? rate : void 0,
          estimated: rate && total && inRange ? (total - loaded) / rate : void 0,
          event: e,
          lengthComputable: total != null,
          [isDownloadStream ? "download" : "upload"]: true
        };
        listener(data);
      }, freq);
    };
    var progressEventDecorator = (total, throttled) => {
      const lengthComputable = total != null;
      return [(loaded) => throttled[0]({
        lengthComputable,
        total,
        loaded
      }), throttled[1]];
    };
    var asyncDecorator = (fn) => (...args2) => utils$1.asap(() => fn(...args2));
    function estimateDataURLDecodedBytes(url2) {
      if (!url2 || typeof url2 !== "string")
        return 0;
      if (!url2.startsWith("data:"))
        return 0;
      const comma = url2.indexOf(",");
      if (comma < 0)
        return 0;
      const meta = url2.slice(5, comma);
      const body = url2.slice(comma + 1);
      const isBase64 = /;base64/i.test(meta);
      if (isBase64) {
        let effectiveLen = body.length;
        const len = body.length;
        for (let i = 0; i < len; i++) {
          if (body.charCodeAt(i) === 37 && i + 2 < len) {
            const a = body.charCodeAt(i + 1);
            const b = body.charCodeAt(i + 2);
            const isHex = (a >= 48 && a <= 57 || a >= 65 && a <= 70 || a >= 97 && a <= 102) && (b >= 48 && b <= 57 || b >= 65 && b <= 70 || b >= 97 && b <= 102);
            if (isHex) {
              effectiveLen -= 2;
              i += 2;
            }
          }
        }
        let pad = 0;
        let idx = len - 1;
        const tailIsPct3D = (j) => j >= 2 && body.charCodeAt(j - 2) === 37 && // '%'
        body.charCodeAt(j - 1) === 51 && // '3'
        (body.charCodeAt(j) === 68 || body.charCodeAt(j) === 100);
        if (idx >= 0) {
          if (body.charCodeAt(idx) === 61) {
            pad++;
            idx--;
          } else if (tailIsPct3D(idx)) {
            pad++;
            idx -= 3;
          }
        }
        if (pad === 1 && idx >= 0) {
          if (body.charCodeAt(idx) === 61) {
            pad++;
          } else if (tailIsPct3D(idx)) {
            pad++;
          }
        }
        const groups = Math.floor(effectiveLen / 4);
        const bytes = groups * 3 - (pad || 0);
        return bytes > 0 ? bytes : 0;
      }
      return Buffer.byteLength(body, "utf8");
    }
    var zlibOptions = {
      flush: zlib__default["default"].constants.Z_SYNC_FLUSH,
      finishFlush: zlib__default["default"].constants.Z_SYNC_FLUSH
    };
    var brotliOptions = {
      flush: zlib__default["default"].constants.BROTLI_OPERATION_FLUSH,
      finishFlush: zlib__default["default"].constants.BROTLI_OPERATION_FLUSH
    };
    var isBrotliSupported = utils$1.isFunction(zlib__default["default"].createBrotliDecompress);
    var { http: httpFollow, https: httpsFollow } = followRedirects__default["default"];
    var isHttps = /https:?/;
    var supportedProtocols = platform.protocols.map((protocol) => {
      return protocol + ":";
    });
    var flushOnFinish = (stream2, [throttled, flush]) => {
      stream2.on("end", flush).on("error", flush);
      return throttled;
    };
    var Http2Sessions = class {
      constructor() {
        this.sessions = /* @__PURE__ */ Object.create(null);
      }
      getSession(authority, options) {
        options = Object.assign({
          sessionTimeout: 1e3
        }, options);
        let authoritySessions = this.sessions[authority];
        if (authoritySessions) {
          let len = authoritySessions.length;
          for (let i = 0; i < len; i++) {
            const [sessionHandle, sessionOptions] = authoritySessions[i];
            if (!sessionHandle.destroyed && !sessionHandle.closed && util__default["default"].isDeepStrictEqual(sessionOptions, options)) {
              return sessionHandle;
            }
          }
        }
        const session = http2__default["default"].connect(authority, options);
        let removed;
        const removeSession = () => {
          if (removed) {
            return;
          }
          removed = true;
          let entries = authoritySessions, len = entries.length, i = len;
          while (i--) {
            if (entries[i][0] === session) {
              if (len === 1) {
                delete this.sessions[authority];
              } else {
                entries.splice(i, 1);
              }
              return;
            }
          }
        };
        const originalRequestFn = session.request;
        const { sessionTimeout } = options;
        if (sessionTimeout != null) {
          let timer;
          let streamsCount = 0;
          session.request = function() {
            const stream2 = originalRequestFn.apply(this, arguments);
            streamsCount++;
            if (timer) {
              clearTimeout(timer);
              timer = null;
            }
            stream2.once("close", () => {
              if (!--streamsCount) {
                timer = setTimeout(() => {
                  timer = null;
                  removeSession();
                }, sessionTimeout);
              }
            });
            return stream2;
          };
        }
        session.once("close", removeSession);
        let entry = [
          session,
          options
        ];
        authoritySessions ? authoritySessions.push(entry) : authoritySessions = this.sessions[authority] = [entry];
        return session;
      }
    };
    var http2Sessions = new Http2Sessions();
    function dispatchBeforeRedirect(options, responseDetails) {
      if (options.beforeRedirects.proxy) {
        options.beforeRedirects.proxy(options);
      }
      if (options.beforeRedirects.config) {
        options.beforeRedirects.config(options, responseDetails);
      }
    }
    function setProxy(options, configProxy, location) {
      let proxy = configProxy;
      if (!proxy && proxy !== false) {
        const proxyUrl = proxyFromEnv__default["default"].getProxyForUrl(location);
        if (proxyUrl) {
          proxy = new URL(proxyUrl);
        }
      }
      if (proxy) {
        if (proxy.username) {
          proxy.auth = (proxy.username || "") + ":" + (proxy.password || "");
        }
        if (proxy.auth) {
          if (proxy.auth.username || proxy.auth.password) {
            proxy.auth = (proxy.auth.username || "") + ":" + (proxy.auth.password || "");
          }
          const base64 = Buffer.from(proxy.auth, "utf8").toString("base64");
          options.headers["Proxy-Authorization"] = "Basic " + base64;
        }
        options.headers.host = options.hostname + (options.port ? ":" + options.port : "");
        const proxyHost = proxy.hostname || proxy.host;
        options.hostname = proxyHost;
        options.host = proxyHost;
        options.port = proxy.port;
        options.path = location;
        if (proxy.protocol) {
          options.protocol = proxy.protocol.includes(":") ? proxy.protocol : `${proxy.protocol}:`;
        }
      }
      options.beforeRedirects.proxy = function beforeRedirect(redirectOptions) {
        setProxy(redirectOptions, configProxy, redirectOptions.href);
      };
    }
    var isHttpAdapterSupported = typeof process !== "undefined" && utils$1.kindOf(process) === "process";
    var wrapAsync = (asyncExecutor) => {
      return new Promise((resolve, reject) => {
        let onDone;
        let isDone;
        const done = (value, isRejected) => {
          if (isDone)
            return;
          isDone = true;
          onDone && onDone(value, isRejected);
        };
        const _resolve = (value) => {
          done(value);
          resolve(value);
        };
        const _reject = (reason) => {
          done(reason, true);
          reject(reason);
        };
        asyncExecutor(_resolve, _reject, (onDoneHandler) => onDone = onDoneHandler).catch(_reject);
      });
    };
    var resolveFamily = ({ address, family }) => {
      if (!utils$1.isString(address)) {
        throw TypeError("address must be a string");
      }
      return {
        address,
        family: family || (address.indexOf(".") < 0 ? 6 : 4)
      };
    };
    var buildAddressEntry = (address, family) => resolveFamily(utils$1.isObject(address) ? address : { address, family });
    var http2Transport = {
      request(options, cb) {
        const authority = options.protocol + "//" + options.hostname + ":" + (options.port || 80);
        const { http2Options, headers } = options;
        const session = http2Sessions.getSession(authority, http2Options);
        const {
          HTTP2_HEADER_SCHEME,
          HTTP2_HEADER_METHOD,
          HTTP2_HEADER_PATH,
          HTTP2_HEADER_STATUS
        } = http2__default["default"].constants;
        const http2Headers = {
          [HTTP2_HEADER_SCHEME]: options.protocol.replace(":", ""),
          [HTTP2_HEADER_METHOD]: options.method,
          [HTTP2_HEADER_PATH]: options.path
        };
        utils$1.forEach(headers, (header, name) => {
          name.charAt(0) !== ":" && (http2Headers[name] = header);
        });
        const req = session.request(http2Headers);
        req.once("response", (responseHeaders) => {
          const response = req;
          responseHeaders = Object.assign({}, responseHeaders);
          const status = responseHeaders[HTTP2_HEADER_STATUS];
          delete responseHeaders[HTTP2_HEADER_STATUS];
          response.headers = responseHeaders;
          response.statusCode = +status;
          cb(response);
        });
        return req;
      }
    };
    var httpAdapter = isHttpAdapterSupported && function httpAdapter2(config) {
      return wrapAsync(async function dispatchHttpRequest(resolve, reject, onDone) {
        let { data, lookup, family, httpVersion = 1, http2Options } = config;
        const { responseType, responseEncoding } = config;
        const method = config.method.toUpperCase();
        let isDone;
        let rejected = false;
        let req;
        httpVersion = +httpVersion;
        if (Number.isNaN(httpVersion)) {
          throw TypeError(`Invalid protocol version: '${config.httpVersion}' is not a number`);
        }
        if (httpVersion !== 1 && httpVersion !== 2) {
          throw TypeError(`Unsupported protocol version '${httpVersion}'`);
        }
        const isHttp2 = httpVersion === 2;
        if (lookup) {
          const _lookup = callbackify$1(lookup, (value) => utils$1.isArray(value) ? value : [value]);
          lookup = (hostname, opt, cb) => {
            _lookup(hostname, opt, (err, arg0, arg1) => {
              if (err) {
                return cb(err);
              }
              const addresses = utils$1.isArray(arg0) ? arg0.map((addr) => buildAddressEntry(addr)) : [buildAddressEntry(arg0, arg1)];
              opt.all ? cb(err, addresses) : cb(err, addresses[0].address, addresses[0].family);
            });
          };
        }
        const abortEmitter = new events.EventEmitter();
        function abort(reason) {
          try {
            abortEmitter.emit("abort", !reason || reason.type ? new CanceledError(null, config, req) : reason);
          } catch (err) {
            console.warn("emit error", err);
          }
        }
        abortEmitter.once("abort", reject);
        const onFinished = () => {
          if (config.cancelToken) {
            config.cancelToken.unsubscribe(abort);
          }
          if (config.signal) {
            config.signal.removeEventListener("abort", abort);
          }
          abortEmitter.removeAllListeners();
        };
        if (config.cancelToken || config.signal) {
          config.cancelToken && config.cancelToken.subscribe(abort);
          if (config.signal) {
            config.signal.aborted ? abort() : config.signal.addEventListener("abort", abort);
          }
        }
        onDone((response, isRejected) => {
          isDone = true;
          if (isRejected) {
            rejected = true;
            onFinished();
            return;
          }
          const { data: data2 } = response;
          if (data2 instanceof stream__default["default"].Readable || data2 instanceof stream__default["default"].Duplex) {
            const offListeners = stream__default["default"].finished(data2, () => {
              offListeners();
              onFinished();
            });
          } else {
            onFinished();
          }
        });
        const fullPath = buildFullPath(config.baseURL, config.url, config.allowAbsoluteUrls);
        const parsed = new URL(fullPath, platform.hasBrowserEnv ? platform.origin : void 0);
        const protocol = parsed.protocol || supportedProtocols[0];
        if (protocol === "data:") {
          if (config.maxContentLength > -1) {
            const dataUrl = String(config.url || fullPath || "");
            const estimated = estimateDataURLDecodedBytes(dataUrl);
            if (estimated > config.maxContentLength) {
              return reject(new AxiosError(
                "maxContentLength size of " + config.maxContentLength + " exceeded",
                AxiosError.ERR_BAD_RESPONSE,
                config
              ));
            }
          }
          let convertedData;
          if (method !== "GET") {
            return settle(resolve, reject, {
              status: 405,
              statusText: "method not allowed",
              headers: {},
              config
            });
          }
          try {
            convertedData = fromDataURI(config.url, responseType === "blob", {
              Blob: config.env && config.env.Blob
            });
          } catch (err) {
            throw AxiosError.from(err, AxiosError.ERR_BAD_REQUEST, config);
          }
          if (responseType === "text") {
            convertedData = convertedData.toString(responseEncoding);
            if (!responseEncoding || responseEncoding === "utf8") {
              convertedData = utils$1.stripBOM(convertedData);
            }
          } else if (responseType === "stream") {
            convertedData = stream__default["default"].Readable.from(convertedData);
          }
          return settle(resolve, reject, {
            data: convertedData,
            status: 200,
            statusText: "OK",
            headers: new AxiosHeaders$1(),
            config
          });
        }
        if (supportedProtocols.indexOf(protocol) === -1) {
          return reject(new AxiosError(
            "Unsupported protocol " + protocol,
            AxiosError.ERR_BAD_REQUEST,
            config
          ));
        }
        const headers = AxiosHeaders$1.from(config.headers).normalize();
        headers.set("User-Agent", "axios/" + VERSION, false);
        const { onUploadProgress, onDownloadProgress } = config;
        const maxRate = config.maxRate;
        let maxUploadRate = void 0;
        let maxDownloadRate = void 0;
        if (utils$1.isSpecCompliantForm(data)) {
          const userBoundary = headers.getContentType(/boundary=([-_\w\d]{10,70})/i);
          data = formDataToStream$1(data, (formHeaders) => {
            headers.set(formHeaders);
          }, {
            tag: `axios-${VERSION}-boundary`,
            boundary: userBoundary && userBoundary[1] || void 0
          });
        } else if (utils$1.isFormData(data) && utils$1.isFunction(data.getHeaders)) {
          headers.set(data.getHeaders());
          if (!headers.hasContentLength()) {
            try {
              const knownLength = await util__default["default"].promisify(data.getLength).call(data);
              Number.isFinite(knownLength) && knownLength >= 0 && headers.setContentLength(knownLength);
            } catch (e) {
            }
          }
        } else if (utils$1.isBlob(data) || utils$1.isFile(data)) {
          data.size && headers.setContentType(data.type || "application/octet-stream");
          headers.setContentLength(data.size || 0);
          data = stream__default["default"].Readable.from(readBlob$1(data));
        } else if (data && !utils$1.isStream(data)) {
          if (Buffer.isBuffer(data))
            ;
          else if (utils$1.isArrayBuffer(data)) {
            data = Buffer.from(new Uint8Array(data));
          } else if (utils$1.isString(data)) {
            data = Buffer.from(data, "utf-8");
          } else {
            return reject(new AxiosError(
              "Data after transformation must be a string, an ArrayBuffer, a Buffer, or a Stream",
              AxiosError.ERR_BAD_REQUEST,
              config
            ));
          }
          headers.setContentLength(data.length, false);
          if (config.maxBodyLength > -1 && data.length > config.maxBodyLength) {
            return reject(new AxiosError(
              "Request body larger than maxBodyLength limit",
              AxiosError.ERR_BAD_REQUEST,
              config
            ));
          }
        }
        const contentLength = utils$1.toFiniteNumber(headers.getContentLength());
        if (utils$1.isArray(maxRate)) {
          maxUploadRate = maxRate[0];
          maxDownloadRate = maxRate[1];
        } else {
          maxUploadRate = maxDownloadRate = maxRate;
        }
        if (data && (onUploadProgress || maxUploadRate)) {
          if (!utils$1.isStream(data)) {
            data = stream__default["default"].Readable.from(data, { objectMode: false });
          }
          data = stream__default["default"].pipeline([data, new AxiosTransformStream$1({
            maxRate: utils$1.toFiniteNumber(maxUploadRate)
          })], utils$1.noop);
          onUploadProgress && data.on("progress", flushOnFinish(
            data,
            progressEventDecorator(
              contentLength,
              progressEventReducer(asyncDecorator(onUploadProgress), false, 3)
            )
          ));
        }
        let auth = void 0;
        if (config.auth) {
          const username = config.auth.username || "";
          const password = config.auth.password || "";
          auth = username + ":" + password;
        }
        if (!auth && parsed.username) {
          const urlUsername = parsed.username;
          const urlPassword = parsed.password;
          auth = urlUsername + ":" + urlPassword;
        }
        auth && headers.delete("authorization");
        let path;
        try {
          path = buildURL(
            parsed.pathname + parsed.search,
            config.params,
            config.paramsSerializer
          ).replace(/^\?/, "");
        } catch (err) {
          const customErr = new Error(err.message);
          customErr.config = config;
          customErr.url = config.url;
          customErr.exists = true;
          return reject(customErr);
        }
        headers.set(
          "Accept-Encoding",
          "gzip, compress, deflate" + (isBrotliSupported ? ", br" : ""),
          false
        );
        const options = {
          path,
          method,
          headers: headers.toJSON(),
          agents: { http: config.httpAgent, https: config.httpsAgent },
          auth,
          protocol,
          family,
          beforeRedirect: dispatchBeforeRedirect,
          beforeRedirects: {},
          http2Options
        };
        !utils$1.isUndefined(lookup) && (options.lookup = lookup);
        if (config.socketPath) {
          options.socketPath = config.socketPath;
        } else {
          options.hostname = parsed.hostname.startsWith("[") ? parsed.hostname.slice(1, -1) : parsed.hostname;
          options.port = parsed.port;
          setProxy(options, config.proxy, protocol + "//" + parsed.hostname + (parsed.port ? ":" + parsed.port : "") + options.path);
        }
        let transport;
        const isHttpsRequest = isHttps.test(options.protocol);
        options.agent = isHttpsRequest ? config.httpsAgent : config.httpAgent;
        if (isHttp2) {
          transport = http2Transport;
        } else {
          if (config.transport) {
            transport = config.transport;
          } else if (config.maxRedirects === 0) {
            transport = isHttpsRequest ? https__default["default"] : http__default["default"];
          } else {
            if (config.maxRedirects) {
              options.maxRedirects = config.maxRedirects;
            }
            if (config.beforeRedirect) {
              options.beforeRedirects.config = config.beforeRedirect;
            }
            transport = isHttpsRequest ? httpsFollow : httpFollow;
          }
        }
        if (config.maxBodyLength > -1) {
          options.maxBodyLength = config.maxBodyLength;
        } else {
          options.maxBodyLength = Infinity;
        }
        if (config.insecureHTTPParser) {
          options.insecureHTTPParser = config.insecureHTTPParser;
        }
        req = transport.request(options, function handleResponse(res) {
          if (req.destroyed)
            return;
          const streams = [res];
          const responseLength = utils$1.toFiniteNumber(res.headers["content-length"]);
          if (onDownloadProgress || maxDownloadRate) {
            const transformStream = new AxiosTransformStream$1({
              maxRate: utils$1.toFiniteNumber(maxDownloadRate)
            });
            onDownloadProgress && transformStream.on("progress", flushOnFinish(
              transformStream,
              progressEventDecorator(
                responseLength,
                progressEventReducer(asyncDecorator(onDownloadProgress), true, 3)
              )
            ));
            streams.push(transformStream);
          }
          let responseStream = res;
          const lastRequest = res.req || req;
          if (config.decompress !== false && res.headers["content-encoding"]) {
            if (method === "HEAD" || res.statusCode === 204) {
              delete res.headers["content-encoding"];
            }
            switch ((res.headers["content-encoding"] || "").toLowerCase()) {
              case "gzip":
              case "x-gzip":
              case "compress":
              case "x-compress":
                streams.push(zlib__default["default"].createUnzip(zlibOptions));
                delete res.headers["content-encoding"];
                break;
              case "deflate":
                streams.push(new ZlibHeaderTransformStream$1());
                streams.push(zlib__default["default"].createUnzip(zlibOptions));
                delete res.headers["content-encoding"];
                break;
              case "br":
                if (isBrotliSupported) {
                  streams.push(zlib__default["default"].createBrotliDecompress(brotliOptions));
                  delete res.headers["content-encoding"];
                }
            }
          }
          responseStream = streams.length > 1 ? stream__default["default"].pipeline(streams, utils$1.noop) : streams[0];
          const response = {
            status: res.statusCode,
            statusText: res.statusMessage,
            headers: new AxiosHeaders$1(res.headers),
            config,
            request: lastRequest
          };
          if (responseType === "stream") {
            response.data = responseStream;
            settle(resolve, reject, response);
          } else {
            const responseBuffer = [];
            let totalResponseBytes = 0;
            responseStream.on("data", function handleStreamData(chunk) {
              responseBuffer.push(chunk);
              totalResponseBytes += chunk.length;
              if (config.maxContentLength > -1 && totalResponseBytes > config.maxContentLength) {
                rejected = true;
                responseStream.destroy();
                abort(new AxiosError(
                  "maxContentLength size of " + config.maxContentLength + " exceeded",
                  AxiosError.ERR_BAD_RESPONSE,
                  config,
                  lastRequest
                ));
              }
            });
            responseStream.on("aborted", function handlerStreamAborted() {
              if (rejected) {
                return;
              }
              const err = new AxiosError(
                "stream has been aborted",
                AxiosError.ERR_BAD_RESPONSE,
                config,
                lastRequest
              );
              responseStream.destroy(err);
              reject(err);
            });
            responseStream.on("error", function handleStreamError(err) {
              if (req.destroyed)
                return;
              reject(AxiosError.from(err, null, config, lastRequest));
            });
            responseStream.on("end", function handleStreamEnd() {
              try {
                let responseData = responseBuffer.length === 1 ? responseBuffer[0] : Buffer.concat(responseBuffer);
                if (responseType !== "arraybuffer") {
                  responseData = responseData.toString(responseEncoding);
                  if (!responseEncoding || responseEncoding === "utf8") {
                    responseData = utils$1.stripBOM(responseData);
                  }
                }
                response.data = responseData;
              } catch (err) {
                return reject(AxiosError.from(err, null, config, response.request, response));
              }
              settle(resolve, reject, response);
            });
          }
          abortEmitter.once("abort", (err) => {
            if (!responseStream.destroyed) {
              responseStream.emit("error", err);
              responseStream.destroy();
            }
          });
        });
        abortEmitter.once("abort", (err) => {
          if (req.close) {
            req.close();
          } else {
            req.destroy(err);
          }
        });
        req.on("error", function handleRequestError(err) {
          reject(AxiosError.from(err, null, config, req));
        });
        req.on("socket", function handleRequestSocket(socket) {
          socket.setKeepAlive(true, 1e3 * 60);
        });
        if (config.timeout) {
          const timeout = parseInt(config.timeout, 10);
          if (Number.isNaN(timeout)) {
            abort(new AxiosError(
              "error trying to parse `config.timeout` to int",
              AxiosError.ERR_BAD_OPTION_VALUE,
              config,
              req
            ));
            return;
          }
          req.setTimeout(timeout, function handleRequestTimeout() {
            if (isDone)
              return;
            let timeoutErrorMessage = config.timeout ? "timeout of " + config.timeout + "ms exceeded" : "timeout exceeded";
            const transitional = config.transitional || transitionalDefaults;
            if (config.timeoutErrorMessage) {
              timeoutErrorMessage = config.timeoutErrorMessage;
            }
            abort(new AxiosError(
              timeoutErrorMessage,
              transitional.clarifyTimeoutError ? AxiosError.ETIMEDOUT : AxiosError.ECONNABORTED,
              config,
              req
            ));
          });
        } else {
          req.setTimeout(0);
        }
        if (utils$1.isStream(data)) {
          let ended = false;
          let errored = false;
          data.on("end", () => {
            ended = true;
          });
          data.once("error", (err) => {
            errored = true;
            req.destroy(err);
          });
          data.on("close", () => {
            if (!ended && !errored) {
              abort(new CanceledError("Request stream has been aborted", config, req));
            }
          });
          data.pipe(req);
        } else {
          data && req.write(data);
          req.end();
        }
      });
    };
    var isURLSameOrigin = platform.hasStandardBrowserEnv ? /* @__PURE__ */ ((origin2, isMSIE) => (url2) => {
      url2 = new URL(url2, platform.origin);
      return origin2.protocol === url2.protocol && origin2.host === url2.host && (isMSIE || origin2.port === url2.port);
    })(
      new URL(platform.origin),
      platform.navigator && /(msie|trident)/i.test(platform.navigator.userAgent)
    ) : () => true;
    var cookies = platform.hasStandardBrowserEnv ? (
      // Standard browser envs support document.cookie
      {
        write(name, value, expires, path, domain, secure, sameSite) {
          if (typeof document === "undefined")
            return;
          const cookie = [`${name}=${encodeURIComponent(value)}`];
          if (utils$1.isNumber(expires)) {
            cookie.push(`expires=${new Date(expires).toUTCString()}`);
          }
          if (utils$1.isString(path)) {
            cookie.push(`path=${path}`);
          }
          if (utils$1.isString(domain)) {
            cookie.push(`domain=${domain}`);
          }
          if (secure === true) {
            cookie.push("secure");
          }
          if (utils$1.isString(sameSite)) {
            cookie.push(`SameSite=${sameSite}`);
          }
          document.cookie = cookie.join("; ");
        },
        read(name) {
          if (typeof document === "undefined")
            return null;
          const match = document.cookie.match(new RegExp("(?:^|; )" + name + "=([^;]*)"));
          return match ? decodeURIComponent(match[1]) : null;
        },
        remove(name) {
          this.write(name, "", Date.now() - 864e5, "/");
        }
      }
    ) : (
      // Non-standard browser env (web workers, react-native) lack needed support.
      {
        write() {
        },
        read() {
          return null;
        },
        remove() {
        }
      }
    );
    var headersToObject = (thing) => thing instanceof AxiosHeaders$1 ? { ...thing } : thing;
    function mergeConfig(config1, config2) {
      config2 = config2 || {};
      const config = {};
      function getMergedValue(target, source, prop, caseless) {
        if (utils$1.isPlainObject(target) && utils$1.isPlainObject(source)) {
          return utils$1.merge.call({ caseless }, target, source);
        } else if (utils$1.isPlainObject(source)) {
          return utils$1.merge({}, source);
        } else if (utils$1.isArray(source)) {
          return source.slice();
        }
        return source;
      }
      function mergeDeepProperties(a, b, prop, caseless) {
        if (!utils$1.isUndefined(b)) {
          return getMergedValue(a, b, prop, caseless);
        } else if (!utils$1.isUndefined(a)) {
          return getMergedValue(void 0, a, prop, caseless);
        }
      }
      function valueFromConfig2(a, b) {
        if (!utils$1.isUndefined(b)) {
          return getMergedValue(void 0, b);
        }
      }
      function defaultToConfig2(a, b) {
        if (!utils$1.isUndefined(b)) {
          return getMergedValue(void 0, b);
        } else if (!utils$1.isUndefined(a)) {
          return getMergedValue(void 0, a);
        }
      }
      function mergeDirectKeys(a, b, prop) {
        if (prop in config2) {
          return getMergedValue(a, b);
        } else if (prop in config1) {
          return getMergedValue(void 0, a);
        }
      }
      const mergeMap = {
        url: valueFromConfig2,
        method: valueFromConfig2,
        data: valueFromConfig2,
        baseURL: defaultToConfig2,
        transformRequest: defaultToConfig2,
        transformResponse: defaultToConfig2,
        paramsSerializer: defaultToConfig2,
        timeout: defaultToConfig2,
        timeoutMessage: defaultToConfig2,
        withCredentials: defaultToConfig2,
        withXSRFToken: defaultToConfig2,
        adapter: defaultToConfig2,
        responseType: defaultToConfig2,
        xsrfCookieName: defaultToConfig2,
        xsrfHeaderName: defaultToConfig2,
        onUploadProgress: defaultToConfig2,
        onDownloadProgress: defaultToConfig2,
        decompress: defaultToConfig2,
        maxContentLength: defaultToConfig2,
        maxBodyLength: defaultToConfig2,
        beforeRedirect: defaultToConfig2,
        transport: defaultToConfig2,
        httpAgent: defaultToConfig2,
        httpsAgent: defaultToConfig2,
        cancelToken: defaultToConfig2,
        socketPath: defaultToConfig2,
        responseEncoding: defaultToConfig2,
        validateStatus: mergeDirectKeys,
        headers: (a, b, prop) => mergeDeepProperties(headersToObject(a), headersToObject(b), prop, true)
      };
      utils$1.forEach(Object.keys({ ...config1, ...config2 }), function computeConfigValue(prop) {
        const merge2 = mergeMap[prop] || mergeDeepProperties;
        const configValue = merge2(config1[prop], config2[prop], prop);
        utils$1.isUndefined(configValue) && merge2 !== mergeDirectKeys || (config[prop] = configValue);
      });
      return config;
    }
    var resolveConfig = (config) => {
      const newConfig = mergeConfig({}, config);
      let { data, withXSRFToken, xsrfHeaderName, xsrfCookieName, headers, auth } = newConfig;
      newConfig.headers = headers = AxiosHeaders$1.from(headers);
      newConfig.url = buildURL(buildFullPath(newConfig.baseURL, newConfig.url, newConfig.allowAbsoluteUrls), config.params, config.paramsSerializer);
      if (auth) {
        headers.set(
          "Authorization",
          "Basic " + btoa((auth.username || "") + ":" + (auth.password ? unescape(encodeURIComponent(auth.password)) : ""))
        );
      }
      if (utils$1.isFormData(data)) {
        if (platform.hasStandardBrowserEnv || platform.hasStandardBrowserWebWorkerEnv) {
          headers.setContentType(void 0);
        } else if (utils$1.isFunction(data.getHeaders)) {
          const formHeaders = data.getHeaders();
          const allowedHeaders = ["content-type", "content-length"];
          Object.entries(formHeaders).forEach(([key, val]) => {
            if (allowedHeaders.includes(key.toLowerCase())) {
              headers.set(key, val);
            }
          });
        }
      }
      if (platform.hasStandardBrowserEnv) {
        withXSRFToken && utils$1.isFunction(withXSRFToken) && (withXSRFToken = withXSRFToken(newConfig));
        if (withXSRFToken || withXSRFToken !== false && isURLSameOrigin(newConfig.url)) {
          const xsrfValue = xsrfHeaderName && xsrfCookieName && cookies.read(xsrfCookieName);
          if (xsrfValue) {
            headers.set(xsrfHeaderName, xsrfValue);
          }
        }
      }
      return newConfig;
    };
    var isXHRAdapterSupported = typeof XMLHttpRequest !== "undefined";
    var xhrAdapter = isXHRAdapterSupported && function(config) {
      return new Promise(function dispatchXhrRequest(resolve, reject) {
        const _config = resolveConfig(config);
        let requestData = _config.data;
        const requestHeaders = AxiosHeaders$1.from(_config.headers).normalize();
        let { responseType, onUploadProgress, onDownloadProgress } = _config;
        let onCanceled;
        let uploadThrottled, downloadThrottled;
        let flushUpload, flushDownload;
        function done() {
          flushUpload && flushUpload();
          flushDownload && flushDownload();
          _config.cancelToken && _config.cancelToken.unsubscribe(onCanceled);
          _config.signal && _config.signal.removeEventListener("abort", onCanceled);
        }
        let request = new XMLHttpRequest();
        request.open(_config.method.toUpperCase(), _config.url, true);
        request.timeout = _config.timeout;
        function onloadend() {
          if (!request) {
            return;
          }
          const responseHeaders = AxiosHeaders$1.from(
            "getAllResponseHeaders" in request && request.getAllResponseHeaders()
          );
          const responseData = !responseType || responseType === "text" || responseType === "json" ? request.responseText : request.response;
          const response = {
            data: responseData,
            status: request.status,
            statusText: request.statusText,
            headers: responseHeaders,
            config,
            request
          };
          settle(function _resolve(value) {
            resolve(value);
            done();
          }, function _reject(err) {
            reject(err);
            done();
          }, response);
          request = null;
        }
        if ("onloadend" in request) {
          request.onloadend = onloadend;
        } else {
          request.onreadystatechange = function handleLoad() {
            if (!request || request.readyState !== 4) {
              return;
            }
            if (request.status === 0 && !(request.responseURL && request.responseURL.indexOf("file:") === 0)) {
              return;
            }
            setTimeout(onloadend);
          };
        }
        request.onabort = function handleAbort() {
          if (!request) {
            return;
          }
          reject(new AxiosError("Request aborted", AxiosError.ECONNABORTED, config, request));
          request = null;
        };
        request.onerror = function handleError(event) {
          const msg = event && event.message ? event.message : "Network Error";
          const err = new AxiosError(msg, AxiosError.ERR_NETWORK, config, request);
          err.event = event || null;
          reject(err);
          request = null;
        };
        request.ontimeout = function handleTimeout() {
          let timeoutErrorMessage = _config.timeout ? "timeout of " + _config.timeout + "ms exceeded" : "timeout exceeded";
          const transitional = _config.transitional || transitionalDefaults;
          if (_config.timeoutErrorMessage) {
            timeoutErrorMessage = _config.timeoutErrorMessage;
          }
          reject(new AxiosError(
            timeoutErrorMessage,
            transitional.clarifyTimeoutError ? AxiosError.ETIMEDOUT : AxiosError.ECONNABORTED,
            config,
            request
          ));
          request = null;
        };
        requestData === void 0 && requestHeaders.setContentType(null);
        if ("setRequestHeader" in request) {
          utils$1.forEach(requestHeaders.toJSON(), function setRequestHeader(val, key) {
            request.setRequestHeader(key, val);
          });
        }
        if (!utils$1.isUndefined(_config.withCredentials)) {
          request.withCredentials = !!_config.withCredentials;
        }
        if (responseType && responseType !== "json") {
          request.responseType = _config.responseType;
        }
        if (onDownloadProgress) {
          [downloadThrottled, flushDownload] = progressEventReducer(onDownloadProgress, true);
          request.addEventListener("progress", downloadThrottled);
        }
        if (onUploadProgress && request.upload) {
          [uploadThrottled, flushUpload] = progressEventReducer(onUploadProgress);
          request.upload.addEventListener("progress", uploadThrottled);
          request.upload.addEventListener("loadend", flushUpload);
        }
        if (_config.cancelToken || _config.signal) {
          onCanceled = (cancel) => {
            if (!request) {
              return;
            }
            reject(!cancel || cancel.type ? new CanceledError(null, config, request) : cancel);
            request.abort();
            request = null;
          };
          _config.cancelToken && _config.cancelToken.subscribe(onCanceled);
          if (_config.signal) {
            _config.signal.aborted ? onCanceled() : _config.signal.addEventListener("abort", onCanceled);
          }
        }
        const protocol = parseProtocol(_config.url);
        if (protocol && platform.protocols.indexOf(protocol) === -1) {
          reject(new AxiosError("Unsupported protocol " + protocol + ":", AxiosError.ERR_BAD_REQUEST, config));
          return;
        }
        request.send(requestData || null);
      });
    };
    var composeSignals = (signals, timeout) => {
      const { length } = signals = signals ? signals.filter(Boolean) : [];
      if (timeout || length) {
        let controller = new AbortController();
        let aborted;
        const onabort = function(reason) {
          if (!aborted) {
            aborted = true;
            unsubscribe();
            const err = reason instanceof Error ? reason : this.reason;
            controller.abort(err instanceof AxiosError ? err : new CanceledError(err instanceof Error ? err.message : err));
          }
        };
        let timer = timeout && setTimeout(() => {
          timer = null;
          onabort(new AxiosError(`timeout ${timeout} of ms exceeded`, AxiosError.ETIMEDOUT));
        }, timeout);
        const unsubscribe = () => {
          if (signals) {
            timer && clearTimeout(timer);
            timer = null;
            signals.forEach((signal2) => {
              signal2.unsubscribe ? signal2.unsubscribe(onabort) : signal2.removeEventListener("abort", onabort);
            });
            signals = null;
          }
        };
        signals.forEach((signal2) => signal2.addEventListener("abort", onabort));
        const { signal } = controller;
        signal.unsubscribe = () => utils$1.asap(unsubscribe);
        return signal;
      }
    };
    var composeSignals$1 = composeSignals;
    var streamChunk = function* (chunk, chunkSize) {
      let len = chunk.byteLength;
      if (!chunkSize || len < chunkSize) {
        yield chunk;
        return;
      }
      let pos = 0;
      let end;
      while (pos < len) {
        end = pos + chunkSize;
        yield chunk.slice(pos, end);
        pos = end;
      }
    };
    var readBytes = async function* (iterable, chunkSize) {
      for await (const chunk of readStream(iterable)) {
        yield* streamChunk(chunk, chunkSize);
      }
    };
    var readStream = async function* (stream2) {
      if (stream2[Symbol.asyncIterator]) {
        yield* stream2;
        return;
      }
      const reader = stream2.getReader();
      try {
        for (; ; ) {
          const { done, value } = await reader.read();
          if (done) {
            break;
          }
          yield value;
        }
      } finally {
        await reader.cancel();
      }
    };
    var trackStream = (stream2, chunkSize, onProgress, onFinish) => {
      const iterator2 = readBytes(stream2, chunkSize);
      let bytes = 0;
      let done;
      let _onFinish = (e) => {
        if (!done) {
          done = true;
          onFinish && onFinish(e);
        }
      };
      return new ReadableStream({
        async pull(controller) {
          try {
            const { done: done2, value } = await iterator2.next();
            if (done2) {
              _onFinish();
              controller.close();
              return;
            }
            let len = value.byteLength;
            if (onProgress) {
              let loadedBytes = bytes += len;
              onProgress(loadedBytes);
            }
            controller.enqueue(new Uint8Array(value));
          } catch (err) {
            _onFinish(err);
            throw err;
          }
        },
        cancel(reason) {
          _onFinish(reason);
          return iterator2.return();
        }
      }, {
        highWaterMark: 2
      });
    };
    var DEFAULT_CHUNK_SIZE = 64 * 1024;
    var { isFunction } = utils$1;
    var globalFetchAPI = (({ Request, Response }) => ({
      Request,
      Response
    }))(utils$1.global);
    var {
      ReadableStream: ReadableStream$1,
      TextEncoder: TextEncoder$1
    } = utils$1.global;
    var test = (fn, ...args2) => {
      try {
        return !!fn(...args2);
      } catch (e) {
        return false;
      }
    };
    var factory = (env) => {
      env = utils$1.merge.call({
        skipUndefined: true
      }, globalFetchAPI, env);
      const { fetch: envFetch, Request, Response } = env;
      const isFetchSupported = envFetch ? isFunction(envFetch) : typeof fetch === "function";
      const isRequestSupported = isFunction(Request);
      const isResponseSupported = isFunction(Response);
      if (!isFetchSupported) {
        return false;
      }
      const isReadableStreamSupported = isFetchSupported && isFunction(ReadableStream$1);
      const encodeText = isFetchSupported && (typeof TextEncoder$1 === "function" ? /* @__PURE__ */ ((encoder) => (str2) => encoder.encode(str2))(new TextEncoder$1()) : async (str2) => new Uint8Array(await new Request(str2).arrayBuffer()));
      const supportsRequestStream = isRequestSupported && isReadableStreamSupported && test(() => {
        let duplexAccessed = false;
        const hasContentType = new Request(platform.origin, {
          body: new ReadableStream$1(),
          method: "POST",
          get duplex() {
            duplexAccessed = true;
            return "half";
          }
        }).headers.has("Content-Type");
        return duplexAccessed && !hasContentType;
      });
      const supportsResponseStream = isResponseSupported && isReadableStreamSupported && test(() => utils$1.isReadableStream(new Response("").body));
      const resolvers = {
        stream: supportsResponseStream && ((res) => res.body)
      };
      isFetchSupported && (() => {
        ["text", "arrayBuffer", "blob", "formData", "stream"].forEach((type) => {
          !resolvers[type] && (resolvers[type] = (res, config) => {
            let method = res && res[type];
            if (method) {
              return method.call(res);
            }
            throw new AxiosError(`Response type '${type}' is not supported`, AxiosError.ERR_NOT_SUPPORT, config);
          });
        });
      })();
      const getBodyLength = async (body) => {
        if (body == null) {
          return 0;
        }
        if (utils$1.isBlob(body)) {
          return body.size;
        }
        if (utils$1.isSpecCompliantForm(body)) {
          const _request = new Request(platform.origin, {
            method: "POST",
            body
          });
          return (await _request.arrayBuffer()).byteLength;
        }
        if (utils$1.isArrayBufferView(body) || utils$1.isArrayBuffer(body)) {
          return body.byteLength;
        }
        if (utils$1.isURLSearchParams(body)) {
          body = body + "";
        }
        if (utils$1.isString(body)) {
          return (await encodeText(body)).byteLength;
        }
      };
      const resolveBodyLength = async (headers, body) => {
        const length = utils$1.toFiniteNumber(headers.getContentLength());
        return length == null ? getBodyLength(body) : length;
      };
      return async (config) => {
        let {
          url: url2,
          method,
          data,
          signal,
          cancelToken,
          timeout,
          onDownloadProgress,
          onUploadProgress,
          responseType,
          headers,
          withCredentials = "same-origin",
          fetchOptions
        } = resolveConfig(config);
        let _fetch = envFetch || fetch;
        responseType = responseType ? (responseType + "").toLowerCase() : "text";
        let composedSignal = composeSignals$1([signal, cancelToken && cancelToken.toAbortSignal()], timeout);
        let request = null;
        const unsubscribe = composedSignal && composedSignal.unsubscribe && (() => {
          composedSignal.unsubscribe();
        });
        let requestContentLength;
        try {
          if (onUploadProgress && supportsRequestStream && method !== "get" && method !== "head" && (requestContentLength = await resolveBodyLength(headers, data)) !== 0) {
            let _request = new Request(url2, {
              method: "POST",
              body: data,
              duplex: "half"
            });
            let contentTypeHeader;
            if (utils$1.isFormData(data) && (contentTypeHeader = _request.headers.get("content-type"))) {
              headers.setContentType(contentTypeHeader);
            }
            if (_request.body) {
              const [onProgress, flush] = progressEventDecorator(
                requestContentLength,
                progressEventReducer(asyncDecorator(onUploadProgress))
              );
              data = trackStream(_request.body, DEFAULT_CHUNK_SIZE, onProgress, flush);
            }
          }
          if (!utils$1.isString(withCredentials)) {
            withCredentials = withCredentials ? "include" : "omit";
          }
          const isCredentialsSupported = isRequestSupported && "credentials" in Request.prototype;
          const resolvedOptions = {
            ...fetchOptions,
            signal: composedSignal,
            method: method.toUpperCase(),
            headers: headers.normalize().toJSON(),
            body: data,
            duplex: "half",
            credentials: isCredentialsSupported ? withCredentials : void 0
          };
          request = isRequestSupported && new Request(url2, resolvedOptions);
          let response = await (isRequestSupported ? _fetch(request, fetchOptions) : _fetch(url2, resolvedOptions));
          const isStreamResponse = supportsResponseStream && (responseType === "stream" || responseType === "response");
          if (supportsResponseStream && (onDownloadProgress || isStreamResponse && unsubscribe)) {
            const options = {};
            ["status", "statusText", "headers"].forEach((prop) => {
              options[prop] = response[prop];
            });
            const responseContentLength = utils$1.toFiniteNumber(response.headers.get("content-length"));
            const [onProgress, flush] = onDownloadProgress && progressEventDecorator(
              responseContentLength,
              progressEventReducer(asyncDecorator(onDownloadProgress), true)
            ) || [];
            response = new Response(
              trackStream(response.body, DEFAULT_CHUNK_SIZE, onProgress, () => {
                flush && flush();
                unsubscribe && unsubscribe();
              }),
              options
            );
          }
          responseType = responseType || "text";
          let responseData = await resolvers[utils$1.findKey(resolvers, responseType) || "text"](response, config);
          !isStreamResponse && unsubscribe && unsubscribe();
          return await new Promise((resolve, reject) => {
            settle(resolve, reject, {
              data: responseData,
              headers: AxiosHeaders$1.from(response.headers),
              status: response.status,
              statusText: response.statusText,
              config,
              request
            });
          });
        } catch (err) {
          unsubscribe && unsubscribe();
          if (err && err.name === "TypeError" && /Load failed|fetch/i.test(err.message)) {
            throw Object.assign(
              new AxiosError("Network Error", AxiosError.ERR_NETWORK, config, request),
              {
                cause: err.cause || err
              }
            );
          }
          throw AxiosError.from(err, err && err.code, config, request);
        }
      };
    };
    var seedCache = /* @__PURE__ */ new Map();
    var getFetch = (config) => {
      let env = config && config.env || {};
      const { fetch: fetch2, Request, Response } = env;
      const seeds = [
        Request,
        Response,
        fetch2
      ];
      let len = seeds.length, i = len, seed, target, map = seedCache;
      while (i--) {
        seed = seeds[i];
        target = map.get(seed);
        target === void 0 && map.set(seed, target = i ? /* @__PURE__ */ new Map() : factory(env));
        map = target;
      }
      return target;
    };
    getFetch();
    var knownAdapters = {
      http: httpAdapter,
      xhr: xhrAdapter,
      fetch: {
        get: getFetch
      }
    };
    utils$1.forEach(knownAdapters, (fn, value) => {
      if (fn) {
        try {
          Object.defineProperty(fn, "name", { value });
        } catch (e) {
        }
        Object.defineProperty(fn, "adapterName", { value });
      }
    });
    var renderReason = (reason) => `- ${reason}`;
    var isResolvedHandle = (adapter) => utils$1.isFunction(adapter) || adapter === null || adapter === false;
    function getAdapter(adapters2, config) {
      adapters2 = utils$1.isArray(adapters2) ? adapters2 : [adapters2];
      const { length } = adapters2;
      let nameOrAdapter;
      let adapter;
      const rejectedReasons = {};
      for (let i = 0; i < length; i++) {
        nameOrAdapter = adapters2[i];
        let id;
        adapter = nameOrAdapter;
        if (!isResolvedHandle(nameOrAdapter)) {
          adapter = knownAdapters[(id = String(nameOrAdapter)).toLowerCase()];
          if (adapter === void 0) {
            throw new AxiosError(`Unknown adapter '${id}'`);
          }
        }
        if (adapter && (utils$1.isFunction(adapter) || (adapter = adapter.get(config)))) {
          break;
        }
        rejectedReasons[id || "#" + i] = adapter;
      }
      if (!adapter) {
        const reasons = Object.entries(rejectedReasons).map(
          ([id, state]) => `adapter ${id} ` + (state === false ? "is not supported by the environment" : "is not available in the build")
        );
        let s = length ? reasons.length > 1 ? "since :\n" + reasons.map(renderReason).join("\n") : " " + renderReason(reasons[0]) : "as no adapter specified";
        throw new AxiosError(
          `There is no suitable adapter to dispatch the request ` + s,
          "ERR_NOT_SUPPORT"
        );
      }
      return adapter;
    }
    var adapters = {
      /**
       * Resolve an adapter from a list of adapter names or functions.
       * @type {Function}
       */
      getAdapter,
      /**
       * Exposes all known adapters
       * @type {Object<string, Function|Object>}
       */
      adapters: knownAdapters
    };
    function throwIfCancellationRequested(config) {
      if (config.cancelToken) {
        config.cancelToken.throwIfRequested();
      }
      if (config.signal && config.signal.aborted) {
        throw new CanceledError(null, config);
      }
    }
    function dispatchRequest(config) {
      throwIfCancellationRequested(config);
      config.headers = AxiosHeaders$1.from(config.headers);
      config.data = transformData.call(
        config,
        config.transformRequest
      );
      if (["post", "put", "patch"].indexOf(config.method) !== -1) {
        config.headers.setContentType("application/x-www-form-urlencoded", false);
      }
      const adapter = adapters.getAdapter(config.adapter || defaults$1.adapter, config);
      return adapter(config).then(function onAdapterResolution(response) {
        throwIfCancellationRequested(config);
        response.data = transformData.call(
          config,
          config.transformResponse,
          response
        );
        response.headers = AxiosHeaders$1.from(response.headers);
        return response;
      }, function onAdapterRejection(reason) {
        if (!isCancel(reason)) {
          throwIfCancellationRequested(config);
          if (reason && reason.response) {
            reason.response.data = transformData.call(
              config,
              config.transformResponse,
              reason.response
            );
            reason.response.headers = AxiosHeaders$1.from(reason.response.headers);
          }
        }
        return Promise.reject(reason);
      });
    }
    var validators$1 = {};
    ["object", "boolean", "number", "function", "string", "symbol"].forEach((type, i) => {
      validators$1[type] = function validator2(thing) {
        return typeof thing === type || "a" + (i < 1 ? "n " : " ") + type;
      };
    });
    var deprecatedWarnings = {};
    validators$1.transitional = function transitional(validator2, version, message) {
      function formatMessage(opt, desc) {
        return "[Axios v" + VERSION + "] Transitional option '" + opt + "'" + desc + (message ? ". " + message : "");
      }
      return (value, opt, opts2) => {
        if (validator2 === false) {
          throw new AxiosError(
            formatMessage(opt, " has been removed" + (version ? " in " + version : "")),
            AxiosError.ERR_DEPRECATED
          );
        }
        if (version && !deprecatedWarnings[opt]) {
          deprecatedWarnings[opt] = true;
          console.warn(
            formatMessage(
              opt,
              " has been deprecated since v" + version + " and will be removed in the near future"
            )
          );
        }
        return validator2 ? validator2(value, opt, opts2) : true;
      };
    };
    validators$1.spelling = function spelling(correctSpelling) {
      return (value, opt) => {
        console.warn(`${opt} is likely a misspelling of ${correctSpelling}`);
        return true;
      };
    };
    function assertOptions(options, schema, allowUnknown) {
      if (typeof options !== "object") {
        throw new AxiosError("options must be an object", AxiosError.ERR_BAD_OPTION_VALUE);
      }
      const keys = Object.keys(options);
      let i = keys.length;
      while (i-- > 0) {
        const opt = keys[i];
        const validator2 = schema[opt];
        if (validator2) {
          const value = options[opt];
          const result = value === void 0 || validator2(value, opt, options);
          if (result !== true) {
            throw new AxiosError("option " + opt + " must be " + result, AxiosError.ERR_BAD_OPTION_VALUE);
          }
          continue;
        }
        if (allowUnknown !== true) {
          throw new AxiosError("Unknown option " + opt, AxiosError.ERR_BAD_OPTION);
        }
      }
    }
    var validator = {
      assertOptions,
      validators: validators$1
    };
    var validators = validator.validators;
    var Axios = class {
      constructor(instanceConfig) {
        this.defaults = instanceConfig || {};
        this.interceptors = {
          request: new InterceptorManager$1(),
          response: new InterceptorManager$1()
        };
      }
      /**
       * Dispatch a request
       *
       * @param {String|Object} configOrUrl The config specific for this request (merged with this.defaults)
       * @param {?Object} config
       *
       * @returns {Promise} The Promise to be fulfilled
       */
      async request(configOrUrl, config) {
        try {
          return await this._request(configOrUrl, config);
        } catch (err) {
          if (err instanceof Error) {
            let dummy = {};
            Error.captureStackTrace ? Error.captureStackTrace(dummy) : dummy = new Error();
            const stack = dummy.stack ? dummy.stack.replace(/^.+\n/, "") : "";
            try {
              if (!err.stack) {
                err.stack = stack;
              } else if (stack && !String(err.stack).endsWith(stack.replace(/^.+\n.+\n/, ""))) {
                err.stack += "\n" + stack;
              }
            } catch (e) {
            }
          }
          throw err;
        }
      }
      _request(configOrUrl, config) {
        if (typeof configOrUrl === "string") {
          config = config || {};
          config.url = configOrUrl;
        } else {
          config = configOrUrl || {};
        }
        config = mergeConfig(this.defaults, config);
        const { transitional, paramsSerializer, headers } = config;
        if (transitional !== void 0) {
          validator.assertOptions(transitional, {
            silentJSONParsing: validators.transitional(validators.boolean),
            forcedJSONParsing: validators.transitional(validators.boolean),
            clarifyTimeoutError: validators.transitional(validators.boolean)
          }, false);
        }
        if (paramsSerializer != null) {
          if (utils$1.isFunction(paramsSerializer)) {
            config.paramsSerializer = {
              serialize: paramsSerializer
            };
          } else {
            validator.assertOptions(paramsSerializer, {
              encode: validators.function,
              serialize: validators.function
            }, true);
          }
        }
        if (config.allowAbsoluteUrls !== void 0)
          ;
        else if (this.defaults.allowAbsoluteUrls !== void 0) {
          config.allowAbsoluteUrls = this.defaults.allowAbsoluteUrls;
        } else {
          config.allowAbsoluteUrls = true;
        }
        validator.assertOptions(config, {
          baseUrl: validators.spelling("baseURL"),
          withXsrfToken: validators.spelling("withXSRFToken")
        }, true);
        config.method = (config.method || this.defaults.method || "get").toLowerCase();
        let contextHeaders = headers && utils$1.merge(
          headers.common,
          headers[config.method]
        );
        headers && utils$1.forEach(
          ["delete", "get", "head", "post", "put", "patch", "common"],
          (method) => {
            delete headers[method];
          }
        );
        config.headers = AxiosHeaders$1.concat(contextHeaders, headers);
        const requestInterceptorChain = [];
        let synchronousRequestInterceptors = true;
        this.interceptors.request.forEach(function unshiftRequestInterceptors(interceptor) {
          if (typeof interceptor.runWhen === "function" && interceptor.runWhen(config) === false) {
            return;
          }
          synchronousRequestInterceptors = synchronousRequestInterceptors && interceptor.synchronous;
          requestInterceptorChain.unshift(interceptor.fulfilled, interceptor.rejected);
        });
        const responseInterceptorChain = [];
        this.interceptors.response.forEach(function pushResponseInterceptors(interceptor) {
          responseInterceptorChain.push(interceptor.fulfilled, interceptor.rejected);
        });
        let promise;
        let i = 0;
        let len;
        if (!synchronousRequestInterceptors) {
          const chain = [dispatchRequest.bind(this), void 0];
          chain.unshift(...requestInterceptorChain);
          chain.push(...responseInterceptorChain);
          len = chain.length;
          promise = Promise.resolve(config);
          while (i < len) {
            promise = promise.then(chain[i++], chain[i++]);
          }
          return promise;
        }
        len = requestInterceptorChain.length;
        let newConfig = config;
        while (i < len) {
          const onFulfilled = requestInterceptorChain[i++];
          const onRejected = requestInterceptorChain[i++];
          try {
            newConfig = onFulfilled(newConfig);
          } catch (error) {
            onRejected.call(this, error);
            break;
          }
        }
        try {
          promise = dispatchRequest.call(this, newConfig);
        } catch (error) {
          return Promise.reject(error);
        }
        i = 0;
        len = responseInterceptorChain.length;
        while (i < len) {
          promise = promise.then(responseInterceptorChain[i++], responseInterceptorChain[i++]);
        }
        return promise;
      }
      getUri(config) {
        config = mergeConfig(this.defaults, config);
        const fullPath = buildFullPath(config.baseURL, config.url, config.allowAbsoluteUrls);
        return buildURL(fullPath, config.params, config.paramsSerializer);
      }
    };
    utils$1.forEach(["delete", "get", "head", "options"], function forEachMethodNoData(method) {
      Axios.prototype[method] = function(url2, config) {
        return this.request(mergeConfig(config || {}, {
          method,
          url: url2,
          data: (config || {}).data
        }));
      };
    });
    utils$1.forEach(["post", "put", "patch"], function forEachMethodWithData(method) {
      function generateHTTPMethod(isForm) {
        return function httpMethod(url2, data, config) {
          return this.request(mergeConfig(config || {}, {
            method,
            headers: isForm ? {
              "Content-Type": "multipart/form-data"
            } : {},
            url: url2,
            data
          }));
        };
      }
      Axios.prototype[method] = generateHTTPMethod();
      Axios.prototype[method + "Form"] = generateHTTPMethod(true);
    });
    var Axios$1 = Axios;
    var CancelToken = class _CancelToken {
      constructor(executor) {
        if (typeof executor !== "function") {
          throw new TypeError("executor must be a function.");
        }
        let resolvePromise;
        this.promise = new Promise(function promiseExecutor(resolve) {
          resolvePromise = resolve;
        });
        const token = this;
        this.promise.then((cancel) => {
          if (!token._listeners)
            return;
          let i = token._listeners.length;
          while (i-- > 0) {
            token._listeners[i](cancel);
          }
          token._listeners = null;
        });
        this.promise.then = (onfulfilled) => {
          let _resolve;
          const promise = new Promise((resolve) => {
            token.subscribe(resolve);
            _resolve = resolve;
          }).then(onfulfilled);
          promise.cancel = function reject() {
            token.unsubscribe(_resolve);
          };
          return promise;
        };
        executor(function cancel(message, config, request) {
          if (token.reason) {
            return;
          }
          token.reason = new CanceledError(message, config, request);
          resolvePromise(token.reason);
        });
      }
      /**
       * Throws a `CanceledError` if cancellation has been requested.
       */
      throwIfRequested() {
        if (this.reason) {
          throw this.reason;
        }
      }
      /**
       * Subscribe to the cancel signal
       */
      subscribe(listener) {
        if (this.reason) {
          listener(this.reason);
          return;
        }
        if (this._listeners) {
          this._listeners.push(listener);
        } else {
          this._listeners = [listener];
        }
      }
      /**
       * Unsubscribe from the cancel signal
       */
      unsubscribe(listener) {
        if (!this._listeners) {
          return;
        }
        const index = this._listeners.indexOf(listener);
        if (index !== -1) {
          this._listeners.splice(index, 1);
        }
      }
      toAbortSignal() {
        const controller = new AbortController();
        const abort = (err) => {
          controller.abort(err);
        };
        this.subscribe(abort);
        controller.signal.unsubscribe = () => this.unsubscribe(abort);
        return controller.signal;
      }
      /**
       * Returns an object that contains a new `CancelToken` and a function that, when called,
       * cancels the `CancelToken`.
       */
      static source() {
        let cancel;
        const token = new _CancelToken(function executor(c) {
          cancel = c;
        });
        return {
          token,
          cancel
        };
      }
    };
    var CancelToken$1 = CancelToken;
    function spread(callback) {
      return function wrap(arr) {
        return callback.apply(null, arr);
      };
    }
    function isAxiosError(payload) {
      return utils$1.isObject(payload) && payload.isAxiosError === true;
    }
    var HttpStatusCode = {
      Continue: 100,
      SwitchingProtocols: 101,
      Processing: 102,
      EarlyHints: 103,
      Ok: 200,
      Created: 201,
      Accepted: 202,
      NonAuthoritativeInformation: 203,
      NoContent: 204,
      ResetContent: 205,
      PartialContent: 206,
      MultiStatus: 207,
      AlreadyReported: 208,
      ImUsed: 226,
      MultipleChoices: 300,
      MovedPermanently: 301,
      Found: 302,
      SeeOther: 303,
      NotModified: 304,
      UseProxy: 305,
      Unused: 306,
      TemporaryRedirect: 307,
      PermanentRedirect: 308,
      BadRequest: 400,
      Unauthorized: 401,
      PaymentRequired: 402,
      Forbidden: 403,
      NotFound: 404,
      MethodNotAllowed: 405,
      NotAcceptable: 406,
      ProxyAuthenticationRequired: 407,
      RequestTimeout: 408,
      Conflict: 409,
      Gone: 410,
      LengthRequired: 411,
      PreconditionFailed: 412,
      PayloadTooLarge: 413,
      UriTooLong: 414,
      UnsupportedMediaType: 415,
      RangeNotSatisfiable: 416,
      ExpectationFailed: 417,
      ImATeapot: 418,
      MisdirectedRequest: 421,
      UnprocessableEntity: 422,
      Locked: 423,
      FailedDependency: 424,
      TooEarly: 425,
      UpgradeRequired: 426,
      PreconditionRequired: 428,
      TooManyRequests: 429,
      RequestHeaderFieldsTooLarge: 431,
      UnavailableForLegalReasons: 451,
      InternalServerError: 500,
      NotImplemented: 501,
      BadGateway: 502,
      ServiceUnavailable: 503,
      GatewayTimeout: 504,
      HttpVersionNotSupported: 505,
      VariantAlsoNegotiates: 506,
      InsufficientStorage: 507,
      LoopDetected: 508,
      NotExtended: 510,
      NetworkAuthenticationRequired: 511,
      WebServerIsDown: 521,
      ConnectionTimedOut: 522,
      OriginIsUnreachable: 523,
      TimeoutOccurred: 524,
      SslHandshakeFailed: 525,
      InvalidSslCertificate: 526
    };
    Object.entries(HttpStatusCode).forEach(([key, value]) => {
      HttpStatusCode[value] = key;
    });
    var HttpStatusCode$1 = HttpStatusCode;
    function createInstance(defaultConfig) {
      const context = new Axios$1(defaultConfig);
      const instance = bind(Axios$1.prototype.request, context);
      utils$1.extend(instance, Axios$1.prototype, context, { allOwnKeys: true });
      utils$1.extend(instance, context, null, { allOwnKeys: true });
      instance.create = function create(instanceConfig) {
        return createInstance(mergeConfig(defaultConfig, instanceConfig));
      };
      return instance;
    }
    var axios = createInstance(defaults$1);
    axios.Axios = Axios$1;
    axios.CanceledError = CanceledError;
    axios.CancelToken = CancelToken$1;
    axios.isCancel = isCancel;
    axios.VERSION = VERSION;
    axios.toFormData = toFormData;
    axios.AxiosError = AxiosError;
    axios.Cancel = axios.CanceledError;
    axios.all = function all(promises) {
      return Promise.all(promises);
    };
    axios.spread = spread;
    axios.isAxiosError = isAxiosError;
    axios.mergeConfig = mergeConfig;
    axios.AxiosHeaders = AxiosHeaders$1;
    axios.formToJSON = (thing) => formDataToJSON(utils$1.isHTMLForm(thing) ? new FormData(thing) : thing);
    axios.getAdapter = adapters.getAdapter;
    axios.HttpStatusCode = HttpStatusCode$1;
    axios.default = axios;
    module2.exports = axios;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/axiosInstance.js
var require_axiosInstance = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/axiosInstance.js"(exports2, module2) {
    var axios = require_axios().default;
    var axiosInstance = axios.create({});
    module2.exports = axiosInstance;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/errorMessage.js
var require_errorMessage = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/errorMessage.js"(exports2, module2) {
    function getErrorAsMessage(url, api, err) {
      let str2 = "";
      if (err.response) {
        err = err.response;
        if (err.data)
          str2 += `${formatErrorMessage(err.data)}`;
      } else if (err.body)
        str2 += `${formatErrorMessage(err.body)}`;
      url = url.split("?")[0];
      if (err.status)
        str2 += `Code: ${err.status}`;
      if (err.action)
        str2 += ` Action: ${err.action}`;
      if (err.type)
        str2 += ` Type: ${err.type}`;
      if (url)
        str2 += ` url: ${url}`;
      if (api && api.method)
        str2 += ` method: ${api.method}`;
      if (err.stack)
        str2 = [str2, err.stack].join("\n");
      return str2;
    }
    function formatErrorMessage(msg) {
      if (typeof msg === "object") {
        let nmsg = "";
        for (const key of Object.keys(msg)) {
          if (msg[key] && typeof msg[key] !== "object")
            nmsg += `${key} ${msg[key]} `;
        }
        return nmsg;
      }
      return msg;
    }
    function accessDenied(action, db, server) {
      const err = {};
      err.status = 403;
      err.url = (server || "") + (db || "");
      err.type = "client";
      err.action = action;
      err.body = `${err.action} not permitted for ${err.url}`;
      return err;
    }
    function getAPIErrorMessage(url, api, err) {
      return `API Error ${getErrorAsMessage(url, api, err)}`;
    }
    function getAccessDeniedMessage(url, api, err) {
      return `Access Denied ${getErrorAsMessage(url, api, err)}`;
    }
    function getInvalidURIMessage(url, call) {
      const str2 = `Invalid argument to
            ${call}. 
            ${url}
            is not a valid Terminus DB API endpoint`;
      return str2;
    }
    function getInvalidParameterMessage(call, msg) {
      const str2 = `Invalid Parameter to
            ${call}. 
            ${msg}`;
      return str2;
    }
    function parseAPIError(response) {
      const err = {};
      err.status = response.status;
      err.type = response.type;
      if (response.data && typeof response.data === "object") {
        let msg;
        try {
          msg = response.text();
        } catch (e) {
          try {
            msg = response.json();
          } catch (error) {
            msg = response.toString();
          }
        }
        err.body = msg;
      } else if (response.data)
        err.body = response.data;
      err.url = response.url;
      err.headers = response.headers;
      err.redirected = response.redirected;
      return err;
    }
    function apiErrorFormatted(url, options, err) {
      const e = new Error(getAPIErrorMessage(url, options, err));
      if (err.response && err.response.data)
        e.data = err.response.data;
      if (err.response && err.response.status)
        e.status = err.response.status;
      return e;
    }
    module2.exports = {
      apiErrorFormatted,
      getErrorAsMessage,
      getAPIErrorMessage,
      getAccessDeniedMessage,
      accessDenied,
      getInvalidURIMessage,
      getInvalidParameterMessage,
      parseAPIError
    };
  }
});

// node_modules/@terminusdb/terminusdb-client/package.json
var require_package = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/package.json"(exports2, module2) {
    module2.exports = {
      name: "@terminusdb/terminusdb-client",
      version: "11.1.2",
      description: "TerminusDB client library",
      main: "index.js",
      types: "./dist/typescript/index.d.ts",
      files: [
        "*.md",
        "lib",
        "dist"
      ],
      directories: {
        lib: "lib",
        test: "test",
        dist: "dist"
      },
      publishConfig: {
        access: "public"
      },
      author: "kevin@terminusdb.com",
      license: "Apache-2.0",
      dependencies: {
        axios: "^1.7.2",
        buffer: "^6.0.3",
        "follow-redirects": "^1.14.8",
        "form-data": "^4.0.0",
        jest: "^29.1.2",
        "node-forge": "^1.0.0",
        pako: "^2.0.4",
        pathval: "^1.1.1",
        "ts-node": "^10.9.1",
        underscore: "^1.13.2"
      },
      devDependencies: {
        "@babel/core": "^7.13.10",
        "@babel/preset-env": "^7.13.12",
        "@babel/register": "^7.13.8",
        "@types/jest": "^29.1.2",
        "babel-loader": "^8.0.6",
        chai: "^4.3.4",
        concurrently: "^7.4.0",
        eol: "^0.9.1",
        eslint: "^8.6.0",
        "eslint-config-airbnb-base": "^15.0.0",
        "eslint-config-prettier": "6.11.0",
        "eslint-plugin-import": "^2.25.4",
        "eslint-plugin-prettier": "3.1.3",
        "html-webpack-plugin": "^5.3.1",
        husky: "^7.0.4",
        "jsdoc-to-markdown": "^7.1.0",
        mocha: "^11.2.2",
        nyc: "^15.1.0",
        prettier: "^1.19.1",
        sinon: "^12.0.1",
        "ts-jest": "^29.0.3",
        typescript: "^4.6.4",
        webpack: "^5.36.2",
        "webpack-cli": "^4.6.0",
        "webpack-dev-server": "^5.2.1"
      },
      scripts: {
        "mkdocs:multi": "node ./docs/createDocs.js",
        "mkdocs:src": "docco lib/*.js -l plain -x md -o docs/api",
        "mkdocs:api": "jsdoc2md --configure docs/doc_config.json --partial  docs/partial/scope.hbs docs/partial/member-index.hbs docs/partial/header.hbs --helper docs/helper/format.js --files lib/woql.js lib/woqlClient.js lib/typedef.js > docs/api/api.js.md",
        mkdocs: "cp README.md docs/ && npm run mkdocs:multi",
        "test:integration": "jest",
        test: "npm run cover",
        "test:only": "mocha --require @babel/register --require @babel/preset-env --recursive  ",
        "test:watch": "mocha --watch --require @babel/register --require @babel/preset-env --recursive",
        "test:examples": "node examples/",
        cover: "nyc --check-coverage --lines 30 npm run test:only ",
        "lint:check": "eslint .",
        lint: "eslint --fix .",
        "validate-types:strict": "npm run generate-types && tsc --project tsconfig.validate.json",
        build: "webpack --mode production && npm run generate-types",
        "coveralls-after": "nyc --reporter=lcov mocha --require @babel/register --require @babel/preset-env",
        "npm:publish": "npm publish --access public",
        "test-single": "mocha $1",
        "woql-test": "mocha test/woqlTripleBuilder.spec.js  test/woql.spec.js test/woqlTripleBuilder01.spec.js test/woqlExtra.spec.js",
        "git-tag": "git tag $npm_package_version",
        prepare: "husky install",
        "generate-types": "tsc && node scripts/fix-eval-export.js"
      },
      repository: {
        type: "git",
        url: "git+https://github.com/terminusdb/terminusdb-client-js.git"
      },
      keywords: [
        "Terminus",
        "WOQL",
        "Driver",
        "Database",
        "DB"
      ],
      bugs: {
        url: "https://github.com/terminusdb/terminusdb-client/issues"
      },
      homepage: "https://github.com/terminusdb/terminusdb-client#readme",
      browser: {
        http: false,
        https: false,
        net: false,
        path: false,
        stream: false,
        tls: false,
        fs: false
      }
    };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/dispatchRequest.js
var require_dispatchRequest = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/dispatchRequest.js"(exports2, module2) {
    var pako = require_pako();
    var axiosInstance = require_axiosInstance();
    var UTILS2 = require_utils();
    var CONST = require_const();
    var ErrorMessage = require_errorMessage();
    var { version } = require_package();
    var typedef2 = require_typedef();
    function btoaImplementation(str2) {
      try {
        return btoa(str2);
      } catch (err) {
        return Buffer.from(str2).toString("base64");
      }
    }
    function getResultWithDataVersion(response) {
      return {
        result: response.data,
        dataVersion: response.headers["terminusdb-data-version"] ? response.headers["terminusdb-data-version"] : ""
      };
    }
    function formatAuthHeader(auth_obj) {
      if (!auth_obj)
        return "";
      const authType = { jwt: "Bearer", basic: "Basic", apikey: "Token" };
      let auth_key = auth_obj.key;
      if (auth_obj.type === "basic") {
        auth_key = btoaImplementation(`${auth_obj.user}:${auth_obj.key}`);
      }
      return `${authType[auth_obj.type]} ${auth_key}`;
    }
    function checkPayload(payload, options, compress) {
      if (!payload || typeof payload !== "object")
        return false;
      const jsonStringPost = JSON.stringify(payload);
      if (jsonStringPost && jsonStringPost.length > 1024 && compress) {
        options.headers["Content-Encoding"] = "gzip";
        return pako.gzip(jsonStringPost);
      }
      return false;
    }
    function DispatchRequest(url, action, payload, local_auth, remote_auth = null, customHeaders = null, getDataVersion = false, compress = false) {
      const options = {
        mode: "cors",
        // no-cors, cors, *same-origin
        redirect: "follow",
        // manual, *follow, error
        referrer: "client",
        maxContentLength: Infinity,
        maxBodyLength: Infinity,
        headers: {}
        // url:url,
        // no-referrer, *client
      };
      if (url.startsWith("https://127.0.0.1") && typeof window === "undefined") {
        const https = require("https");
        const agent = new https.Agent({
          rejectUnauthorized: false
        });
        options.httpsAgent = agent;
      }
      if (local_auth && typeof local_auth === "object") {
        options.headers.Authorization = formatAuthHeader(local_auth);
      }
      if (remote_auth && typeof remote_auth === "object") {
        options.headers["Authorization-Remote"] = formatAuthHeader(remote_auth);
      }
      if (customHeaders && typeof customHeaders === "object") {
        Object.keys(customHeaders).map((key) => {
          options.headers[key] = customHeaders[key];
        });
      }
      if (typeof window === "undefined") {
        options.headers["User-Agent"] = `terminusdb-client-js/${version}`;
      }
      switch (action) {
        case CONST.DELETE: {
          if (payload) {
            options.headers = options.headers ? options.headers : {};
            options.headers["Content-Type"] = "application/json; charset=utf-8";
            options.data = payload;
          }
          return axiosInstance.delete(url, options).then((response) => getDataVersion ? getResultWithDataVersion(response) : response.data).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
        case CONST.HEAD: {
          return axiosInstance.head(url, options).then((response) => getDataVersion ? getResultWithDataVersion(response) : response.data).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
        case CONST.GET: {
          if (payload) {
            const ext = UTILS2.URIEncodePayload(payload);
            if (ext)
              url += `?${ext}`;
          }
          return axiosInstance.get(url, options).then((response) => {
            const r = getDataVersion ? getResultWithDataVersion(response) : response.data;
            return r;
          }).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
        case CONST.ADD_CSV:
        case CONST.INSERT_TRIPLES: {
          options.headers = options.headers ? options.headers : {};
          options.headers["Content-Type"] = "application/form-data; charset=utf-8";
          return axiosInstance.put(url, payload, options).then((response) => getDataVersion ? getResultWithDataVersion(response) : response.data).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
        case CONST.PUT: {
          options.headers = options.headers ? options.headers : {};
          options.headers["Content-Type"] = "application/json; charset=utf-8";
          let compressedContent = null;
          const jsonString = JSON.stringify(payload);
          if (jsonString.length > 1024 && compress) {
            options.headers["Content-Encoding"] = "gzip";
            compressedContent = pako.gzip(jsonString);
          }
          return axiosInstance.put(url, compressedContent || payload, options).then((response) => getDataVersion ? getResultWithDataVersion(response) : response.data).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
        case CONST.QUERY_DOCUMENT: {
          options.headers = options.headers ? options.headers : {};
          options.headers["X-HTTP-Method-Override"] = "GET";
        }
        default: {
          options.headers = options.headers ? options.headers : {};
          if (!options.headers["content-type"] && !options.headers["Content-Type"]) {
            options.headers["Content-Type"] = "application/json; charset=utf-8";
          }
          const compressedContentPost = checkPayload(payload, options, compress);
          return axiosInstance.post(url, compressedContentPost || payload || {}, options).then((response) => {
            const r = getDataVersion ? getResultWithDataVersion(response) : response.data;
            return r;
          }).catch((err) => {
            throw ErrorMessage.apiErrorFormatted(url, options, err);
          });
        }
      }
    }
    module2.exports = DispatchRequest;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/connectionConfig.js
var require_connectionConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/connectionConfig.js"(exports2, module2) {
    var { encodeURISegment } = require_utils();
    var typedef2 = require_typedef();
    function ConnectionConfig(serverUrl, params) {
      this.server = void 0;
      this.baseServer = void 0;
      this.remote_auth = void 0;
      this.local_auth = void 0;
      this.organizationid = false;
      this.dbid = false;
      this.default_branch_id = params && params.default_branch_id ? params.default_branch_id : "main";
      this.default_repo_id = "local";
      this.system_db = "_system";
      this.api_extension = "api/";
      this.branchid = this.default_branch_id;
      this.repoid = this.default_repo_id;
      this.refid = false;
      this.connection_error = false;
      const surl = this.parseServerURL(serverUrl);
      this.server = surl;
      if (params)
        this.update(params);
    }
    ConnectionConfig.prototype.copy = function() {
      const other = new ConnectionConfig(this.server);
      other.api_extension = this.api_extension;
      other.remote_auth = this.remote_auth;
      other.local_auth = this.local_auth;
      other.organizationid = this.organizationid;
      other.dbid = this.dbid;
      other.branchid = this.branchid;
      other.repoid = this.repoid;
      other.refid = this.refid;
      return other;
    };
    ConnectionConfig.prototype.update = function(params) {
      if (!params)
        return;
      const orgID = params.organization || params.user;
      this.setOrganization(orgID);
      if (typeof params.db !== "undefined")
        this.setDB(params.db);
      if (typeof params.token !== "undefined")
        this.setLocalBasicAuth(params.token, params.user, "apikey");
      else if (typeof params.jwt !== "undefined")
        this.setLocalBasicAuth(params.jwt, params.user, "jwt");
      else if (typeof params.key !== "undefined")
        this.setLocalBasicAuth(params.key, params.user);
      else if (typeof params.user !== "undefined")
        this.setLocalBasicAuth(null, params.user);
      if (typeof params.branch !== "undefined")
        this.setBranch(params.branch);
      if (typeof params.ref !== "undefined")
        this.setRef(params.ref);
      if (typeof params.repo !== "undefined")
        this.setRepo(params.repo);
    };
    ConnectionConfig.prototype.serverURL = function() {
      return this.server;
    };
    ConnectionConfig.prototype.author = function() {
      return this.author;
    };
    ConnectionConfig.prototype.apiURL = function() {
      return this.server + this.api_extension;
    };
    ConnectionConfig.prototype.apiURLInfo = function() {
      return `${this.apiURL()}info`;
    };
    ConnectionConfig.prototype.db = function() {
      if (!this.dbid)
        throw new Error("Invalid database name");
      return this.dbid;
    };
    ConnectionConfig.prototype.branch = function() {
      return this.branchid;
    };
    ConnectionConfig.prototype.ref = function() {
      return this.refid;
    };
    ConnectionConfig.prototype.organization = function() {
      return this.organizationid;
    };
    ConnectionConfig.prototype.repo = function() {
      return this.repoid;
    };
    ConnectionConfig.prototype.localAuth = function() {
      if (this.local_auth)
        return this.local_auth;
      return false;
    };
    ConnectionConfig.prototype.localUser = function() {
      if (this.local_auth)
        return this.local_auth.user;
      return false;
    };
    ConnectionConfig.prototype.user = function(ignoreJwt) {
      if (!ignoreJwt && this.remote_auth && this.remote_auth.type === "jwt")
        return this.remote_auth.user;
      if (this.local_auth) {
        return this.local_auth.user;
      }
      return false;
    };
    ConnectionConfig.prototype.parseServerURL = function(str2) {
      if (str2 && (str2.substring(0, 7) === "http://" || str2.substring(0, 8) === "https://")) {
        if (str2.lastIndexOf("/") !== str2.length - 1) {
          str2 += "/";
        }
        return this.serverUrlEncoding(str2);
      }
      throw new Error(`Invalid Server URL: ${str2}`);
    };
    ConnectionConfig.prototype.serverUrlEncoding = function(str2) {
      const orgArr = str2.split("/");
      if (orgArr.length > 4) {
        this.baseServer = str2.replace(`${orgArr[3]}/`, "");
        const org = encodeURISegment(orgArr[3]);
        return str2.replace(orgArr[3], org);
      }
      this.baseServer = str2;
      return str2;
    };
    ConnectionConfig.prototype.clearCursor = function() {
      this.branchid = this.default_branch_id;
      this.repoid = this.default_repo_id;
      this.organizationid = false;
      this.dbid = false;
      this.refid = false;
    };
    ConnectionConfig.prototype.setError = function(errorMessage) {
      this.connection_error = errorMessage;
    };
    ConnectionConfig.prototype.setOrganization = function(orgId = "admin") {
      this.organizationid = orgId;
    };
    ConnectionConfig.prototype.setDB = function(dbId) {
      this.dbid = dbId;
    };
    ConnectionConfig.prototype.setRepo = function(repoId) {
      this.repoid = repoId;
    };
    ConnectionConfig.prototype.setBranch = function(branchId) {
      this.branchid = branchId || this.default_branch_id;
    };
    ConnectionConfig.prototype.setRef = function(refId) {
      this.refid = refId;
    };
    ConnectionConfig.prototype.setRemoteBasicAuth = function(remoteKey, remoteUserID) {
      if (!remoteKey) {
        this.remote_auth = void 0;
      } else {
        this.remote_auth = { type: "jwt", user: remoteUserID, key: remoteKey };
      }
    };
    ConnectionConfig.prototype.setLocalBasicAuth = function(userKey, userId = "admin", type = "basic") {
      this.local_auth = { type, user: userId, key: userKey };
    };
    ConnectionConfig.prototype.setLocalAuth = function(newCredential) {
      this.local_auth = newCredential;
    };
    ConnectionConfig.prototype.setRemoteAuth = function(newCredential) {
      this.remote_auth = newCredential;
    };
    ConnectionConfig.prototype.remoteAuth = function() {
      if (this.remote_auth)
        return this.remote_auth;
      return false;
    };
    ConnectionConfig.prototype.dbURL = function() {
      return this.dbBase("db");
    };
    ConnectionConfig.prototype.userURL = function(user) {
      let url = `${this.apiURL()}user`;
      if (user)
        url += `/${encodeURISegment(user)}`;
      return url;
    };
    ConnectionConfig.prototype.organizationURL = function(orgId, action) {
      let url = `${this.apiURL()}organization`;
      if (orgId)
        url += `/${encodeURISegment(orgId)}`;
      if (action)
        url += `/${encodeURISegment(action)}`;
      return url;
    };
    ConnectionConfig.prototype.userOrganizationsURL = function() {
      const url = `${this.apiURL()}user_organizations`;
      return url;
    };
    ConnectionConfig.prototype.rolesURL = function() {
      return `${this.apiURL()}role`;
    };
    ConnectionConfig.prototype.updateRolesURL = function() {
      return `${this.apiURL()}update_role`;
    };
    ConnectionConfig.prototype.graphURL = function(graphType) {
      return `${this.branchBase("graph")}/${graphType}/main`;
    };
    ConnectionConfig.prototype.triplesURL = function(graphType) {
      let url = "";
      if (this.db() === this.system_db) {
        const s = this.dbBase("triples");
      } else {
        url = this.branchBase("triples");
      }
      url += `/${graphType}/main`;
      return url;
    };
    ConnectionConfig.prototype.csvURL = function() {
      const s = this.branchBase("csv");
      return s;
    };
    ConnectionConfig.prototype.queryURL = function() {
      if (this.db() === this.system_db)
        return this.dbBase("woql");
      return this.branchBase("woql");
    };
    ConnectionConfig.prototype.log = function() {
      if (this.db() === this.system_db)
        return this.dbBase("log");
      return this.branchBase("log");
    };
    ConnectionConfig.prototype.updateOrganizationRoleURL = function() {
      return `${this.apiURL()}update_role`;
    };
    ConnectionConfig.prototype.cloneURL = function(newRepoId) {
      let crl = `${this.apiURL()}clone/${this.organization()}`;
      if (newRepoId)
        crl += `/${newRepoId}`;
      return crl;
    };
    ConnectionConfig.prototype.cloneableURL = function() {
      return `${this.serverURL()}${this.organization()}/${this.db()}`;
    };
    ConnectionConfig.prototype.pullURL = function() {
      const purl = this.branchBase("pull");
      return purl;
    };
    ConnectionConfig.prototype.patchURL = function() {
      const purl = this.branchBase("patch");
      return purl;
    };
    ConnectionConfig.prototype.diffURL = function() {
      const purl = this.branchBase("diff");
      return purl;
    };
    ConnectionConfig.prototype.applyURL = function() {
      const purl = this.branchBase("apply");
      return purl;
    };
    ConnectionConfig.prototype.docHistoryURL = function(params) {
      const paramsStr = this.queryParameter(params);
      if (this.db() === this.system_db) {
        return this.dbBase("history") + paramsStr;
      }
      return this.branchBase("history") + paramsStr;
    };
    ConnectionConfig.prototype.fetchURL = function(remoteName) {
      const purl = this.dbBase("fetch");
      return `${purl}/${remoteName}/_commits`;
    };
    ConnectionConfig.prototype.remoteURL = function(remoteName) {
      const base = this.dbBase("remote");
      if (remoteName) {
        return `${base}/${encodeURISegment(remoteName)}`;
      }
      return base;
    };
    ConnectionConfig.prototype.rebaseURL = function() {
      const purl = this.branchBase("rebase");
      return purl;
    };
    ConnectionConfig.prototype.resetURL = function() {
      const purl = this.branchBase("reset");
      return purl;
    };
    ConnectionConfig.prototype.pushURL = function() {
      const purl = this.branchBase("push");
      return purl;
    };
    ConnectionConfig.prototype.branchURL = function(branchId) {
      const url = this.repoBase("branch");
      return `${url}/branch/${branchId}`;
    };
    ConnectionConfig.prototype.squashBranchURL = function(nuid) {
      const b = this.repoBase("squash");
      return `${b}/branch/${nuid}`;
    };
    ConnectionConfig.prototype.resetBranchUrl = function(nuid) {
      const b = this.repoBase("reset");
      return `${b}/branch/${nuid}`;
    };
    ConnectionConfig.prototype.commitDescriptorUrl = function(commitId) {
      return `${this.organization()}/${this.db()}/${this.repoid}/commit/${commitId}`;
    };
    ConnectionConfig.prototype.optimizeBranchUrl = function(branchId) {
      const dbBase = this.dbBase("optimize");
      return `${dbBase}/${this.repoid}/branch/${encodeURIComponent(branchId)}`;
    };
    ConnectionConfig.prototype.dbBase = function(action) {
      return `${this.apiURL()}${action}/${this.dbURLFragment()}`;
    };
    ConnectionConfig.prototype.repoBase = function(action) {
      let b = this.dbBase(action);
      if (this.repo())
        b += `/${this.repo()}`;
      else
        b += `/${this.default_repo_id}`;
      return b;
    };
    ConnectionConfig.prototype.branchBase = function(action) {
      let b = this.repoBase(action);
      if (this.repo() === "_meta") {
        return b;
      }
      if (this.branch() === "_commits") {
        return `${b}/${this.branch()}`;
      }
      if (this.ref()) {
        return `${b}/commit/${this.ref()}`;
      }
      if (this.branch()) {
        return `${b}/branch/${encodeURIComponent(this.branch())}`;
      }
      b += `/branch/${this.default_branch_id}`;
      return b;
    };
    ConnectionConfig.prototype.dbURLFragment = function() {
      if (this.db() === this.system_db)
        return this.db();
      return `${encodeURISegment(this.organization())}/${encodeURISegment(this.db())}`;
    };
    ConnectionConfig.prototype.documentURL = function(params) {
      const paramsStr = this.queryParameter(params);
      if (this.db() === this.system_db) {
        return this.dbBase("document") + paramsStr;
      }
      return this.branchBase("document") + paramsStr;
    };
    ConnectionConfig.prototype.prefixesURL = function() {
      if (this.db() === this.system_db) {
        return this.dbBase("prefixes");
      }
      return this.branchBase("prefixes");
    };
    ConnectionConfig.prototype.queryParameter = function(params) {
      if (!params || typeof params !== "object")
        return "";
      const queryString2 = Object.keys(params).map((key) => `${key}=${encodeURISegment(params[key])}`).join("&");
      return `?${queryString2}`;
    };
    ConnectionConfig.prototype.jsonSchemaURL = function(params) {
      const paramsStr = this.queryParameter(params);
      if (this.db() === this.system_db) {
        return this.dbBase("schema") + paramsStr;
      }
      return this.branchBase("schema") + paramsStr;
    };
    module2.exports = ConnectionConfig;
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
    WOQLPrinter.prototype.printJSON = function(json2, level, fluent, newline) {
      level = level || 0;
      fluent = fluent || false;
      let str2 = "";
      if (!json2["@type"]) {
        console.log("Bad structure passed to print json, no type: ", json2);
        return "";
      }
      if (["Value", "NodeValue", "DataValue", "ArithmeticValue", "OrderTemplate"].indexOf(json2["@type"]) > -1) {
        return this.pvar(json2);
      }
      let operator = json2["@type"];
      if (typeof json2["@type"] === "string" && operator.indexOf(":") > -1) {
        operator = json2["@type"].split(":")[1];
      }
      if (operator === "QueryResource") {
        return this.getQueryResourceStr(json2, level, fluent, newline);
      }
      if (operator) {
        const ujson = this.unboxJSON(operator, json2);
        if (ujson) {
          const meat = this.printArgument(
            operator,
            this.getBoxedPredicate(operator, json2),
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
          const call = this.getFunctionForOperator(operator, json2);
          const indent = newline ? level * this.indent_spaces : 0;
          str2 += `${this.getWOQLPrelude(call, fluent, indent)}(`;
        }
        const args2 = this.getArgumentOrder(operator, json2);
        const divlimit = args2.indexOf("query") === -1 ? args2.length - 1 : args2.length - 2;
        args2.forEach((item, i) => {
          let nfluent = !!(item === "query" && operator !== "Put" || item === "consequent" || item === "resource");
          if (item === "resource" && typeof json2[item] === "string")
            nfluent = false;
          str2 += this.printArgument(operator, item, json2[item], level, nfluent);
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
        console.log("wrong structure passed to print json ", json2);
      }
      return str2;
    };
    WOQLPrinter.prototype.getQueryResourceStr = function(json2, level, fluent, newline) {
      if (!json2.source) {
        console.log("wrong structure passed to print json ", json2);
        return "";
      }
      const functName = json2.source.url ? "remote" : "file";
      const indent = newline ? level * this.indent_spaces : 0;
      let str2 = `${this.getWOQLPrelude(functName, fluent, indent)}(`;
      const source = json2.source.file ? `"${json2.source.file}"` : `"${json2.source.url}"`;
      const format = json2.format === "csv" ? "" : json2.format;
      str2 += source;
      if (format)
        str2 += `, ${format}`;
      return str2;
    };
    WOQLPrinter.prototype.getArgumentOrder = function(operator, json2) {
      const args2 = Object.keys(json2);
      args2.splice(args2.indexOf("@type"), 1);
      return args2;
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
    WOQLPrinter.prototype.decompileDocument = function(args2) {
      const jsonDoc = {};
      this.decompileDictionary(jsonDoc, args2);
      return `WOQL.doc(${JSON.stringify(jsonDoc)})`;
    };
    WOQLPrinter.prototype.decompileDictionary = function(jsonDoc, args2) {
      if (args2.dictionary && args2.dictionary.data && Array.isArray(args2.dictionary.data)) {
        args2.dictionary.data.forEach((item) => {
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
    WOQLPrinter.prototype.decompileVariables = function(args2, checkIsArray = false) {
      if (Array.isArray(args2)) {
        let str2 = "";
        args2.forEach((varName, index) => {
          str2 += `"v:${varName}"`;
          if (index < args2.length - 1)
            str2 += ", ";
        });
        if (checkIsArray && args2.length > 1)
          str2 = `[${str2}]`;
        return str2;
      }
      return "";
    };
    WOQLPrinter.prototype.decompileRegexPattern = function(json2) {
      if (typeof json2 === "object" && json2["@type"] === "DataValue") {
        return this.pvar(json2);
      }
      if (json2["@type"].startsWith("Path")) {
        return `"${this.decompilePathPattern(json2)}"`;
      }
      const str2 = json2;
      return `"${str2.replace("\\", "\\\\")}"`;
    };
    WOQLPrinter.prototype.pvar = function(json2) {
      if (json2.variable) {
        let varname = json2.variable;
        const order = json2.order ? json2.order : "";
        if (varname.indexOf(":") === -1) {
          varname = `v:${varname}`;
        }
        return order !== "" && order !== "asc" ? `["${varname}","${order}"]` : `"${varname}"`;
      }
      if (json2.node) {
        return `"${json2.node}"`;
      }
      if (json2.data) {
        return JSON.stringify(json2.data);
      }
      if (json2.list) {
        const listArr = json2.list;
        if (Array.isArray(listArr)) {
          const listTmp = [];
          listArr.forEach((listItem, index) => {
            listTmp.push(this.pvar(listItem));
          });
          return `[${listTmp.join(", ")}]`;
        }
        return this.pvar(json2.list);
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
    WOQLPrinter.prototype.getFunctionForOperator = function(operator, json2) {
      if (this.operator_maps[operator])
        return this.operator_maps[operator];
      if (operator === "Triple" && json2.graph)
        return "quad";
      const f = camelToSnake(operator);
      if (this.shortcuts[f])
        return this.shortcuts[f];
      return f;
    };
    WOQLPrinter.prototype.getBoxedPredicate = function(operator, json2) {
      for (let i = 0; i < this.boxed_predicates.length; i++) {
        if (json2[this.boxed_predicates[i]]) {
          return this.boxed_predicates[i];
        }
      }
      if (operator === "QueryListElement") {
        return "woql:query";
      }
      return false;
    };
    WOQLPrinter.prototype.unboxJSON = function(operator, json2) {
      const bp = this.getBoxedPredicate(operator, json2);
      if (bp) {
        return json2[bp];
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

// node_modules/@terminusdb/terminusdb-client/lib/query/woqlCore.js
var require_woqlCore = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/query/woqlCore.js"(exports2, module2) {
    var UTILS2 = require_utils();
    var WOQLPrinter = require_woqlPrinter();
    var { Var: Var2, Vars: Vars2, Doc: Doc2 } = require_woqlDoc();
    var typedef2 = require_typedef();
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
    WOQLQuery2.prototype.containsUpdate = function(json2) {
      json2 = json2 || this.query;
      if (this.update_operators.indexOf(json2["@type"]) !== -1)
        return true;
      if (json2.consequent && this.containsUpdate(json2.consequent))
        return true;
      if (json2.query)
        return this.containsUpdate(json2.query);
      if (json2.and) {
        for (var i = 0; i < json2.and.length; i++) {
          if (this.containsUpdate(json2.and[i]))
            return true;
        }
      }
      if (json2.or) {
        for (var i = 0; i < json2.or.length; i++) {
          if (this.containsUpdate(json2.or[i]))
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
      for (const pref2 in UTILS2.standard_urls) {
        def[pref2] = UTILS2.standard_urls[pref2];
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
    WOQLQuery2.prototype.json = function(json2) {
      if (json2) {
        this.query = copyJSON(json2);
        return this;
      }
      return copyJSON(this.query, true);
    };
    WOQLQuery2.prototype.prettyPrint = function(clang = "js") {
      const printer = new WOQLPrinter(this.vocab, clang);
      return printer.printJSON(this.query);
    };
    WOQLQuery2.prototype.findLastSubject = function(json2) {
      if (json2 && json2.and) {
        for (var i = json2.and.length - 1; i >= 0; i--) {
          const lqs = this.findLastSubject(json2.and[i]);
          if (lqs)
            return lqs;
        }
      }
      if (json2 && json2.or) {
        for (var i = json2.or.length - 1; i >= 0; i--) {
          const lqs = this.findLastSubject(json2.or[i]);
          if (lqs)
            return lqs;
        }
      }
      if (json2 && json2.query) {
        const ls = this.findLastSubject(json2.query);
        if (ls)
          return ls;
      }
      if (json2 && json2.subject) {
        return json2;
      }
      return false;
    };
    WOQLQuery2.prototype.findLastProperty = function(json2) {
      if (json2 && json2.and) {
        for (var i = json2.and.length - 1; i >= 0; i--) {
          const lqs = this.findLastProperty(json2.and[i]);
          if (lqs)
            return lqs;
        }
      }
      if (json2 && json2.or) {
        for (var i = json2.or.length - 1; i >= 0; i--) {
          const lqs = this.findLastProperty(json2.or[i]);
          if (lqs)
            return lqs;
        }
      }
      if (json2 && json2.query) {
        const ls = this.findLastProperty(json2.query);
        if (ls)
          return ls;
      }
      if (json2 && json2.subject && this._is_property_triple(json2.predicate, json2.object)) {
        return json2;
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
              if (!sub || !UTILS2.empty(sub))
                nupart.push(sub);
            } else {
              nupart.push(part[j]);
            }
          }
          nuj[k] = nupart;
        } else if (part === null) {
        } else if (typeof part === "object") {
          const q = copyJSON(part, rollup);
          if (!q || !UTILS2.empty(q))
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
    var typedef2 = require_typedef();
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
      const args2 = this.triple(subject, predicate, object);
      if (!graphRef)
        return this.parameterError("Quad takes four parameters, the last should be a graph filter");
      this.cursor["@type"] = "Triple";
      this.cursor.graph = this.cleanGraph(graphRef);
      return this;
    };
    WOQLQuery2.prototype.added_quad = function(subject, predicate, object, graphRef) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      const args2 = this.triple(subject, predicate, object);
      if (!graphRef)
        return this.parameterError("Quad takes four parameters, the last should be a graph filter");
      this.cursor["@type"] = "AddedQuad";
      this.cursor.graph = this.cleanGraph(graphRef);
      return this;
    };
    WOQLQuery2.prototype.removed_quad = function(subject, predicate, object, graphRef) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      const args2 = this.triple(subject, predicate, object);
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
      const args2 = this.triple(subject, predicate, object);
      this.cursor["@type"] = "DeleteTriple";
      return this.updated();
    };
    WOQLQuery2.prototype.add_triple = function(subject, predicate, object) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      const args2 = this.triple(subject, predicate, object);
      this.cursor["@type"] = "AddTriple";
      return this.updated();
    };
    WOQLQuery2.prototype.delete_quad = function(subject, predicate, object, graphRef) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      const args2 = this.triple(subject, predicate, object);
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
      const args2 = this.triple(subject, predicate, object);
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
    WOQLQuery2.prototype.plus = function(...args2) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Plus";
      this.cursor.left = this.arop(args2.shift());
      if (args2.length > 1) {
        this.cursor.right = this.jobj(new WOQLQuery2().plus(...args2.map(this.arop)));
      } else {
        this.cursor.right = this.arop(args2[0]);
      }
      return this;
    };
    WOQLQuery2.prototype.minus = function(...args2) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Minus";
      this.cursor.left = this.arop(args2.shift());
      if (args2.length > 1) {
        this.cursor.right = this.jobj(new WOQLQuery2().minus(...args2.map(this.arop)));
      } else {
        this.cursor.right = this.arop(args2[0]);
      }
      return this;
    };
    WOQLQuery2.prototype.times = function(...args2) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Times";
      this.cursor.left = this.arop(args2.shift());
      if (args2.length > 1) {
        this.cursor.right = this.jobj(new WOQLQuery2().times(...args2.map(this.arop)));
      } else {
        this.cursor.right = this.arop(args2[0]);
      }
      return this;
    };
    WOQLQuery2.prototype.divide = function(...args2) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Divide";
      this.cursor.left = this.arop(args2.shift());
      if (args2.length > 1) {
        this.cursor.right = this.jobj(new WOQLQuery2().divide(...args2.map(this.arop)));
      } else {
        this.cursor.right = this.arop(args2[0]);
      }
      return this;
    };
    WOQLQuery2.prototype.div = function(...args2) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Div";
      this.cursor.left = this.arop(args2.shift());
      if (args2.length > 1) {
        this.cursor.right = this.jobj(new WOQLQuery2().div(...args2.map(this.arop)));
      } else {
        this.cursor.right = this.arop(args2[0]);
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
    WOQLQuery2.prototype.dot = function(document2, field, value) {
      if (this.cursor["@type"])
        this.wrapCursorWithAnd();
      this.cursor["@type"] = "Dot";
      this.cursor.document = this.expandValueVariable(document2);
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
    var typedef2 = require_typedef();
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

// node_modules/@terminusdb/terminusdb-client/lib/woql.js
var require_woql = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/woql.js"(exports, module) {
    var WOQLQuery = require_woqlBuilder();
    var WOQLLibrary = require_woqlLibrary();
    var {
      Vars,
      Var,
      Doc,
      VarsUnique,
      VarUnique,
      SetVarsUniqueCounter
    } = require_woqlDoc();
    var typedef = require_typedef();
    var WOQLClient = require_woqlClient();
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
    WOQL.read_object = function(IRI, output, formatObj) {
      return new WOQLQuery().read_document(IRI, output, formatObj);
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
    WOQL.plus = function(...args2) {
      return new WOQLQuery().plus(...args2);
    };
    WOQL.minus = function(...args2) {
      return new WOQLQuery().minus(...args2);
    };
    WOQL.times = function(...args2) {
      return new WOQLQuery().times(...args2);
    };
    WOQL.divide = function(...args2) {
      return new WOQLQuery().divide(...args2);
    };
    WOQL.div = function(...args2) {
      return new WOQLQuery().div(...args2);
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
    WOQL.client = function(client) {
      if (client)
        this._client = client;
      return this._client;
    };
    WOQL.Vars = function(...varNames) {
      return new Vars(...varNames);
    };
    WOQL.VarsUnique = function(...varNames) {
      return new VarsUnique(...varNames);
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
    WOQL.dot = function(document2, field, value) {
      return new WOQLQuery().dot(document2, field, value);
    };
    module.exports = WOQL;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/woqlClient.js
var require_woqlClient = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/woqlClient.js"(exports2, module2) {
    var FormData2 = require_form_data();
    var fs = require("fs");
    var { Buffer: Buffer2 } = require("buffer");
    var typedef2 = require_typedef();
    var CONST = require_const();
    var DispatchRequest = require_dispatchRequest();
    var ErrorMessage = require_errorMessage();
    var ConnectionConfig = require_connectionConfig();
    var WOQL2 = require_woql();
    var WOQLQuery2 = require_woqlCore();
    var WOQLClient2 = class {
      connectionConfig = null;
      databaseList = [];
      organizationList = [];
      /**
      * @constructor
      * @param {string} serverUrl - the terminusdb server url
      * @param {typedef.ParamsObj} [params] - an object with the connection parameters
      * @example
      * //to connect with your local terminusDB
      * const client = new TerminusClient.WOQLClient(SERVER_URL,{user:"admin",key:"myKey"})
      * async function getSchema() {
      *      client.db("test")
      *      client.checkout("dev")
      *      const schema = await client.getSchema()
      * }
      * //The client has an internal state which defines what
      * //organization / database / repository / branch / ref it is currently attached to
      *
      * //to connect with your TerminusDB Cloud Instance
      * const client = new TerminusClient.WOQLClient('SERVER_CLOUD_URL/mycloudTeam',
      *                      {user:"myemail@something.com", organization:'mycloudTeam'})
      *
      * client.setApiKey(MY_ACCESS_TOKEN)
      *
      * //to get the list of all organization's databases
      * async function callGetDatabases(){
      *      const dbList = await client.getDatabases()
      *      console.log(dbList)
      * }
      *
      * async function getSchema() {
      *      client.db("test")
      *      client.checkout("dev")
      *      const schema = await client.getSchema()
      * }
      */
      constructor(serverUrl, params) {
        this.connectionConfig = new ConnectionConfig(serverUrl, params);
      }
    };
    WOQLClient2.prototype.setApiKey = function(accessToken) {
      const currentAuth = this.connectionConfig.localAuth() || {};
      currentAuth.key = accessToken;
      currentAuth.type = "apikey";
      this.connectionConfig.setLocalAuth(currentAuth);
    };
    WOQLClient2.prototype.customHeaders = function(customHeaders) {
      if (customHeaders)
        this._customHeaders = customHeaders;
      else
        return this._customHeaders;
    };
    WOQLClient2.prototype.CONST = CONST;
    WOQLClient2.prototype.copy = function() {
      const other = new WOQLClient2(this.server());
      other.connectionConfig = this.connectionConfig.copy();
      other.databaseList = this.databaseList;
      return other;
    };
    WOQLClient2.prototype.server = function() {
      return this.connectionConfig.serverURL();
    };
    WOQLClient2.prototype.api = function() {
      return this.connectionConfig.apiURL();
    };
    WOQLClient2.prototype.organization = function(orgId) {
      if (typeof orgId !== "undefined") {
        this.connectionConfig.setOrganization(orgId);
        this.databases([]);
      }
      return this.connectionConfig.organization();
    };
    WOQLClient2.prototype.hasDatabase = async function(orgName, dbName) {
      const dbCheckUrl = `${this.connectionConfig.apiURL()}db/${orgName}/${dbName}`;
      return new Promise((resolve, reject) => {
        this.dispatch(CONST.HEAD, dbCheckUrl).then((req) => {
          resolve(true);
        }).catch((err) => {
          if (err.status === 404) {
            resolve(false);
          } else {
            reject(err);
          }
        });
      });
    };
    WOQLClient2.prototype.getDatabases = async function() {
      if (!this.connectionConfig.organization()) {
        throw new Error("You need to set the organization name");
      }
      await this.getUserOrganizations();
      const dbs = this.userOrganizations().find(
        (element) => element.name === this.connectionConfig.organization()
      );
      const dbList = dbs && dbs.databases ? dbs.databases : [];
      this.databases(dbList);
      return dbList;
    };
    WOQLClient2.prototype.databases = function(dbList) {
      if (dbList)
        this.databaseList = dbList;
      return this.databaseList || [];
    };
    WOQLClient2.prototype.user = function() {
      return this.connectionConfig.user();
    };
    WOQLClient2.prototype.userOrganization = function() {
      return this.user();
    };
    WOQLClient2.prototype.databaseInfo = function(dbName) {
      const database = this.databases().find((element) => element.name === dbName);
      return database || {};
    };
    WOQLClient2.prototype.db = function(dbId) {
      if (typeof dbId !== "undefined") {
        this.connectionConfig.setDB(dbId);
      }
      return this.connectionConfig.dbid;
    };
    WOQLClient2.prototype.setSystemDb = function() {
      this.db(this.connectionConfig.system_db);
    };
    WOQLClient2.prototype.repo = function(repoId) {
      if (typeof repoId !== "undefined") {
        this.connectionConfig.setRepo(repoId);
      }
      return this.connectionConfig.repo();
    };
    WOQLClient2.prototype.checkout = function(branchId) {
      if (typeof branchId !== "undefined") {
        this.connectionConfig.setBranch(branchId);
      }
      return this.connectionConfig.branch();
    };
    WOQLClient2.prototype.ref = function(commitId) {
      if (typeof commitId !== "undefined") {
        this.connectionConfig.setRef(commitId);
      }
      return this.connectionConfig.ref();
    };
    WOQLClient2.prototype.localAuth = function(newCredential) {
      if (typeof newCredential !== "undefined") {
        this.connectionConfig.setLocalAuth(newCredential);
      }
      return this.connectionConfig.localAuth();
    };
    WOQLClient2.prototype.local_auth = WOQLClient2.prototype.localAuth;
    WOQLClient2.prototype.remoteAuth = function(newCredential) {
      if (typeof newCredential !== "undefined") {
        this.connectionConfig.setRemoteAuth(newCredential);
      }
      return this.connectionConfig.remoteAuth();
    };
    WOQLClient2.prototype.remote_auth = WOQLClient2.prototype.remoteAuth;
    WOQLClient2.prototype.author = function() {
      return this.connectionConfig.user();
    };
    WOQLClient2.prototype.set = function(params) {
      this.connectionConfig.update(params);
    };
    WOQLClient2.prototype.resource = function(resourceType, resourceId) {
      let base = `${this.organization()}/${this.db()}/`;
      if (resourceType === "db")
        return base;
      if (resourceType === "meta")
        return `${base}_meta`;
      base += `${this.repo()}`;
      if (resourceType === "repo")
        return base;
      if (resourceType === "commits")
        return `${base}/_commits`;
      const resourceIdValue = resourceId || (resourceType === "ref" ? this.ref() : this.checkout());
      if (resourceType === "branch")
        return `${base}/branch/${resourceIdValue}`;
      if (resourceType === "ref")
        return `${base}/commit/${resourceIdValue}`;
    };
    WOQLClient2.prototype.connect = function(params) {
      if (params)
        this.connectionConfig.update(params);
      return this.dispatch(CONST.GET, this.connectionConfig.apiURLInfo()).then((response) => response);
    };
    WOQLClient2.prototype.createDatabase = function(dbId, dbDetails, orgId) {
      if (orgId)
        this.organization(orgId);
      if (dbId) {
        this.db(dbId);
        return this.dispatch(CONST.POST, this.connectionConfig.dbURL(), dbDetails);
      }
      const errmsg = `Create database parameter error - you must specify a valid database id  - ${dbId} is invalid`;
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.CREATE_DATABASE, errmsg))
      );
    };
    WOQLClient2.prototype.updateDatabase = function(dbDoc) {
      const dbid = dbDoc.id || this.db();
      this.organization(dbDoc.organization || this.organization());
      if (dbid) {
        this.db(dbid);
        return this.dispatch(CONST.PUT, this.connectionConfig.dbURL(), dbDoc);
      }
      const errmsg = `Update database error - you must specify a valid database id - ${dbid} is invalid`;
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.UPDATE_DATABASE, errmsg))
      );
    };
    WOQLClient2.prototype.deleteDatabase = function(dbId, orgId, force) {
      const orgIdValue = orgId || this.organization();
      this.organization(orgIdValue);
      const payload = force ? { force: true } : null;
      if (dbId && this.db(dbId)) {
        return this.dispatch(CONST.DELETE, this.connectionConfig.dbURL(), payload);
      }
      const errmsg = `Delete database parameter error - you must specify a valid database id  - ${dbId} is invalid`;
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.DELETE, errmsg))
      );
    };
    WOQLClient2.prototype.getTriples = function(graphType) {
      if (graphType) {
        return this.dispatch(
          CONST.GET,
          this.connectionConfig.triplesURL(graphType)
        );
      }
      const errmsg = "Get triples parameter error - you must specify a valid graph type (inference, instance, schema), and graph id";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.GET, errmsg))
      );
    };
    WOQLClient2.prototype.updateTriples = function(graphType, turtle, commitMsg) {
      if (commitMsg && turtle && graphType) {
        const commit = this.generateCommitInfo(commitMsg);
        commit.turtle = turtle;
        return this.dispatch(
          CONST.UPDATE_TRIPLES,
          this.connectionConfig.triplesURL(graphType),
          commit
        );
      }
      const errmsg = "Update triples parameter error - you must specify a valid graph id, graph type, turtle contents and commit message";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.UPDATE_TRIPLES, errmsg))
      );
    };
    WOQLClient2.prototype.insertTriples = function(graphType, turtle, commitMsg) {
      if (commitMsg && turtle && graphType) {
        const commit = this.generateCommitInfo(commitMsg);
        commit.turtle = turtle;
        return this.dispatch(
          CONST.INSERT_TRIPLES,
          this.connectionConfig.triplesURL(graphType),
          commit
        );
      }
      const errmsg = "Update triples parameter error - you must specify a valid graph id, graph type, turtle contents and commit message";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.INSERT_TRIPLES, errmsg))
      );
    };
    WOQLClient2.prototype.message = function(message, pathname) {
      let url = this.api();
      url += pathname ? this.api() + pathname : "message";
      return this.dispatch(CONST.GET, url, message).then((response) => response);
    };
    WOQLClient2.prototype.action = function(actionName, payload) {
      const url = `${this.api()}action/${actionName}`;
      return this.dispatch(CONST.ACTION, url, payload).then((response) => response);
    };
    WOQLClient2.prototype.info = function() {
      const url = `${this.api()}info`;
      return this.dispatch(CONST.GET, url).then((response) => response);
    };
    function getResourceObjects(queryObject, result_array) {
      if (queryObject instanceof Array) {
        for (let i = 0; i < queryObject.length; i += 1) {
          getResourceObjects(queryObject[i], result_array);
        }
      } else {
        const keys = Object.keys(queryObject);
        for (let i = 0; i < keys.length; i += 1) {
          if (keys[i] === "resource") {
            if (queryObject[keys[i]]["@type"] && queryObject[keys[i]]["@type"] === "QueryResource") {
              result_array.push(queryObject[keys[i]]);
            }
          }
          if (queryObject[keys[i]] instanceof Object || queryObject[keys[i]] instanceof Array) {
            getResourceObjects(queryObject[keys[i]], result_array);
          }
        }
      }
    }
    WOQLClient2.prototype.query = function(woql, commitMsg, allWitnesses, lastDataVersion = "", getDataVersion = false, resources = []) {
      allWitnesses = allWitnesses || false;
      commitMsg = commitMsg || "Commit generated with javascript client without message";
      const providedResourcesLookupMap = (resources ?? []).reduce((map, res) => ({ ...map, [res.filename.split("/").pop()]: res.data }), {});
      if (woql?.json && (!woql.containsUpdate() || commitMsg)) {
        const doql = woql.containsUpdate() ? this.generateCommitInfo(commitMsg) : {};
        doql.query = woql.json();
        let postBody;
        const resourceObjects = [];
        getResourceObjects(doql.query, resourceObjects);
        if (resourceObjects.length > 0) {
          const formData = new FormData2();
          resourceObjects.forEach((resourceObject) => {
            const providedResourceInsteadOfFile = typeof resourceObject.source.post === "string" ? providedResourcesLookupMap?.[resourceObject.source.post.split("/").pop()] : void 0;
            const fileName = resourceObject.source.post.split("/").pop();
            if (providedResourceInsteadOfFile) {
              formData.append("file", Buffer2.from(providedResourceInsteadOfFile), { filename: fileName, contentType: "application/csv" });
            } else {
              formData.append("file", fs.createReadStream(resourceObject.source.post));
            }
            resourceObject.source.post = fileName;
          });
          formData.append("payload", Buffer2.from(JSON.stringify(doql)), { filename: "body.json", contentType: "application/json" });
          if (formData.getHeaders) {
            this.customHeaders(formData.getHeaders());
          } else {
            this.customHeaders({ "Content-Type": "multipart/form-data" });
          }
          postBody = formData;
        } else {
          postBody = doql;
        }
        if (allWitnesses)
          doql.all_witnesses = true;
        if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
          this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
        }
        return this.dispatch(CONST.WOQL_QUERY, this.connectionConfig.queryURL(), postBody, getDataVersion);
      }
      let errmsg = "WOQL query parameter error";
      if (woql && woql.json && woql.containsUpdate() && !commitMsg) {
        errmsg += " - you must include a textual commit message to perform this update";
      } else {
        errmsg += " - you must specify a valid WOQL Query";
      }
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.WOQL_QUERY, errmsg))
      );
    };
    WOQLClient2.prototype.branch = function(newBranchId, isEmpty) {
      if (newBranchId) {
        let source = this.ref() ? { origin: `${this.organization()}/${this.db()}/${this.repo()}/commit/${this.ref()}` } : {
          origin: `${this.organization()}/${this.db()}/${this.repo()}/branch/${this.checkout()}`
        };
        if (isEmpty && isEmpty === true) {
          source = {};
        }
        return this.dispatch(CONST.BRANCH, this.connectionConfig.branchURL(newBranchId), source);
      }
      const errmsg = "Branch parameter error - you must specify a valid new branch id";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.BRANCH, errmsg)));
    };
    WOQLClient2.prototype.squashBranch = function(branchId, commitMsg) {
      if (commitMsg && branchId) {
        const commit = this.generateCommitInfo(commitMsg);
        return this.dispatch(
          CONST.SQUASH_BRANCH,
          this.connectionConfig.squashBranchURL(branchId),
          commit
        );
      }
      const errmsg = "Branch parameter error - you must specify a valid new branch id and a commit message";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.SQUASH_BRANCH, errmsg))
      );
    };
    WOQLClient2.prototype.resetBranch = function(branchId, commitId) {
      if (commitId && branchId) {
        return this.dispatch(
          CONST.RESET_BRANCH,
          this.connectionConfig.resetBranchUrl(branchId),
          { commit_descriptor: commitId }
        );
      }
      const errmsg = "Branch parameter error - you must specify a valid new branch id and a commit message";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.RESET_BRANCH, errmsg))
      );
    };
    WOQLClient2.prototype.optimizeBranch = function(branchId) {
      if (branchId) {
        return this.dispatch(
          CONST.OPTIMIZE_SYSTEM,
          this.connectionConfig.optimizeBranchUrl(branchId),
          {}
        );
      }
      const errmsg = "Branch parameter error - you must specify a valid branch id";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.BRANCH, errmsg)));
    };
    WOQLClient2.prototype.deleteBranch = function(branchId) {
      if (branchId) {
        return this.dispatch(CONST.DELETE, this.connectionConfig.branchURL(branchId));
      }
      const errmsg = "Branch parameter error - you must specify a valid new branch id";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.BRANCH, errmsg)));
    };
    WOQLClient2.prototype.pull = function(remoteSourceRepo) {
      const rc_args = this.prepareRevisionControlArgs(remoteSourceRepo);
      if (rc_args && rc_args.remote && rc_args.remote_branch) {
        return this.dispatch(CONST.PULL, this.connectionConfig.pullURL(), rc_args);
      }
      const errmsg = "Pull parameter error - you must specify a valid remote source and branch to pull from";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.PULL, errmsg)));
    };
    WOQLClient2.prototype.fetch = function(remoteId) {
      return this.dispatch(CONST.FETCH, this.connectionConfig.fetchURL(remoteId));
    };
    WOQLClient2.prototype.push = function(remoteTargetRepo) {
      const rc_args = this.prepareRevisionControlArgs(remoteTargetRepo);
      if (rc_args && rc_args.remote && rc_args.remote_branch) {
        return this.dispatch(CONST.PUSH, this.connectionConfig.pushURL(), rc_args);
      }
      const errmsg = "Push parameter error - you must specify a valid remote target";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.PUSH, errmsg)));
    };
    WOQLClient2.prototype.rebase = function(rebaseSource) {
      const rc_args = this.prepareRevisionControlArgs(rebaseSource);
      if (rc_args && rc_args.rebase_from) {
        return this.dispatch(CONST.REBASE, this.connectionConfig.rebaseURL(), rc_args);
      }
      const errmsg = "Rebase parameter error - you must specify a valid rebase source to rebase from";
      return Promise.reject(
        new Error(ErrorMessage.getInvalidParameterMessage(CONST.REBASE, errmsg))
      );
    };
    WOQLClient2.prototype.reset = function(commitPath) {
      return this.dispatch(CONST.RESET, this.connectionConfig.resetURL(), {
        commit_descriptor: commitPath
      });
    };
    WOQLClient2.prototype.createRemote = function(remoteName, remoteLocation) {
      if (!remoteName || typeof remoteName !== "string") {
        const errmsg = "Create remote parameter error - you must specify a valid remote name";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.CREATE_REMOTE, errmsg))
        );
      }
      if (!remoteLocation || typeof remoteLocation !== "string") {
        const errmsg = "Create remote parameter error - you must specify a valid remote location URL";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.CREATE_REMOTE, errmsg))
        );
      }
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.remoteURL(),
        { remote_name: remoteName, remote_location: remoteLocation }
      );
    };
    WOQLClient2.prototype.getRemote = function(remoteName) {
      if (!remoteName || typeof remoteName !== "string") {
        const errmsg = "Get remote parameter error - you must specify a valid remote name";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.GET_REMOTE, errmsg))
        );
      }
      const url = `${this.connectionConfig.remoteURL()}?remote_name=${encodeURIComponent(remoteName)}`;
      return this.dispatch(CONST.GET, url);
    };
    WOQLClient2.prototype.updateRemote = function(remoteName, remoteLocation) {
      if (!remoteName || typeof remoteName !== "string") {
        const errmsg = "Update remote parameter error - you must specify a valid remote name";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.UPDATE_REMOTE, errmsg))
        );
      }
      if (!remoteLocation || typeof remoteLocation !== "string") {
        const errmsg = "Update remote parameter error - you must specify a valid remote location URL";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.UPDATE_REMOTE, errmsg))
        );
      }
      return this.dispatch(
        CONST.PUT,
        this.connectionConfig.remoteURL(),
        { remote_name: remoteName, remote_location: remoteLocation }
      );
    };
    WOQLClient2.prototype.deleteRemote = function(remoteName) {
      if (!remoteName || typeof remoteName !== "string") {
        const errmsg = "Delete remote parameter error - you must specify a valid remote name";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.DELETE_REMOTE, errmsg))
        );
      }
      const url = `${this.connectionConfig.remoteURL()}?remote_name=${encodeURIComponent(remoteName)}`;
      return this.dispatch(CONST.DELETE, url);
    };
    WOQLClient2.prototype.clonedb = function(cloneSource, newDbId, orgId) {
      orgId = orgId || this.user_organization();
      this.organization(orgId);
      const rc_args = this.prepareRevisionControlArgs(cloneSource);
      if (newDbId && rc_args && rc_args.remote_url) {
        return this.dispatch(CONST.CLONE, this.connectionConfig.cloneURL(newDbId), rc_args);
      }
      const errmsg = "Clone parameter error - you must specify a valid id for the cloned database";
      return Promise.reject(new Error(ErrorMessage.getInvalidParameterMessage(CONST.BRANCH, errmsg)));
    };
    WOQLClient2.prototype.dispatch = function(action, apiUrl, payload, getDataVersion, compress = false) {
      if (!apiUrl) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              action,
              this.connectionConfig.connection_error
            )
          )
        );
      }
      return DispatchRequest(
        apiUrl,
        action,
        payload,
        this.localAuth(),
        this.remoteAuth(),
        this.customHeaders(),
        getDataVersion,
        compress
      );
    };
    WOQLClient2.prototype.generateCommitInfo = function(msg, author) {
      if (!author) {
        author = this.author();
      }
      const commitInfo = { commit_info: { author, message: msg } };
      return commitInfo;
    };
    WOQLClient2.prototype.generateCommitDescriptor = function(commitId) {
      const cd = this.connectionConfig.commitDescriptorUrl(commitId);
      const ci = { commit_descriptor: cd };
      return ci;
    };
    WOQLClient2.prototype.prepareRevisionControlArgs = function(rc_args) {
      if (!rc_args || typeof rc_args !== "object")
        return false;
      if (!rc_args.author)
        rc_args.author = this.author();
      return rc_args;
    };
    WOQLClient2.prototype.addDocument = function(json2, params, dbId, message = "add a new document", lastDataVersion = "", getDataVersion = false, compress = false) {
      if (dbId) {
        this.db(dbId);
      }
      if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
        this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
      }
      const docParams = params || {};
      docParams.author = this.author();
      docParams.message = message;
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.documentURL(docParams),
        json2,
        getDataVersion,
        compress
      );
    };
    WOQLClient2.prototype.queryDocument = function(query, params, dbId, branch, lastDataVersion = "", getDataVersion = false) {
      if (dbId) {
        this.db(dbId);
      }
      if (branch) {
        this.checkout(branch);
      }
      if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
        this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
      }
      return this.dispatch(
        CONST.QUERY_DOCUMENT,
        this.connectionConfig.documentURL(params),
        query,
        getDataVersion
      );
    };
    WOQLClient2.prototype.getDocument = function(params, dbId, branch, lastDataVersion = "", getDataVersion = false, query = void 0) {
      if (dbId) {
        this.db(dbId);
      }
      if (branch) {
        this.checkout(branch);
      }
      if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
        this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
      }
      let queryDoc;
      if (query) {
        queryDoc = query;
      } else if (params && typeof params === "object" && params.query) {
        queryDoc = { query: params.query };
        delete params.query;
      }
      if (queryDoc) {
        return this.dispatch(
          CONST.QUERY_DOCUMENT,
          this.connectionConfig.documentURL(params),
          queryDoc,
          getDataVersion
        );
      }
      return this.dispatch(CONST.GET, this.connectionConfig.documentURL(params), {}, getDataVersion);
    };
    WOQLClient2.prototype.updateDocument = function(json2, params, dbId, message = "update document", lastDataVersion = "", getDataVersion = false, compress = false, create = false) {
      const docParams = params || {};
      docParams.author = this.author();
      docParams.message = message;
      if (create) {
        docParams.create = create;
      }
      if (dbId) {
        this.db(dbId);
      }
      if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
        this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
      }
      return this.dispatch(
        CONST.PUT,
        this.connectionConfig.documentURL(docParams),
        json2,
        getDataVersion,
        compress
      );
    };
    WOQLClient2.prototype.deleteDocument = function(params, dbId, message = "delete document", lastDataVersion = "", getDataVersion = false) {
      const docParams = params || {};
      let payload = null;
      if (Array.isArray(params.id)) {
        payload = params.id;
        delete docParams.id;
      }
      docParams.author = this.author();
      docParams.message = message;
      if (dbId) {
        this.db(dbId);
      }
      if (typeof lastDataVersion === "string" && lastDataVersion !== "") {
        this.customHeaders({ "TerminusDB-Data-Version": lastDataVersion });
      }
      return this.dispatch(
        CONST.DELETE,
        this.connectionConfig.documentURL(docParams),
        payload,
        getDataVersion
      );
    };
    WOQLClient2.prototype.getSchemaFrame = function(type, dbId) {
      let params;
      if (type)
        params = { type };
      if (dbId) {
        this.db(dbId);
      }
      return this.dispatch(CONST.GET, this.connectionConfig.jsonSchemaURL(params));
    };
    WOQLClient2.prototype.getSchema = function(dbId, branch) {
      const params = { graph_type: "schema", as_list: true };
      return this.getDocument(params, dbId, branch);
    };
    WOQLClient2.prototype.getClasses = function(dbId) {
      const params = { graph_type: "schema", as_list: true, type: "sys:Class" };
      return this.getDocument(params, dbId);
    };
    WOQLClient2.prototype.getEnums = function(dbId) {
      const params = { graph_type: "schema", as_list: true, type: "sys:Enum" };
      return this.getDocument(params, dbId);
    };
    WOQLClient2.prototype.getClassDocuments = function(dbId) {
      const params = { graph_type: "schema", as_list: true, type: "sys:Class" };
      return this.getDocument(params, dbId).then((result) => {
        let documents = [];
        if (result) {
          documents = result.filter((item) => !item["@subdocument"] && !item["@abstract"]);
        }
        return documents;
      });
    };
    WOQLClient2.prototype.getBranches = function(dbId) {
      const params = { type: "Branch", as_list: true };
      const branch = this.checkout();
      return this.getDocument(params, dbId, "_commits").then((result) => {
        const branchesObj = {};
        if (result) {
          result.forEach((item) => {
            branchesObj[item.name] = item;
          });
        }
        this.checkout(branch);
        return branchesObj;
      });
    };
    WOQLClient2.prototype.getCommitsLog = function(start = 0, count = 1) {
      return this.dispatch(
        CONST.GET,
        `${this.connectionConfig.log()}?start=${start}&count=${count}`
      );
    };
    WOQLClient2.prototype.getPrefixes = function(dbId) {
      if (dbId)
        this.db(dbId);
      return this.dispatch(
        CONST.GET,
        this.connectionConfig.prefixesURL()
      );
    };
    WOQLClient2.prototype.getUserOrganizations = function() {
      return this.dispatch(
        CONST.GET,
        this.connectionConfig.userOrganizationsURL()
      ).then((response) => {
        const orgList = Array.isArray(response) ? response : [];
        this.userOrganizations(orgList);
        return orgList;
      });
    };
    WOQLClient2.prototype.userOrganizations = function(orgList) {
      if (orgList)
        this.organizationList = orgList;
      return this.organizationList || [];
    };
    WOQLClient2.prototype.patch = function(before, patch) {
      if (typeof before !== "object" || typeof patch !== "object") {
        const errmsg = '"before" or "after" parameter error - you must specify a valid before and after json document';
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.PATCH, errmsg))
        );
      }
      const payload = { before, patch };
      return this.dispatch(
        CONST.POST,
        `${this.connectionConfig.apiURL()}patch`,
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.patchResource = function(patch, message) {
      if (!Array.isArray(patch)) {
        const errmsg = '"patch" parameter error - you must specify a valid patch document';
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.PATCH, errmsg))
        );
      }
      const payload = { patch, author: this.author(), message };
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.patchURL(),
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.getJSONDiff = function(before, after, options) {
      if (typeof before !== "object" || typeof after !== "object") {
        const errmsg = '"before" or "after" parameter error - you must specify a valid before or after json document';
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.GET_DIFF, errmsg))
        );
      }
      const opt = typeof options === "undefined" ? {} : options;
      const payload = { before, after, ...opt };
      return this.dispatch(
        CONST.POST,
        `${this.connectionConfig.apiURL()}diff`,
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.getVersionObjectDiff = function(dataVersion, jsonObject, id, options) {
      if (typeof jsonObject !== "object" || typeof dataVersion !== "string" || typeof id !== "string") {
        const errmsg = "Parameters error - you must specify a valid jsonObject document, a valid branch or commit and a valid id";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.GET_DIFF, errmsg))
        );
      }
      const opt = options || {};
      const payload = {
        after: jsonObject,
        before_data_version: dataVersion,
        id,
        ...opt
      };
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.diffURL(),
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.getVersionDiff = function(beforeVersion, afterVersion, id, options) {
      if (typeof beforeVersion !== "string" || typeof afterVersion !== "string") {
        const errmsg = "Error, you have to provide a beforeVersion and afterVersion input";
        return Promise.reject(
          new Error(ErrorMessage.getInvalidParameterMessage(CONST.GET_DIFF, errmsg))
        );
      }
      const opt = options || {};
      const payload = {
        before_data_version: beforeVersion,
        after_data_version: afterVersion,
        ...opt
      };
      if (id) {
        payload.document_id = id;
      }
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.diffURL(),
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.apply = function(beforeVersion, afterVersion, message, matchFinalState, options) {
      const opt = options || {};
      const commitMsg = this.generateCommitInfo(message);
      const payload = {
        before_commit: beforeVersion,
        after_commit: afterVersion,
        ...commitMsg,
        ...opt
      };
      if (matchFinalState) {
        payload.match_final_state = matchFinalState;
      }
      return this.dispatch(
        CONST.POST,
        this.connectionConfig.applyURL(),
        payload
      ).then((response) => response);
    };
    WOQLClient2.prototype.getDocumentHistory = function(id, historyParams) {
      const params = historyParams || {};
      params.id = id;
      return this.dispatch(
        CONST.GET,
        this.connectionConfig.docHistoryURL(params)
      ).then((response) => response);
    };
    WOQLClient2.prototype.sendCustomRequest = function(requestType, customRequestURL, payload) {
      return this.dispatch(
        requestType,
        customRequestURL,
        payload
      ).then((response) => response);
    };
    module2.exports = WOQLClient2;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/terminusRule.js
var require_terminusRule = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/terminusRule.js"(exports, module) {
    var UTILS = require_utils();
    function TerminusRule() {
    }
    TerminusRule.prototype.literal = function(tf) {
      if (typeof tf === "undefined") {
        return this.pattern.literal;
      }
      this.pattern.literal = tf;
      return this;
    };
    TerminusRule.prototype.type = function(...list) {
      if (typeof list === "undefined" || list.length === 0) {
        return this.pattern.type;
      }
      this.pattern.type = list;
      return this;
    };
    TerminusRule.prototype.scope = function(scope) {
      if (typeof scope === "undefined") {
        return this.pattern.scope;
      }
      this.pattern.scope = scope;
      return this;
    };
    TerminusRule.prototype.value = function(...val) {
      if (typeof val === "undefined") {
        return this.pattern.value;
      }
      this.pattern.value = val;
      return this;
    };
    TerminusRule.prototype.json = function(mjson) {
      if (!mjson) {
        const njson = {};
        if (this.pattern)
          njson.pattern = this.pattern.json();
        if (this.rule)
          njson.rule = this.rule;
        return njson;
      }
      if (mjson.pattern)
        this.pattern.setPattern(mjson.pattern);
      if (mjson.rule)
        this.rule = mjson.rule;
      return this;
    };
    function TerminusPattern(pattern) {
    }
    TerminusPattern.prototype.setPattern = function(pattern) {
      if (typeof pattern.literal !== "undefined")
        this.literal = pattern.literal;
      if (pattern.type)
        this.type = pattern.type;
      if (pattern.scope)
        this.scope = pattern.scope;
      if (pattern.value)
        this.value = pattern.value;
    };
    TerminusPattern.prototype.json = function() {
      const json2 = {};
      if (typeof this.literal !== "undefined")
        json2.literal = this.literal;
      if (this.type)
        json2.type = this.type;
      if (this.scope)
        json2.scope = this.scope;
      if (this.value)
        json2.value = this.value;
      return json2;
    };
    TerminusPattern.prototype.testBasics = function(scope, value) {
      if (this.scope && scope && this.scope !== scope)
        return false;
      if (this.type) {
        const dt = value["@type"];
        if (!dt || !this.testValue(dt, this.type))
          return false;
      }
      if (typeof this.literal !== "undefined") {
        if (!(this.literal === !(typeof value["@type"] === "undefined")))
          return false;
      }
      if (typeof this.value !== "undefined") {
        if (!this.testValue(value, this.value))
          return false;
      }
      return true;
    };
    TerminusPattern.prototype.testValue = function(value, constraint) {
      if (!value)
        return null;
      const vundertest = value["@value"] ? value["@value"] : value;
      if (typeof constraint === "function")
        return constraint(vundertest);
      if (constraint && !Array.isArray(constraint))
        constraint = [constraint];
      for (let i = 0; i < constraint.length; i++) {
        const nc = constraint[i];
        if (typeof vundertest === "string") {
          if (this.stringMatch(nc, vundertest))
            return true;
        } else if (typeof vundertest === "number") {
          if (this.numberMatch(nc, vundertest))
            return true;
        }
      }
      return false;
    };
    TerminusPattern.prototype.unpack = function(arr, nonstring) {
      let str2 = "";
      if (nonstring) {
        for (let i = 0; i < arr.length; i++) {
          if (typeof arr[i] === "string") {
            str2 += `"${arr[i]}"`;
          } else {
            str2 += arr[i];
          }
          if (i < arr.length - 1)
            str2 += ", ";
        }
      } else {
        str2 = `"${arr.join('","')}"`;
      }
      return str2;
    };
    TerminusPattern.prototype.IDsMatch = function(ida, idb) {
      return UTILS.compareIDs(ida, idb);
    };
    TerminusPattern.prototype.classIDsMatch = function(ida, idb) {
      return this.IDsMatch(ida, idb);
    };
    TerminusPattern.prototype.propertyIDsMatch = function(ida, idb) {
      const match = this.IDsMatch(ida, idb);
      return match;
    };
    TerminusPattern.prototype.rangeIDsMatch = function(ida, idb) {
      return this.IDsMatch(ida, idb);
    };
    TerminusPattern.prototype.valuesMatch = function(vala2, valb2) {
      return vala2 === valb2;
    };
    TerminusPattern.prototype.numberMatch = function(vala, valb) {
      if (typeof vala === "string") {
        try {
          return eval(valb + vala);
        } catch (e) {
          return false;
        }
      }
      return vala === valb;
    };
    TerminusPattern.prototype.stringMatch = function(vala2, valb2) {
      if (vala2.substring(0, 1) === "/") {
        const pat = new RegExp(vala2.substring(1));
        return pat.test(valb2);
      }
      return vala2 === valb2;
    };
    module.exports = { TerminusRule, TerminusPattern };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlRule.js
var require_woqlRule = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlRule.js"(exports2, module2) {
    var TerminusRule2 = require_terminusRule();
    var UTILS2 = require_utils();
    function WOQLRule() {
      TerminusRule2.TerminusRule.call(this);
      this.pattern = new WOQLPattern();
    }
    Object.setPrototypeOf(WOQLRule.prototype, TerminusRule2.TerminusRule.prototype);
    WOQLRule.prototype.setVariables = function(vars) {
      if (vars && vars.length) {
        this.pattern.variables = UTILS2.removeNamespacesFromVariables(vars);
        this.current_variable = this.pattern.variables[this.pattern.variables.length - 1];
      }
      return this;
    };
    WOQLRule.prototype.vars = function(...varlist) {
      return this.setVariables(varlist);
    };
    WOQLRule.prototype.v = function(v) {
      if (v) {
        this.current_variable = UTILS2.removeNamespaceFromVariable(v);
        return this;
      }
      return this.current_variable;
    };
    WOQLRule.prototype.edge = function(source, target) {
      this.scope("edge");
      if (source) {
        const vs = UTILS2.removeNamespaceFromVariable(source);
        this.setVariables([vs]);
        this.pattern.source = vs;
      }
      if (target) {
        const vs = UTILS2.removeNamespaceFromVariable(target);
        if (!source)
          this.setVariables([vs]);
        this.pattern.target = vs;
      }
      return this;
    };
    WOQLRule.prototype.rownum = function(rownum) {
      if (typeof rownum === "undefined")
        return this.pattern.rownum;
      this.pattern.rownum = rownum;
      return this;
    };
    WOQLRule.prototype.in = function(...list) {
      if (this.current_variable) {
        if (!this.pattern.constraints)
          this.pattern.constraints = {};
        if (!this.pattern.constraints[this.current_variable]) {
          this.pattern.constraints[this.current_variable] = [];
        }
        this.pattern.constraints[this.current_variable].push(list);
      }
      return this;
    };
    WOQLRule.prototype.filter = function(tester) {
      if (this.current_variable) {
        if (!this.pattern.constraints)
          this.pattern.constraints = {};
        if (!this.pattern.constraints[this.current_variable]) {
          this.pattern.constraints[this.current_variable] = [];
        }
        this.pattern.constraints[this.current_variable].push(tester);
      }
      return this;
    };
    WOQLRule.prototype.matchRow = function(rules, row2, rownum, action) {
      const matches = [];
      for (let i = 0; i < rules.length; i++) {
        if (action && this.rule && typeof this.rule[action] === "undefined")
          continue;
        if (rules[i].pattern.matchRow(row2, rownum)) {
          matches.push(rules[i]);
        }
      }
      return matches;
    };
    WOQLRule.prototype.matchCell = function(rules, row2, key, rownum, action) {
      const matches = [];
      for (let i = 0; i < rules.length; i++) {
        if (action && this.rule && typeof this.rule[action] === "undefined")
          continue;
        if (rules[i].pattern.matchCell(row2, key, rownum)) {
          matches.push(rules[i]);
        }
      }
      return matches;
    };
    WOQLRule.prototype.matchColumn = function(rules, key, ruleName) {
      const matches = [];
      for (let i = 0; i < rules.length; i++) {
        if (ruleName && this.rule && typeof this.rule[ruleName] === void 0)
          continue;
        const ruleRow = rules[i];
        if (ruleName && ruleRow.rule[ruleName] === void 0) {
          continue;
        }
        if (ruleRow.pattern.matchColumn(key)) {
          matches.push(ruleRow);
        }
      }
      return matches;
    };
    WOQLRule.prototype.matchNode = function(rules, row2, key, nid, action) {
      const matches = [];
      for (let i = 0; i < rules.length; i++) {
        if (action && this.rule && typeof this.rule[action] === "undefined")
          continue;
        if (rules[i].pattern.matchNode(row2, key, nid)) {
          matches.push(rules[i]);
        }
      }
      return matches;
    };
    WOQLRule.prototype.matchPair = function(rules, row2, keya, keyb, action) {
      const matches = [];
      for (let i = 0; i < rules.length; i++) {
        if (action && this.rule && typeof this.rule[action] === "undefined")
          continue;
        if (rules[i].pattern.matchPair(row2, keya, keyb)) {
          matches.push(rules[i]);
        }
      }
      return matches;
    };
    WOQLRule.prototype.matchEdge = WOQLRule.prototype.matchPair;
    function WOQLPattern(pattern) {
      TerminusRule2.TerminusPattern.call(this, pattern);
    }
    Object.setPrototypeOf(WOQLPattern.prototype, TerminusRule2.TerminusPattern.prototype);
    WOQLPattern.prototype.prettyPrint = function() {
      let str2 = `${this.scope}('`;
      if (this.variables) {
        str2 += this.variables.join("', '");
      }
      str2 += "')";
      if (typeof this.literal !== "undefined") {
        str2 += `.literal(${this.literal})`;
      }
      if (typeof this.type !== "undefined") {
        str2 += `.type(${this.unpack(this.type)})`;
      }
      if (typeof this.value !== "undefined") {
        str2 += `.value(${this.unpack(this.value, true)})`;
      }
      for (const v in this.constraints) {
        str2 += `.v('${v}')`;
        for (let i = 0; i < this.constraints[v].length; i++) {
          if (typeof this.constraints[v][i] === "function") {
            str2 += `.filter(${this.constraints[v][i]})`;
          } else {
            str2 += `.in(${json.unpack(this.constraints[v][i])})`;
          }
        }
      }
      return str2;
    };
    WOQLPattern.prototype.matchRow = function(row2, rownum) {
      if (typeof this.rownum !== "undefined" && typeof rownum !== "undefined") {
        if (!this.numberMatch(this.rownum, rownum))
          return false;
      }
      if (this.scope && this.scope !== "row")
        return false;
      if (!this.testVariableConstraints(row2))
        return false;
      return true;
    };
    WOQLPattern.prototype.matchCell = function(row2, key, rownum) {
      if (typeof this.rownum !== "undefined" && typeof rownum !== "undefined") {
        if (!this.numberMatch(this.rownum, rownum))
          return false;
      }
      if (!this.testBasics("column", row2[key]))
        return false;
      if (this.variables && this.variables.length && this.variables.indexOf(key) === -1)
        return false;
      if (!this.testVariableConstraints(row2))
        return false;
      return true;
    };
    WOQLPattern.prototype.matchNode = function(row2, key) {
      if (!this.testBasics("node", row2[key]))
        return false;
      if (this.variables && this.variables.length && this.variables.indexOf(key) === -1)
        return false;
      if (!this.testVariableConstraints(row2))
        return false;
      return true;
    };
    WOQLPattern.prototype.matchColumn = function(key) {
      if (this.scope && this.scope !== "column")
        return false;
      if (this.variables && this.variables.length && this.variables.indexOf(key) === -1)
        return false;
      return true;
    };
    WOQLPattern.prototype.matchPair = function(row2, keya, keyb) {
      if (this.scope && this.scope !== "edge")
        return false;
      if (this.source && this.source !== keya)
        return false;
      if (this.target && this.target !== keyb)
        return false;
      if (!this.testVariableConstraints(row2))
        return false;
      return true;
    };
    WOQLPattern.prototype.testVariableConstraints = function(row2) {
      for (const k in this.constraints) {
        if (!this.testVariableConstraint(k, row2[k]))
          return false;
      }
      return true;
    };
    WOQLPattern.prototype.testVariableConstraint = function(name, val) {
      if (!this.constraints[name])
        return true;
      for (let i = 0; i < this.constraints[name].length; i++) {
        if (!this.testValue(val, this.constraints[name][i])) {
          return false;
        }
      }
      return true;
    };
    WOQLPattern.prototype.setPattern = function(pattern) {
      for (const key in pattern) {
        this[key] = pattern[key];
      }
    };
    WOQLPattern.prototype.json = function() {
      const json2 = {};
      if (this.scope) {
        json2.scope = this.scope;
      }
      if (this.value) {
        json2.value = this.value;
      }
      if (this.rownum)
        json2.rownum = this.rownum;
      if (this.variables)
        json2.variables = this.variables;
      if (this.literal)
        json2.literal = this.literal;
      if (this.type)
        json2.type = this.type;
      if (this.constraints)
        json2.constraints = this.constraints;
      if (this.source)
        json2.source = this.source;
      if (this.target)
        json2.target = this.target;
      return json2;
    };
    module2.exports = { WOQLRule, WOQLPattern };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/viewConfig.js
var require_viewConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/viewConfig.js"(exports2, module2) {
    var { WOQLRule } = require_woqlRule();
    function ViewConfig() {
      this.rules = [];
    }
    ViewConfig.prototype.render = function(func) {
      if (func)
        this.view_render = func;
      return this.view_render;
    };
    ViewConfig.prototype.renderer = function(val) {
      if (val)
        this.view_renderer = val;
      return this.view_renderer;
    };
    ViewConfig.prototype.getRulesJSON = function() {
      const jr = [];
      for (let i = 0; i < this.rules.length; i++) {
        jr.push(this.rules[i].json());
      }
      return jr;
    };
    ViewConfig.prototype.getBasicJSON = function() {
      const jr = {};
      if (this.view_render)
        jr.render = this.view_render;
      if (this.view_renderer)
        jr.renderer = this.view_renderer;
      if (this.vbindings)
        jr.bindings = this.vbindings;
      return jr;
    };
    ViewConfig.prototype.loadBasicJSON = function(json2) {
      if (json2.render)
        this.view_render = json2.view_render;
      if (json2.renderer)
        this.view_renderer = json2.view_renderer;
      if (json2.bindings)
        this.vbindings = json2.bindings;
    };
    ViewConfig.prototype.getBasicprettyPrint = function() {
      let str2 = "";
      if (typeof this.render() !== "undefined") {
        str2 += `view.render(${this.render()})
`;
      }
      if (typeof this.renderer() !== "undefined") {
        str2 += `view.renderer('${this.renderer()}')
`;
      }
      if (typeof this.bindings() !== "undefined") {
        str2 += `view.bindings('${this.bindings()}')
`;
      }
      return str2;
    };
    ViewConfig.prototype.bindings = function(bindings) {
      if (typeof bindings !== "undefined") {
        this.vbindings = bindings;
      }
      return this.vbindings;
    };
    function WOQLViewRule2() {
      WOQLRule.call(this);
      this.rule = {};
    }
    Object.setPrototypeOf(WOQLViewRule2.prototype, WOQLRule.prototype);
    WOQLViewRule2.prototype.prettyPrint = function(type) {
      let str2 = "";
      if (this.pattern) {
        str2 = this.pattern.prettyPrint(type);
      }
      if (typeof this.color() !== "undefined") {
        str2 += `.color([${this.color().join(",")}])`;
      }
      if (typeof this.hidden() !== "undefined") {
        str2 += `.hidden(${this.hidden()})`;
      }
      if (typeof this.size() !== "undefined") {
        str2 += `.size('${this.size()}')`;
      }
      if (typeof this.icon() !== "undefined") {
        str2 += `.icon(${JSON.stringify(this.icon())})`;
      }
      if (typeof this.text() !== "undefined") {
        str2 += `.text(${JSON.stringify(this.text())})`;
      }
      if (typeof this.border() !== "undefined") {
        str2 += `.border(${JSON.stringify(this.border())})`;
      }
      if (typeof this.args() !== "undefined") {
        str2 += `.args(${JSON.stringify(this.args())})`;
      }
      if (typeof this.renderer() !== "undefined") {
        str2 += `.renderer('${this.renderer()}')`;
      }
      if (typeof this.render() !== "undefined") {
        str2 += `.render(${this.render()})`;
      }
      if (typeof this.click() !== "undefined") {
        str2 += `.click(${this.click()})`;
      }
      if (typeof this.hover() !== "undefined") {
        str2 += `.hover(${this.hover()})`;
      }
      return str2;
    };
    WOQLViewRule2.prototype.json = function(mjson) {
      if (!mjson) {
        const json2 = {};
        if (this.pattern)
          json2.pattern = this.pattern.json();
        json2.rule = this.rule;
        return json2;
      }
      this.rule = mjson.rule || {};
      if (mjson.pattern)
        this.pattern.setPattern(mjson.pattern);
      return this;
    };
    WOQLViewRule2.prototype.size = function(size) {
      if (typeof size === "undefined") {
        return this.rule.size;
      }
      this.rule.size = size;
      return this;
    };
    WOQLViewRule2.prototype.color = function(color) {
      if (typeof color === "undefined") {
        return this.rule.color;
      }
      this.rule.color = color;
      return this;
    };
    WOQLViewRule2.prototype.icon = function(json2) {
      if (json2) {
        this.rule.icon = json2;
        return this;
      }
      return this.rule.icon;
    };
    WOQLViewRule2.prototype.text = function(json2) {
      if (json2) {
        this.rule.text = json2;
        return this;
      }
      return this.rule.text;
    };
    WOQLViewRule2.prototype.border = function(json2) {
      if (json2) {
        this.rule.border = json2;
        return this;
      }
      return this.rule.border;
    };
    WOQLViewRule2.prototype.renderer = function(rend) {
      if (typeof rend === "undefined") {
        return this.rule.renderer;
      }
      this.rule.renderer = rend;
      return this;
    };
    WOQLViewRule2.prototype.render = function(func) {
      if (typeof func === "undefined") {
        return this.rule.render;
      }
      this.rule.render = func;
      return this;
    };
    WOQLViewRule2.prototype.click = function(onClick) {
      if (onClick) {
        this.rule.click = onClick;
        return this;
      }
      return this.rule.click;
    };
    WOQLViewRule2.prototype.hover = function(onHover) {
      if (onHover) {
        this.rule.hover = onHover;
        return this;
      }
      return this.rule.hover;
    };
    WOQLViewRule2.prototype.hidden = function(hidden) {
      if (typeof hidden === "undefined") {
        return this.rule.hidden;
      }
      this.rule.hidden = hidden;
      return this;
    };
    WOQLViewRule2.prototype.args = function(args2) {
      if (typeof args2 === "undefined") {
        return this.rule.args;
      }
      this.rule.args = args2;
      return this;
    };
    module2.exports = { WOQLViewRule: WOQLViewRule2, ViewConfig };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlPaging.js
var require_woqlPaging = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlPaging.js"() {
    var WOQLQuery2 = require_woqlCore();
    var UTILS2 = require_utils();
    WOQLQuery2.prototype.getLimit = function() {
      return this.getPagingProperty("limit");
    };
    WOQLQuery2.prototype.setLimit = function(l) {
      return this.setPagingProperty("limit", l);
    };
    WOQLQuery2.prototype.isPaged = function(q) {
      q = q || this.query;
      for (const prop of Object.keys(q)) {
        if (prop === "limit")
          return true;
        if (this.paging_transitive_properties.indexOf(prop) !== -1) {
          return this.isPaged(q[prop][q[prop].length - 1]);
        }
      }
      return false;
    };
    WOQLQuery2.prototype.getPage = function() {
      if (this.isPaged()) {
        const psize = this.getLimit();
        if (this.hasStart()) {
          const s = this.getStart();
          return parseInt(s / psize) + 1;
        }
        return 1;
      }
      return false;
    };
    WOQLQuery2.prototype.setPage = function(pagenum) {
      const pstart = this.getLimit() * (pagenum - 1);
      if (this.hasStart()) {
        this.setStart(pstart);
      } else {
        this.addStart(pstart);
      }
      return this;
    };
    WOQLQuery2.prototype.nextPage = function() {
      return this.setPage(this.getPage() + 1);
    };
    WOQLQuery2.prototype.firstPage = function() {
      return this.setPage(1);
    };
    WOQLQuery2.prototype.previousPage = function() {
      const npage = this.getPage() - 1;
      if (npage > 0)
        this.setPage(npage);
      return this;
    };
    WOQLQuery2.prototype.setPageSize = function(size) {
      this.setPagingProperty("limit", size);
      if (this.hasStart()) {
        this.setStart(0);
      } else {
        this.addStart(0);
      }
      return this;
    };
    WOQLQuery2.prototype.hasSelect = function() {
      return !!this.getPagingProperty("select");
    };
    WOQLQuery2.prototype.getSelectVariables = function(q) {
      q = q || this.query;
      for (const prop of Object.keys(q)) {
        if (prop === "select") {
          const vars = q[prop].slice(0, q[prop].length - 1);
          return vars;
        }
        if (this.paging_transitive_properties.indexOf(prop) !== -1) {
          const val = this.getSelectVariables(q[prop][q[prop].length - 1]);
          if (typeof val !== "undefined") {
            return val;
          }
        }
      }
    };
    WOQLQuery2.prototype.hasStart = function() {
      return typeof this.getPagingProperty("start") !== "undefined";
    };
    WOQLQuery2.prototype.getStart = function() {
      return this.getPagingProperty("start");
    };
    WOQLQuery2.prototype.setStart = function(start) {
      return this.setPagingProperty("start", start);
    };
    WOQLQuery2.prototype.addStart = function(s) {
      if (this.hasStart())
        this.setStart(s);
      else {
        const nq = { start: [s, this.query] };
        this.query = nq;
      }
      return this;
    };
    WOQLQuery2.prototype.getPagingProperty = function(pageprop, q) {
      q = q || this.query;
      for (const prop of Object.keys(q)) {
        if (prop === pageprop)
          return q[prop][0];
        if (this.paging_transitive_properties.indexOf(prop) !== -1) {
          const val = this.getPagingProperty(pageprop, q[prop][q[prop].length - 1]);
          if (typeof val !== "undefined") {
            return val;
          }
        }
      }
    };
    WOQLQuery2.prototype.setPagingProperty = function(pageprop, val, q) {
      q = q || this.query;
      for (const prop of Object.keys(q)) {
        if (prop === pageprop) {
          q[prop][0] = val;
        } else if (this.paging_transitive_properties.indexOf(prop) !== -1) {
          this.setPagingProperty(pageprop, val, q[prop][q[prop].length - 1]);
        }
      }
      return this;
    };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlResult.js
var require_woqlResult = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlResult.js"(exports2, module2) {
    var UTILS2 = require_utils();
    var WOQL2 = require_woql();
    var WOQLQ = require_woqlPaging();
    function WOQLResult(results, query, config) {
      this.bindings = results ? results.bindings : [];
      this.insert_count = results ? results.inserts : 0;
      this.delete_count = results ? results.deletes : 0;
      this.transaction_retry_count = results ? results.transaction_retry_count : 0;
      this.variable_names = results ? results["api:variable_names"] : false;
      this.query = query || WOQL2.query();
      this.cursor = 0;
    }
    WOQLResult.prototype.formatter = function(context) {
      context = context || this.query.getContext();
      const formatted = [];
      if (this.bindings && this.bindings.length > 0) {
        this.bindings.forEach((row2) => {
          const rowObj = {};
          Object.keys(row2).forEach((propName) => {
            const propValue = row2[propName];
            if (typeof propValue === "string") {
              if (propValue !== "system:unknown") {
                rowObj[propName] = UTILS2.shorten(propValue, context);
              } else {
                rowObj[propName] = "";
              }
            } else {
              rowObj[propName] = propValue["@value"];
            }
          });
          formatted.push(rowObj);
        });
      }
      return formatted;
    };
    WOQLResult.prototype.compress = function(context) {
      context = context || this.query.getContext();
      if (context) {
        for (let i = 0; i < this.bindings.length; i++) {
          for (const prop of Object.keys(this.bindings[i])) {
            const nprop = UTILS2.shorten(prop, context);
            let nval = this.bindings[i][prop];
            if (typeof this.bindings[i][prop] === "string") {
              nval = UTILS2.shorten(this.bindings[i][prop], context);
            } else if (Array.isArray(this.bindings[i][prop])) {
              nval = [];
              for (let j = 0; j < this.bindings[i][prop].length; j++) {
                let oval = this.bindings[i][prop][j];
                if (typeof oval === "string")
                  oval = UTILS2.shorten(oval, context);
                else if (Array.isArray(oval)) {
                  const noval = [];
                  for (let k = 0; k < oval.length; k++) {
                    let kval = oval[k];
                    if (typeof kval === "string")
                      kval = UTILS2.shorten(kval, context);
                    noval.push(kval);
                  }
                  oval = noval;
                }
                nval.push(oval);
              }
            }
            delete this.bindings[i][prop];
            this.bindings[i][nprop] = nval;
          }
        }
      }
      return this;
    };
    WOQLResult.prototype.hasBindings = function() {
      if (this.bindings && this.count())
        return true;
      return false;
    };
    WOQLResult.prototype.hasUpdates = function() {
      if (this.inserts() > 0 || this.deletes() > 0)
        return true;
      return false;
    };
    WOQLResult.prototype.getBindings = function() {
      return this.bindings;
    };
    WOQLResult.prototype.rows = function() {
      return this.bindings;
    };
    WOQLResult.prototype.getVariableList = function() {
      if (this.variable_names) {
        return this.variable_names;
      }
      if (this.bindings && this.bindings[0]) {
        return Object.keys(this.bindings[0]);
      }
      return [];
    };
    WOQLResult.prototype.count = function() {
      return this.bindings.length;
    };
    WOQLResult.prototype.inserts = function() {
      return this.insert_count;
    };
    WOQLResult.prototype.deletes = function() {
      return this.delete_count;
    };
    WOQLResult.prototype.first = function() {
      this.cursor = 0;
      return this.bindings[0];
    };
    WOQLResult.prototype.last = function() {
      this.cursor = this.bindings.length - 1;
      return this.bindings[this.bindings.length - 1];
    };
    WOQLResult.prototype.next = function() {
      if (this.cursor >= this.bindings.length)
        return false;
      const res = this.bindings[this.cursor];
      this.cursor++;
      return res;
    };
    WOQLResult.prototype.prev = function() {
      if (this.cursor > 0) {
        this.cursor--;
        return this.bindings[this.cursor];
      }
    };
    WOQLResult.prototype.sort = function(key, asc_or_desc) {
      this.bindings.sort((a, b) => this.compareValues(a[key], b[key], asc_or_desc));
      this;
    };
    WOQLResult.prototype.compareValues = function(a, b, asc_or_desc = "asc") {
      if (!a || !b)
        return 0;
      if (typeof a["@value"] !== "undefined" && typeof b["@value"] !== "undefined") {
        a = a["@value"];
        b = b["@value"];
      }
      if (a > b) {
        return asc_or_desc && asc_or_desc === "asc" ? 1 : -1;
      }
      if (b > a) {
        return asc_or_desc && asc_or_desc === "asc" ? -1 : 1;
      }
    };
    module2.exports = WOQLResult;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlTable.js
var require_woqlTable = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlTable.js"(exports2, module2) {
    var WOQLTableConfig = require_tableConfig();
    var UTILS2 = require_utils();
    var { WOQLRule } = require_woqlRule();
    var WOQLResult = require_woqlResult();
    var WOQLClient2 = require_woqlClient();
    function WOQLTable(client, config) {
      this.client = client;
      this.config = config || new WOQLTableConfig();
      return this;
    }
    WOQLTable.prototype.options = function(config) {
      this.config = config;
      return this;
    };
    WOQLTable.prototype.setResult = function(result) {
      this.result = result;
      return this;
    };
    WOQLTable.prototype.count = function() {
      return this.result.count();
    };
    WOQLTable.prototype.first = function() {
      return this.result.first();
    };
    WOQLTable.prototype.prev = function() {
      return this.result.prev();
    };
    WOQLTable.prototype.next = function() {
      return this.result.next();
    };
    WOQLTable.prototype.canAdvancePage = function() {
      return this.result.count() === this.result.query.getLimit();
    };
    WOQLTable.prototype.canChangePage = function() {
      return this.canAdvancePage() || this.canRetreatPage();
    };
    WOQLTable.prototype.canRetreatPage = function() {
      return this.result.query.getPage() > 1;
    };
    WOQLTable.prototype.getPageSize = function() {
      return this.result.query.getLimit();
    };
    WOQLTable.prototype.setPage = function(l) {
      return this.result.query.setPage(l);
    };
    WOQLTable.prototype.getPage = function() {
      return this.result.query.getPage();
    };
    WOQLTable.prototype.setPageSize = function(l) {
      return this.update(this.result.query.setPageSize(l));
    };
    WOQLTable.prototype.nextPage = function() {
      return this.update(this.result.query.nextPage());
    };
    WOQLTable.prototype.firstPage = function() {
      return this.update(this.result.query.firstPage());
    };
    WOQLTable.prototype.previousPage = function() {
      return this.update(this.result.query.previousPage());
    };
    WOQLTable.prototype.getColumnsToRender = function() {
      if (this.hasColumnOrder()) {
        var cols = this.getColumnOrder();
      } else {
        var cols = this.result.getVariableList();
      }
      const self2 = this;
      return cols ? cols.filter((col) => !self2.hidden(col)) : [];
    };
    WOQLTable.prototype.getColumnHeaderContents = function(colid2) {
      colid2 = UTILS2.removeNamespaceFromVariable(colid2);
      const rules = new WOQLRule().matchColumn(this.config.rules, colid2, "header");
      if (rules.length) {
        const header = rules[rules.length - 1].rule ? rules[rules.length - 1].rule.header : null;
        if (typeof header === "string") {
          return header;
        }
        if (typeof header === "function") {
          return header(colid2);
        }
        return header;
      }
      if (colid2[0] === "_")
        return " ";
      return UTILS2.labelFromVariable(colid2);
    };
    WOQLTable.prototype.hidden = function(col) {
      colid = UTILS2.removeNamespaceFromVariable(col);
      const matched_rules = new WOQLRule().matchColumn(this.config.rules, colid, "hidden");
      if (matched_rules.length) {
        return matched_rules[matched_rules.length - 1].rule.hidden;
      }
      return false;
    };
    WOQLTable.prototype.update = function(nquery) {
      return nquery.execute(this.client).then((results) => {
        const nresult = new WOQLResult(results, nquery);
        this.setResult(nresult);
        if (this.notify)
          this.notify(nresult);
        return nresult;
      });
    };
    WOQLTable.prototype.hasDefinedEvent = function(row2, key, scope, action, rownum) {
      if (scope === "row") {
        var matched_rules = new WOQLRule().matchRow(this.config.rules, row2, rownum, action);
      } else {
        var matched_rules = new WOQLRule().matchCell(this.config.rules, row2, key, rownum, action);
      }
      if (matched_rules && matched_rules.length)
        return true;
      return false;
    };
    WOQLTable.prototype.getDefinedEvent = function(row2, key, scope, action, rownum) {
      if (scope === "row") {
        var matched_rules = new WOQLRule().matchRow(this.config.rules, row2, rownum, action);
      } else {
        var matched_rules = new WOQLRule().matchCell(this.config.rules, row2, key, rownum, action);
      }
      if (Array.isArray(matched_rules) && matched_rules.length > 0) {
        if (matched_rules.length === 1)
          return matched_rules[0].rule[action];
        const findRule = matched_rules.find((item) => item.rule[action] !== void 0 || item.rule[action] === false);
        return findRule && findRule.rule ? findRule.rule[action] : void 0;
      }
      return void 0;
    };
    WOQLTable.prototype.getRowClick = function(row2) {
      const re = this.getDefinedEvent(row2, false, "row", "click");
      return re;
    };
    WOQLTable.prototype.uncompressed = function(row2, col) {
      const re = this.getDefinedEvent(row2, col, "row", "uncompressed");
      return re;
    };
    WOQLTable.prototype.getCellClick = function(row2, col) {
      const cc = this.getDefinedEvent(row2, col, "column", "click");
      return cc;
    };
    WOQLTable.prototype.getRowHover = function(row2) {
      return this.getDefinedEvent(row2, false, "row", "hover");
    };
    WOQLTable.prototype.getCellHover = function(row2, key) {
      return this.getDefinedEvent(row2, key, "column", "hover");
    };
    WOQLTable.prototype.getColumnOrder = function() {
      return this.config.column_order();
    };
    WOQLTable.prototype.bindings = function() {
      return this.config.bindings();
    };
    WOQLTable.prototype.getColumnFilter = function(colid2) {
      const filter = new WOQLRule().matchColumn(this.config.rules, colid2, "filter");
      if (Array.isArray(filter) && filter.length > 0 && filter[0].rule) {
        return filter[0].rule.filter;
      }
      return null;
    };
    WOQLTable.prototype.getColumnDimensions = function(key) {
      const cstyle = {};
      const w = new WOQLRule().matchColumn(this.config.rules, key, "width");
      if (w && w.length && w[w.length - 1].rule.width) {
        cstyle.width = w[w.length - 1].rule.width;
      }
      const max = new WOQLRule().matchColumn(this.config.rules, key, "maxWidth");
      if (max && max.length)
        cstyle.maxWidth = max[max.length - 1].rule.maxWidth;
      const min = new WOQLRule().matchColumn(this.config.rules, key, "minWidth");
      if (min && min.length)
        cstyle.minWidth = min[min.length - 1].rule.minWidth;
      return cstyle;
    };
    WOQLTable.prototype.hasColumnOrder = WOQLTable.prototype.getColumnOrder;
    WOQLTable.prototype.hasCellClick = WOQLTable.prototype.getCellClick;
    WOQLTable.prototype.hasRowClick = WOQLTable.prototype.getRowClick;
    WOQLTable.prototype.hasCellHover = WOQLTable.prototype.getCellHover;
    WOQLTable.prototype.hasRowHover = WOQLTable.prototype.getRowHover;
    WOQLTable.prototype.getRenderer = function(key, row2, rownum) {
      return this.getDefinedEvent(row2, key, "column", "renderer", rownum);
      if (!renderer) {
        const r = this.getRendererForDatatype(row2[key]);
        renderer = r.name;
        if (!args)
          args = r.args;
      }
      if (renderer) {
        return this.datatypes.createRenderer(renderer, args);
      }
    };
    WOQLTable.prototype.isSortableColumn = function(key) {
      if (this.getDefinedEvent(false, key, "column", "unsortable"))
        return false;
      return true;
    };
    WOQLTable.prototype.isFilterableColumn = function(key) {
      if (this.getDefinedEvent(false, key, "column", "filterable") === false)
        return false;
      return true;
    };
    WOQLTable.prototype.renderValue = function(renderer2, val, key, row2) {
      if (val && val["@type"]) {
        renderer2.type = val["@type"];
        var dv = new DataValue(val["@value"], val["@type"], key, row2);
      } else if (val && val["@language"]) {
        renderer2.type = "xsd:string";
        var dv = new DataValue(val["@value"], renderer2.type, key, row2);
      } else if (val && typeof val === "string") {
        renderer2.type = "id";
        var dv = new DataValue(val, "id", key, row2);
      }
      if (dv)
        return renderer2.renderValue(dv);
      return "";
    };
    function DataValue(val, type) {
      this.datavalue = val === "system:unknown" ? "" : val;
      this.datatype = type;
    }
    DataValue.prototype.value = function(nvalue) {
      if (nvalue) {
        this.datavalue = nvalue;
        return this;
      }
      return this.datavalue;
    };
    WOQLTable.prototype.getRendererForDatatype = function(val) {
      if (val && val["@type"]) {
        return this.datatypes.getRenderer(val["@type"], val["@value"]);
      }
      if (val && val["@language"]) {
        return this.datatypes.getRenderer("xsd:string", val["@value"]);
      }
      if (val && typeof val === "string") {
        return this.datatypes.getRenderer("id", val);
      }
      return false;
    };
    WOQLTable.prototype.getSpecificRender = function(key, row2) {
      const rend = this.getDefinedEvent(row2, key, "column", "render");
      return rend;
    };
    module2.exports = WOQLTable;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/tableConfig.js
var require_tableConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/tableConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var WOQLTable = require_woqlTable();
    var UTILS2 = require_utils();
    function WOQLTableConfig() {
      Config.ViewConfig.call(this);
      this.type = "table";
    }
    Object.setPrototypeOf(WOQLTableConfig.prototype, Config.ViewConfig.prototype);
    WOQLTableConfig.prototype.create = function(client) {
      const wqt = new WOQLTable(client, this);
      return wqt;
    };
    WOQLTableConfig.prototype.json = function() {
      const jr = [];
      for (let i = 0; i < this.rules.length; i++) {
        jr.push(this.rules[i].json());
      }
      const conf = {};
      if (typeof this.column_order() !== "undefined") {
        conf.column_order = this.column_order();
      }
      if (typeof this.pagesize() !== "undefined") {
        conf.pagesize = this.pagesize();
      }
      if (typeof this.renderer() !== "undefined") {
        conf.renderer = this.renderer();
      }
      if (typeof this.filter() !== "undefined") {
        conf.filter = this.filter();
      }
      if (typeof this.filterable() !== "undefined") {
        conf.filterable = this.filterable();
      }
      if (typeof this.pager() !== "undefined") {
        conf.pager = this.pager();
      }
      if (typeof this.bindings() !== "undefined") {
        conf.bindings = this.bindings();
      }
      if (typeof this.page() !== "undefined") {
        conf.page = this.page();
      }
      if (typeof this.changesize() !== "undefined") {
        conf.changesize = this.changesize();
      }
      const mj = { table: conf, rules: jr };
      return mj;
    };
    WOQLTableConfig.prototype.loadJSON = function(config, rules) {
      const jr = [];
      if (Array.isArray(rules)) {
        for (let i = 0; i < rules.length; i++) {
          const nr = new WOQLTableRule();
          nr.json(rules[i]);
          jr.push(nr);
        }
      }
      this.rules = jr;
      if (!config)
        return this;
      if (typeof config.column_order !== "undefined") {
        this.column_order(...config.column_order);
      }
      if (typeof config.pagesize !== "undefined") {
        this.pagesize(config.pagesize);
      }
      if (typeof config.renderer !== "undefined") {
        this.renderer(config.renderer);
      }
      if (typeof config.filter !== "undefined") {
        this.filter(config.filter);
      }
      if (typeof config.filterable !== "undefined") {
        this.filterable(config.filterable);
      }
      if (typeof config.bindings !== "undefined") {
        this.bindings(config.bindings);
      }
      if (typeof config.pager !== "undefined") {
        this.pager(config.pager);
      }
      if (typeof config.page !== "undefined") {
        this.page(config.page);
      }
      if (typeof config.changesize !== "undefined") {
        this.changesize(config.changesize);
      }
      return this;
    };
    WOQLTableConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.table();\n";
      if (typeof this.column_order() !== "undefined") {
        str2 += `view.column_order('${this.column_order()}')
`;
      }
      if (typeof this.pagesize() !== "undefined") {
        str2 += `view.pagesize(${this.pagesize()})
`;
      }
      if (typeof this.renderer() !== "undefined") {
        str2 += `view.renderer('${this.renderer()}')
`;
      }
      if (typeof this.pager() !== "undefined") {
        str2 += `view.pager(${this.pager()})
`;
      }
      if (typeof this.page() !== "undefined") {
        str2 += `view.page(${this.page()})
`;
      }
      if (typeof this.changesize() !== "undefined") {
        str2 += `view.changesize(${this.changesize()})
`;
      }
      for (let i = 0; i < this.rules.length; i++) {
        const x = this.rules[i].prettyPrint();
        if (x)
          str2 += `view.${x}
`;
      }
      return str2;
    };
    WOQLTableConfig.prototype.filterable = function(canfilter) {
      if (!canfilter && canfilter !== false)
        return this.tfilterable;
      this.tfilterable = canfilter;
      return this;
    };
    WOQLTableConfig.prototype.filter = function(filter) {
      if (!filter)
        return this.tfilter;
      this.tfilter = filter;
      return this;
    };
    WOQLTableConfig.prototype.renderer = function(rend) {
      if (!rend)
        return this.trenderer;
      this.trenderer = rend;
      return this;
    };
    WOQLTableConfig.prototype.header = function(theader) {
      if (typeof theader === "undefined")
        return this.theader;
      this.theader = theader;
      return this;
    };
    WOQLTableConfig.prototype.column_order = function(...val) {
      if (typeof val === "undefined" || val.length === 0) {
        return this.order;
      }
      this.order = UTILS2.removeNamespacesFromVariables(val);
      return this;
    };
    WOQLTableConfig.prototype.pager = function(val) {
      if (typeof val === "undefined") {
        return this.show_pager;
      }
      this.show_pager = val;
      return this;
    };
    WOQLTableConfig.prototype.changesize = function(val) {
      if (typeof val === "undefined")
        return this.change_pagesize;
      this.change_pagesize = val;
      return this;
    };
    WOQLTableConfig.prototype.pagesize = function(val) {
      if (typeof val === "undefined") {
        return this.show_pagesize;
      }
      this.show_pagesize = val;
      return this;
    };
    WOQLTableConfig.prototype.page = function(val) {
      if (typeof val === "undefined") {
        return this.show_pagenumber;
      }
      this.show_pagenumber = val;
      return this;
    };
    WOQLTableConfig.prototype.column = function(...cols) {
      const nr = new WOQLTableRule().scope("column");
      nr.setVariables(cols);
      this.rules.push(nr);
      return nr;
    };
    WOQLTableConfig.prototype.row = function() {
      const nr = new WOQLTableRule().scope("row");
      this.rules.push(nr);
      return nr;
    };
    function WOQLTableRule() {
      Config.WOQLViewRule.call(this);
    }
    Object.setPrototypeOf(WOQLTableRule.prototype, Config.WOQLViewRule.prototype);
    WOQLTableRule.prototype.header = function(hdr) {
      if (typeof hdr === "undefined") {
        return this.rule.header;
      }
      this.rule.header = hdr;
      return this;
    };
    WOQLTableRule.prototype.filter = function(hdr) {
      if (typeof hdr === "undefined") {
        return this.rule.filter;
      }
      this.rule.filter = hdr;
      return this;
    };
    WOQLTableRule.prototype.filterable = function(hdr) {
      if (typeof hdr === "undefined") {
        return this.rule.filterable;
      }
      this.rule.filterable = hdr;
      return this;
    };
    WOQLTableRule.prototype.width = function(wid) {
      if (typeof wid === "undefined") {
        return this.rule.width;
      }
      this.rule.width = wid;
      return this;
    };
    WOQLTableRule.prototype.maxWidth = function(wid) {
      if (typeof wid === "undefined") {
        return this.rule.maxWidth;
      }
      this.rule.maxWidth = wid;
      return this;
    };
    WOQLTableRule.prototype.minWidth = function(wid) {
      if (typeof wid === "undefined") {
        return this.rule.minWidth;
      }
      this.rule.minWidth = wid;
      return this;
    };
    WOQLTableRule.prototype.unsortable = function(unsortable) {
      if (typeof unsortable === "undefined") {
        return this.rule.unsortable;
      }
      this.rule.unsortable = unsortable;
      return this;
    };
    WOQLTableRule.prototype.uncompressed = function(uncompressed) {
      if (typeof uncompressed === "undefined") {
        return this.rule.uncompressed;
      }
      this.rule.uncompressed = uncompressed;
      return this;
    };
    WOQLTableRule.prototype.prettyPrint = function() {
      let str2 = Config.WOQLViewRule.prototype.prettyPrint.apply(this);
      if (typeof this.header() !== "undefined") {
        str2 += `.header(${this.header()})`;
      }
      if (this.sortable()) {
        str2 += ".sortable(true)";
      }
      if (typeof this.width() !== "undefined") {
        str2 += `.width(${this.width()})`;
      }
      if (typeof this.maxWidth() !== "undefined") {
        str2 += `.maxWidth(${this.maxWidth()})`;
      }
      if (typeof this.minWidth() !== "undefined") {
        str2 += `.minWidth(${this.minWidth()})`;
      }
      return str2;
    };
    module2.exports = WOQLTableConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlChooser.js
var require_woqlChooser = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlChooser.js"(exports2, module2) {
    var WOQLChooserConfig = require_chooserConfig();
    var UTILS2 = require_utils();
    var { WOQLRule } = require_woqlRule();
    function WOQLChooser(client, config) {
      this.client = client;
      this.config = config || new WOQLChooserConfig();
      this.selected = false;
      this.cursor = 0;
      return this;
    }
    WOQLChooser.prototype.options = function(config) {
      this.config = config;
      return this;
    };
    WOQLChooser.prototype.set = function(id) {
      if (this.selected !== id) {
        this.selected = id;
        const ch = this.config.change;
        if (ch)
          ch(id);
      }
    };
    WOQLChooser.prototype.setResult = function(result) {
      this.result = result;
      this.choices = [];
      let rows = 0;
      const variables = result.getVariableList();
      if (!this.config.values() && variables.length) {
        this.config.values(variables[0]);
      }
      if (this.config.sort()) {
        this.result.sort(this.config.sort(), this.config.direction());
      }
      while (row = this.result.next()) {
        if (row && this.includeRow(row, this.result.cursor)) {
          this.choices.push(this.rowToChoice(row, rows++));
        }
      }
      return this;
    };
    WOQLChooser.prototype.includeRow = function(row2, index) {
      const matched_rules = new WOQLRule().matchRow(this.config.rules, row2, index, "hidden");
      for (let i = 0; i < matched_rules.length; i++) {
        if (matched_rules[i].rule.hidden)
          return false;
      }
      return true;
    };
    WOQLChooser.prototype.rowToChoice = function(row2, rownum) {
      const choice = {
        id: this.getRowID(row2)
      };
      choice.label = this.getLabelFromBinding(row2, rownum);
      choice.title = this.getTitleFromBinding(row2, rownum);
      choice.selected = this.getSelectedFromBinding(row2, rownum);
      return choice;
    };
    WOQLChooser.prototype.getRowID = function(row2) {
      const rval = row2[this.config.values()];
      if (rval["@value"])
        return rval["@value"];
      return rval;
    };
    WOQLChooser.prototype.getLabelFromBinding = function(row2, rownum) {
      const sp = this.getSpecialRenderer(row2, rownum, "label");
      if (sp)
        return this.renderSpecial(sp, row2, rownum);
      if (this.config.labels()) {
        if (row2[this.config.labels()]) {
          let lab = row2[this.config.labels()];
          if (lab["@value"])
            lab = lab["@value"];
          if (lab !== "system:unknown")
            return lab;
        }
      }
      return UTILS2.labelFromURL(this.getRowID(row2));
    };
    WOQLChooser.prototype.getTitleFromBinding = function(row2, rownum) {
      const sp = this.getSpecialRenderer(row2, rownum, "title");
      if (sp)
        return this.renderSpecial(sp, row2, rownum);
      if (this.config.titles()) {
        if (row2[this.config.titles()]) {
          let lab = row2[this.config.titles()];
          if (lab["@value"])
            lab = lab["@value"];
          if (lab !== "system:unknown")
            return lab;
        }
      }
      return false;
    };
    WOQLChooser.prototype.getSelectedFromBinding = function(row2, rownum) {
      const matched_rules = new WOQLRule().matchRow(this.config.rules, row2, rownum, "selected");
      if (matched_rules && matched_rules.length) {
        return matched_rules[matched_rules.length - 1].rule.selected;
      }
      return false;
    };
    WOQLChooser.prototype.render = function() {
      if (this.renderer)
        return this.renderer.render(this);
    };
    WOQLChooser.prototype.setRenderer = function(rend) {
      this.renderer = rend;
      return this;
    };
    WOQLChooser.prototype.getSpecialRenderer = function(row2, index, type) {
      const matched_rules = new WOQLRule().matchRow(this.config.rules, row2, index, type);
      for (let i = 0; i < matched_rules.length; i++) {
        if (matched_rules[i].rule[type])
          return matched_rules[i].rule[type];
      }
      return false;
    };
    WOQLChooser.prototype.renderSpecial = function(rule, row2) {
      if (rule && typeof rule === "function") {
        return rule(row2);
      }
      if (rule && typeof rule === "string") {
        return rule;
      }
    };
    WOQLChooser.prototype.count = function() {
      return this.result.count();
    };
    WOQLChooser.prototype.first = function() {
      this.cursor = 0;
      return this.choices[this.cursor];
    };
    WOQLChooser.prototype.next = function() {
      const res = this.choices[this.cursor];
      this.cursor++;
      return res;
    };
    WOQLChooser.prototype.prev = function() {
      if (this.cursor > 0) {
        this.cursor--;
        return this.choices[this.cursor];
      }
    };
    module2.exports = WOQLChooser;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/chooserConfig.js
var require_chooserConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/chooserConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var WOQLChooser = require_woqlChooser();
    var UTILS2 = require_utils();
    function WOQLChooserConfig() {
      Config.ViewConfig.call(this);
      this.type = "chooser";
    }
    Object.setPrototypeOf(WOQLChooserConfig.prototype, Config.ViewConfig.prototype);
    WOQLChooserConfig.prototype.create = function(client) {
      const wqt = new WOQLChooser(client, this);
      return wqt;
    };
    WOQLChooserConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.chooser();\n";
      str2 += this.getBasicPrettyPrint();
      if (typeof this.change() !== "undefined") {
        str2 += `view.change(${this.change()})
`;
      }
      if (typeof this.show_empty() !== "undefined") {
        str2 += `view.show_empty('${this.show_empty()}')
`;
      }
      if (typeof this.values() !== "undefined") {
        str2 += `view.values('${UTILS2.removeNamespaceFromVariable(this.values())}')
`;
      }
      if (typeof this.labels() !== "undefined") {
        str2 += `view.labels('${UTILS2.removeNamespaceFromVariable(this.labels())}')
`;
      }
      if (typeof this.titles() !== "undefined") {
        str2 += `view.titles('${UTILS2.removeNamespaceFromVariable(this.titles())}')
`;
      }
      if (typeof this.sort() !== "undefined") {
        str2 += `view.sort(${this.sort()})
`;
      }
      if (typeof this.direction() !== "undefined") {
        str2 += `view.direction('${this.direction()}')
`;
      }
      for (let i = 0; i < this.rules.length; i++) {
        str2 += `view.${this.rules[i].prettyPrint("chooser")}
`;
      }
      return str2;
    };
    WOQLChooserConfig.prototype.json = function() {
      const conf = this.getBasicJSON();
      if (typeof this.change() !== "undefined") {
        conf.change = this.change();
      }
      if (typeof this.show_empty() !== "undefined") {
        conf.show_empty = this.show_empty();
      }
      if (typeof this.values() !== "undefined") {
        conf.values = this.values();
      }
      if (typeof this.labels() !== "undefined") {
        conf.labels = this.labels();
      }
      if (typeof this.titles() !== "undefined") {
        conf.titles = this.titles();
      }
      if (typeof this.sort() !== "undefined") {
        conf.sort = this.sort();
      }
      if (typeof this.direction() !== "undefined") {
        conf.direction = this.direction();
      }
      const mj = { chooser: conf, rules: this.getRulesJSON() };
      return mj;
    };
    WOQLChooserConfig.prototype.loadJSON = function(config, rules) {
      const jr = [];
      for (let i = 0; i < rules.length; i++) {
        const nr = new WOQLChooserRule();
        nr.json(rules[i]);
        jr.push(nr);
      }
      this.rules = jr;
      this.loadBasicJSON(config);
      if (typeof config.change !== "undefined") {
        this.change(config.change);
      }
      if (typeof config.show_empty !== "undefined") {
        this.show_empty(config.show_empty);
      }
      if (typeof config.values !== "undefined") {
        this.values(config.values);
      }
      if (typeof config.labels !== "undefined") {
        this.labels(config.labels);
      }
      if (typeof config.titles !== "undefined") {
        this.titles(config.titles);
      }
      if (typeof config.sort !== "undefined") {
        this.sort(config.sort);
      }
      if (typeof config.direction !== "undefined") {
        this.direction(config.direction);
      }
    };
    WOQLChooserConfig.prototype.change = function(v) {
      if (typeof v !== "undefined") {
        this.onChange = v;
        return this;
      }
      return this.onChange;
    };
    WOQLChooserConfig.prototype.show_empty = function(p) {
      if (typeof p !== "undefined") {
        this.placeholder = p;
        return this;
      }
      return this.placeholder;
    };
    WOQLChooserConfig.prototype.rule = function(v) {
      const nr = new WOQLChooserRule().scope("row");
      this.rules.push(nr);
      if (v)
        nr.vars(v);
      return nr;
    };
    WOQLChooserConfig.prototype.values = function(v) {
      if (typeof v !== "undefined") {
        if (v.substring(0, 2) === "v:")
          v = v.substring(2);
        this.value_variable = v;
        return this;
      }
      return this.value_variable;
    };
    WOQLChooserConfig.prototype.labels = function(v) {
      if (v) {
        if (v.substring(0, 2) === "v:")
          v = v.substring(2);
        this.label_variable = v;
        return this;
      }
      return this.label_variable;
    };
    WOQLChooserConfig.prototype.titles = function(v) {
      if (v) {
        if (v.substring(0, 2) === "v:")
          v = v.substring(2);
        this.title_variable = v;
        return this;
      }
      return this.title_variable;
    };
    WOQLChooserConfig.prototype.sort = function(v) {
      if (v) {
        if (v.substring(0, 2) === "v:")
          v = v.substring(2);
        this.sort_variable = v;
        return this;
      }
      return this.sort_variable;
    };
    WOQLChooserConfig.prototype.direction = function(v) {
      if (v) {
        this.sort_direction = v;
        return this;
      }
      return this.sort_direction;
    };
    function WOQLChooserRule(scope) {
      Config.WOQLViewRule.call(this, scope);
    }
    Object.setPrototypeOf(WOQLChooserRule.prototype, Config.WOQLViewRule.prototype);
    WOQLChooserRule.prototype.label = function(l) {
      if (l) {
        this.rule.label = l;
        return this;
      }
      return this.rule.label;
    };
    WOQLChooserRule.prototype.title = function(l) {
      if (l) {
        this.rule.title = l;
        return this;
      }
      return this.rule.title;
    };
    WOQLChooserRule.prototype.values = function(l) {
      if (l) {
        this.rule.values = l;
        return this;
      }
      return this.rule.values;
    };
    WOQLChooserRule.prototype.selected = function(s) {
      if (typeof s !== "undefined") {
        this.rule.selected = s;
        return this;
      }
      return this.rule.selected;
    };
    WOQLChooserRule.prototype.prettyPrint = function() {
      let str2 = WOQLViewRule.prototype.prettyPrint.apply(this);
      if (typeof this.selected() !== "undefined") {
        str2 += `.selected(${this.selected()})`;
      }
      if (typeof this.label() !== "undefined") {
        str2 += `.label("${this.label()}")`;
      }
      if (typeof this.title() !== "undefined") {
        str2 += `.title("${this.title()}")`;
      }
      if (typeof this.values() !== "undefined") {
        str2 += `.values("${this.values()}")`;
      }
      return str2;
    };
    module2.exports = WOQLChooserConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlGraph.js
var require_woqlGraph = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlGraph.js"(exports2, module2) {
    var WOQLGraphConfig = require_graphConfig();
    var UTILS2 = require_utils();
    var { WOQLRule } = require_woqlRule();
    function WOQLGraph(client, config) {
      this.client = client;
      this.config = config || new WOQLGraphConfig();
      this.nodes = [];
      this.edges = [];
    }
    WOQLGraph.prototype.options = function(config) {
      this.config = config;
      return this;
    };
    WOQLGraph.prototype.setResult = function(result) {
      this.result = result;
      this.calculateVariableTypes(result);
    };
    WOQLGraph.prototype.calculateVariableTypes = function() {
      const bindings = this.result;
      if (bindings && bindings.length) {
        for (let i = 0; i < bindings.length; i++) {
          this.extractFromBinding(bindings[i], i);
        }
      }
      this.edges = this.combineEdges(this.edges);
      this.nodes = this.combineNodes(this.nodes);
    };
    WOQLGraph.prototype.extractFromBinding = function(row2, rownum) {
      if (this.includeRow(row2, rownum)) {
        const nodes = [];
        const lits = [];
        for (const k in row2) {
          if (typeof row2[k] === "string") {
            if (row2[k] && row2[k] !== "system:unknown" && this.includeNode(k, row2)) {
              nodes.push(k);
            }
          } else if (row2[k]["@value"] && this.includeLiteralNode(k, row2)) {
            nodes.push(k);
          }
        }
        if (nodes.length === 0)
          return;
        this.createEdgesFromIDs(nodes, row2);
        for (let i = 0; i < nodes.length; i++) {
          let ndid = row2[nodes[i]];
          ndid = ndid["@value"] ? ndid["@value"] : ndid;
          this.addAdornedNode(ndid, nodes[i], row2);
        }
      }
    };
    WOQLGraph.prototype.addAdornedEdge = function(source, target, ks, kt, row2) {
      source = source["@value"] ? source["@value"] : source;
      target = target["@value"] ? target["@value"] : target;
      const edge = {
        type: "link",
        target,
        source,
        text: target
      };
      const matched_rules = new WOQLRule().matchEdge(this.config.rules, row2, ks, kt);
      let hid = false;
      for (let i = 0; i < matched_rules.length; i++) {
        const { rule } = matched_rules[i];
        if (typeof rule.hidden !== "undefined") {
          hid = rule.hidden;
        }
        if (rule.size) {
          edge.size = UTILS2.getConfigValue(rule.size, row2);
        }
        if (rule.text) {
          edge.text = UTILS2.getConfigValue(rule.text, row2);
        }
        if (rule.color) {
          edge.color = UTILS2.getConfigValue(rule.color, row2);
        }
        if (rule.icon) {
          edge.icon = UTILS2.getConfigValue(rule.icon, row2);
        }
        if (rule.distance) {
          edge.distance = UTILS2.getConfigValue(rule.distance, row2);
        }
        if (rule.arrow) {
          edge.arrow = UTILS2.getConfigValue(rule.arrow, row2);
        }
        if (rule.symmetric) {
          edge.symmetric = UTILS2.getConfigValue(rule.symmetric, row2);
        }
        if (rule.weight) {
          edge.weight = UTILS2.getConfigValue(rule.weight, row2);
        }
      }
      if (!hid)
        this.edges.push(edge);
    };
    WOQLGraph.prototype.addAdornedNode = function(nid, k, row2) {
      const node = { type: "node", id: nid, nodetype: k };
      const matched_rules = new WOQLRule().matchNode(this.config.rules, row2, k, nid);
      for (let i = 0; i < matched_rules.length; i++) {
        const { rule } = matched_rules[i];
        if (rule.size) {
          node.radius = UTILS2.getConfigValue(rule.size, row2);
        }
        if (rule.color) {
          node.color = UTILS2.getConfigValue(rule.color, row2);
        }
        if (rule.charge) {
          node.charge = UTILS2.getConfigValue(rule.charge, row2);
        }
        if (rule.collisionRadius) {
          node.collisionRadius = UTILS2.getConfigValue(rule.collisionRadius, row2);
        }
        if (rule.icon) {
          node.icon = UTILS2.getConfigValue(rule.icon, row2);
        }
        if (rule.text) {
          node.text = UTILS2.getConfigValue(rule.text, row2);
        }
        if (rule.border) {
          node.border = UTILS2.getConfigValue(rule.border, row2);
        }
      }
      if (!node.text) {
        if (typeof row2[k] === "string")
          node.text = row2[k];
        else if (row2[k]["@value"])
          node.text = row2[k]["@value"];
      }
      this.nodes.push(node);
    };
    WOQLGraph.prototype.getLiteralOwner = function(nodes, v, row2) {
      const cs = this.config.source();
      if (cs && row2[cs]) {
        return cs;
      }
      const edges = this.config.edges();
      if (edges) {
        for (let i = 0; i < edges.length; i++) {
          if (edges[i][1] === v) {
            return edges[i][0];
          }
        }
        return false;
      }
      return nodes[0];
    };
    WOQLGraph.prototype.createEdgesFromIDs = function(nodes, row2) {
      if (nodes.length < 2)
        return;
      const cs = this.config.source();
      const es = this.config.edges();
      if (!cs && es && es.length) {
        for (var i = 0; i < es.length; i++) {
          if (nodes.indexOf(es[i][0]) !== -1 && nodes.indexOf(es[i][1]) !== -1) {
            this.addAdornedEdge(row2[es[i][0]], row2[es[i][1]], es[i][0], es[i][1], row2);
          }
        }
        return;
      }
      const s = cs && nodes.indexOf(cs) !== -1 ? cs : nodes[0];
      for (var i = 0; i < nodes.length; i++) {
        if (nodes[i] === s)
          continue;
        this.addAdornedEdge(row2[s], row2[nodes[i]], s, nodes[i], row2);
      }
    };
    WOQLGraph.prototype.getEdges = function() {
      return this.edges;
    };
    WOQLGraph.prototype.combineNodes = function(nodes) {
      const nnodes = {};
      for (let i = 0; i < nodes.length; i++) {
        const n = nodes[i];
        if (this.nodes_referenced_by_edges.indexOf(n.id) === -1)
          continue;
        if (typeof nnodes[n.id] === "undefined") {
          nnodes[n.id] = n;
        } else {
          for (const k in n) {
            if (typeof nnodes[n.id][k] === "undefined") {
              nnodes[n.id][k] = n[k];
            }
          }
        }
      }
      return Object.values(nnodes);
    };
    WOQLGraph.prototype.combineEdges = function(edges) {
      this.nodes_referenced_by_edges = [];
      const nedges = {};
      for (let i = 0; i < edges.length; i++) {
        const e = edges[i];
        if (typeof nedges[e.source] === "undefined") {
          nedges[e.source] = {};
        }
        if (typeof nedges[e.source][e.target] === "undefined") {
          nedges[e.source][e.target] = e;
        } else {
          for (var k in e) {
            if (typeof nedges[e.source][e.target][k] === "undefined")
              nedges[e.source][e.target][k] = e[k];
          }
        }
        if (this.nodes_referenced_by_edges.indexOf(e.source) === -1) {
          this.nodes_referenced_by_edges.push(e.source);
        }
        if (this.nodes_referenced_by_edges.indexOf(e.target) === -1) {
          this.nodes_referenced_by_edges.push(e.target);
        }
      }
      const fedges = [];
      for (var k in nedges) {
        for (const k2 in nedges[k]) {
          fedges.push(nedges[k][k2]);
        }
      }
      return fedges;
    };
    WOQLGraph.prototype.getNodes = function() {
      return this.nodes;
    };
    WOQLGraph.prototype.includeNode = function(v, row2) {
      const matched_rules = new WOQLRule().matchNode(this.config.rules, row2, v, false, "hidden");
      for (let i = 0; i < matched_rules.length; i++) {
        if (matched_rules[i].rule.hidden)
          return false;
      }
      return true;
    };
    WOQLGraph.prototype.includeLiteralNode = function(variableName, row2) {
      if (this.config.literals() === false)
        return false;
      const matched_rules = new WOQLRule().matchNode(this.config.rules, row2, variableName, false, "hidden");
      for (let i = 0; i < matched_rules.length; i++) {
        if (matched_rules[i].rule.hidden)
          return false;
      }
      return true;
    };
    WOQLGraph.prototype.includeRow = function(row2, num) {
      const matched_rules = new WOQLRule().matchRow(this.config.rules, row2, num, "hidden");
      for (let i = 0; i < matched_rules.length; i++) {
        if (matched_rules[i].rule.hidden)
          return false;
      }
      return true;
    };
    module2.exports = WOQLGraph;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/graphConfig.js
var require_graphConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/graphConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var WOQLGraph = require_woqlGraph();
    var UTILS2 = require_utils();
    function WOQLGraphConfig() {
      Config.ViewConfig.call(this);
      this.type = "graph";
    }
    Object.setPrototypeOf(WOQLGraphConfig.prototype, Config.ViewConfig.prototype);
    WOQLGraphConfig.prototype.create = function(client) {
      const wqt = new WOQLGraph(client, this);
      return wqt;
    };
    WOQLGraphConfig.prototype.literals = function(v) {
      if (typeof v !== "undefined") {
        this.show_literals = v;
        return this;
      }
      return this.show_literals;
    };
    WOQLGraphConfig.prototype.source = function(v) {
      if (v) {
        this.source_variable = UTILS2.removeNamespaceFromVariable(v);
        return this;
      }
      return this.source_variable;
    };
    WOQLGraphConfig.prototype.fontfamily = function(v) {
      if (typeof v !== "undefined") {
        this.fontfam = v;
        return this;
      }
      return this.fontfam;
    };
    WOQLGraphConfig.prototype.show_force = function(v) {
      if (typeof v !== "undefined") {
        this.force = v;
        return this;
      }
      return this.force;
    };
    WOQLGraphConfig.prototype.fix_nodes = function(v) {
      if (typeof v !== "undefined") {
        this.fixed = v;
        return this;
      }
      return this.fixed;
    };
    WOQLGraphConfig.prototype.explode_out = function(v) {
      if (typeof v !== "undefined") {
        this.explode = v;
        return this;
      }
      return this.explode;
    };
    WOQLGraphConfig.prototype.selected_grows = function(v) {
      if (typeof v !== "undefined") {
        this.bigsel = v;
        return this;
      }
      return this.bigsel;
    };
    WOQLGraphConfig.prototype.width = function(size) {
      if (typeof size !== "undefined") {
        this.gwidth = size;
        return this;
      }
      return this.gwidth;
    };
    WOQLGraphConfig.prototype.height = function(size) {
      if (typeof size !== "undefined") {
        this.gheight = size;
        return this;
      }
      return this.gheight;
    };
    WOQLGraphConfig.prototype.edges = function(...edges) {
      if (edges && edges.length) {
        const nedges = [];
        for (let i = 0; i < edges.length; i++) {
          nedges.push(UTILS2.removeNamespacesFromVariables(edges[i]));
        }
        this.show_edges = nedges;
        return this;
      }
      return this.show_edges;
    };
    WOQLGraphConfig.prototype.edge = function(source, target) {
      const nr = new WOQLGraphRule().edge(source, target);
      this.rules.push(nr);
      return nr;
    };
    WOQLGraphConfig.prototype.node = function(...cols) {
      const nr = new WOQLGraphRule();
      if (cols && cols.length) {
        nr.scope("node").setVariables(cols);
      } else {
        nr.scope("row");
      }
      this.rules.push(nr);
      return nr;
    };
    WOQLGraphConfig.prototype.loadJSON = function(config, rules) {
      const jr = [];
      for (let i = 0; i < rules.length; i++) {
        const nr = new WOQLGraphRule();
        nr.json(rules[i]);
        jr.push(nr);
      }
      this.rules = jr;
      if (typeof config.literals !== "undefined") {
        this.literals(config.literals);
      }
      if (typeof config.source !== "undefined") {
        this.source(config.source);
      }
      if (typeof config.fontfamily !== "undefined") {
        this.fontfamily(config.fontfamily);
      }
      if (typeof config.show_force !== "undefined") {
        this.show_force(config.show_force);
      }
      if (typeof config.fix_nodes !== "undefined") {
        this.fix_nodes(config.fix_nodes);
      }
      if (typeof config.explode_out !== "undefined") {
        this.explode_out(config.explode_out);
      }
      if (typeof config.selected_grows !== "undefined") {
        this.selected_grows(config.selected_grows);
      }
      if (typeof config.width !== "undefined") {
        this.width(config.width);
      }
      if (typeof config.height !== "undefined") {
        this.height(config.height);
      }
      if (typeof config.edges !== "undefined") {
        this.edges(...config.edges);
      }
    };
    WOQLGraphConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.graph();\n";
      if (typeof this.literals() !== "undefined") {
        str2 += `view.literals('${this.literals()}')
`;
      }
      if (typeof this.source() !== "undefined") {
        str2 += `view.source('${UTILS2.removeNamespaceFromVariable(this.source())}')
`;
      }
      if (typeof this.fontfamily() !== "undefined") {
        str2 += `view.fontfamily('${this.fontfamily()}')
`;
      }
      if (typeof this.show_force() !== "undefined") {
        str2 += `view.show_force('${this.show_force()}')
`;
      }
      if (typeof this.fix_nodes() !== "undefined") {
        str2 += `view.fix_nodes('${this.fix_nodes()}')
`;
      }
      if (typeof this.explode_out() !== "undefined") {
        str2 += `view.explode_out('${this.explode_out()}')
`;
      }
      if (typeof this.selected_grows() !== "undefined") {
        str2 += `view.selected_grows('${this.selected_grows()}')
`;
      }
      if (typeof this.width() !== "undefined") {
        str2 += `view.width('${this.width()}')
`;
      }
      if (typeof this.height() !== "undefined") {
        str2 += `view.height('${this.height()}')
`;
      }
      if (typeof this.edges() !== "undefined") {
        const nedges = this.edges();
        const estrs = [];
        for (var i = 0; i < nedges.length; i++) {
          estrs.push(`['${nedges[i][0]}, ${nedges[i][1]}']`);
        }
        str2 += `view.edges('${estrs.join(", ")}')
`;
      }
      for (var i = 0; i < this.rules.length; i++) {
        const x = this.rules[i].prettyPrint();
        if (x)
          str2 += `view.${x}
`;
      }
      return str2;
    };
    WOQLGraphConfig.prototype.json = function() {
      const jr = [];
      for (let i = 0; i < this.rules.length; i++) {
        jr.push(this.rules[i].json());
      }
      const json2 = {};
      if (typeof this.literals() !== "undefined") {
        json2.literals = this.literals();
      }
      if (typeof this.source() !== "undefined") {
        json2.source = this.source();
      }
      if (typeof this.fontfamily() !== "undefined") {
        json2.fontfamily = this.fontfamily();
      }
      if (typeof this.show_force() !== "undefined") {
        json2.show_force = this.show_force();
      }
      if (typeof this.fix_nodes() !== "undefined") {
        json2.fix_nodes = this.fix_nodes();
      }
      if (typeof this.explode_out() !== "undefined") {
        json2.explode_out = this.explode_out();
      }
      if (typeof this.selected_grows() !== "undefined") {
        json2.selected_grows = this.selected_grows();
      }
      if (typeof this.width() !== "undefined") {
        json2.width = this.width();
      }
      if (typeof this.height() !== "undefined") {
        json2.height = this.height();
      }
      if (typeof this.edges() !== "undefined") {
        json2.edges = this.edges();
      }
      const mj = { graph: json2, rules: jr };
      return mj;
    };
    function WOQLGraphRule(scope) {
      Config.WOQLViewRule.call(this, scope);
    }
    Object.setPrototypeOf(WOQLGraphRule.prototype, Config.WOQLViewRule.prototype);
    WOQLGraphRule.prototype.charge = function(v) {
      if (typeof v === "undefined") {
        return this.rule.charge;
      }
      this.rule.charge = v;
      return this;
    };
    WOQLGraphRule.prototype.collisionRadius = function(v) {
      if (typeof v === "undefined") {
        return this.rule.collisionRadius;
      }
      this.rule.collisionRadius = v;
      return this;
    };
    WOQLGraphRule.prototype.arrow = function(json2) {
      if (json2) {
        this.rule.arrow = json2;
        return this;
      }
      return this.rule.arrow;
    };
    WOQLGraphRule.prototype.distance = function(d) {
      if (typeof d !== "undefined") {
        this.rule.distance = d;
        return this;
      }
      return this.rule.distance;
    };
    WOQLGraphRule.prototype.symmetric = function(d) {
      if (typeof d !== "undefined") {
        this.rule.symmetric = d;
        return this;
      }
      return this.rule.symmetric;
    };
    WOQLGraphRule.prototype.weight = function(w) {
      if (typeof w !== "undefined") {
        this.rule.weight = w;
        return this;
      }
      return this.rule.weight;
    };
    WOQLGraphRule.prototype.prettyPrint = function() {
      let str2 = Config.WOQLViewRule.prototype.prettyPrint.apply(this);
      if (typeof this.charge() !== "undefined") {
        str2 += `.charge('${this.charge()}')`;
      }
      if (typeof this.distance() !== "undefined") {
        str2 += `.distance('${this.distance()}')`;
      }
      if (typeof this.weight() !== "undefined") {
        str2 += `.weight('${this.weight()}')`;
      }
      if (typeof this.symmetric() !== "undefined") {
        str2 += `.symmetric(${this.symmetric()})`;
      }
      if (typeof this.collisionRadius() !== "undefined") {
        str2 += `.collisionRadius(${this.collisionRadius()})`;
      }
      if (typeof this.arrow() !== "undefined") {
        str2 += `.arrow(${JSON.stringify(this.arrow())})`;
      }
      return str2;
    };
    module2.exports = WOQLGraphConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlChart.js
var require_woqlChart = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlChart.js"() {
    var WOQLChartConfig = require_chartConfig();
    function WOQLChart(client, config) {
      this.client = client;
      this.config = config || new WOQLChartConfig();
      return this;
    }
    WOQLChart.prototype.options = function(config) {
      this.config = config;
      return this;
    };
    WOQLChart.prototype.setResult = function(res) {
      this.result = res;
      return this;
    };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/chartConfig.js
var require_chartConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/chartConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var WOQLChart = require_woqlChart();
    function WOQLChartConfig() {
      Config.ViewConfig.call(this);
      this.type = "chart";
    }
    Object.setPrototypeOf(WOQLChartConfig.prototype, Config.ViewConfig.prototype);
    WOQLChartConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.chart();\n";
      for (let i = 0; i < this.rules.length; i++) {
        str2 += `view.${this.rules[i].prettyPrint()}
`;
      }
      if (typeof this.margin() !== "undefined") {
        str2 += `view.margin(${this.margin()})
`;
      }
      if (typeof this.title() !== "undefined") {
        str2 += `view.title('${this.title()}')
`;
      }
      if (typeof this.description() !== "undefined") {
        str2 += `view.description('${this.description()}')
`;
      }
      if (typeof this.layout() !== "undefined") {
        str2 += `view.layout('${this.layout()}')
`;
      }
      if (typeof this.align() !== "undefined") {
        str2 += `view.align('${this.align()}')
`;
      }
      str2 += this.getBasicPrettyPrint();
      return str2;
    };
    WOQLChartConfig.prototype.json = function() {
      const conf = {};
      if (typeof this.margin() !== "undefined") {
        conf.margin = this.margin();
      }
      if (typeof this.title() !== "undefined") {
        conf.title = this.title();
      }
      if (typeof this.description() !== "undefined") {
        conf.description = this.description();
      }
      if (typeof this.layout() !== "undefined") {
        conf.layout = this.layout();
      }
      if (typeof this.align() !== "undefined") {
        conf.align = this.align();
      }
      const mj = { chart: conf, rules: this.getRulesJSON() };
      return mj;
    };
    WOQLChartConfig.prototype.loadJSON = function(config, rules) {
      this.loadBasicJSON(config);
      const jr = [];
      for (let i = 0; i < rules.length; i++) {
        const nr = new WOQLChartRule();
        nr.json(rules[i]);
        jr.push(nr);
      }
      this.rules = jr;
      if (typeof config.margin !== "undefined") {
        this.margin(config.margin);
      }
      if (typeof config.title !== "undefined") {
        this.title(config.title);
      }
      if (typeof config.description !== "undefined") {
        this.description(config.description);
      }
      if (typeof config.layout !== "undefined") {
        this.layout(config.layout);
      }
      if (typeof config.align !== "undefined") {
        this.align(config.align);
      }
    };
    WOQLChartConfig.prototype.title = function(title) {
      if (typeof title === "undefined") {
        return this._title;
      }
      this._title = title;
      return this;
    };
    WOQLChartConfig.prototype.description = function(description) {
      if (description) {
        this._description = description;
        return this;
      }
      return this._description;
    };
    WOQLChartConfig.prototype.layout = function(layout) {
      if (layout) {
        this._layout = layout;
        return this;
      }
      return this._layout;
    };
    WOQLChartConfig.prototype.align = function(align) {
      if (align) {
        this._align = align;
        return this;
      }
      return this._align;
    };
    WOQLChartConfig.prototype.margin = function(marginObj) {
      if (marginObj) {
        this._margin = marginObj;
        return this;
      }
      return this._margin;
    };
    WOQLChartConfig.prototype.create = function(client) {
      const wqt = new WOQLChartConfig(client, this);
      return wqt;
    };
    WOQLChartConfig.prototype.xAxis = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("XAxis");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.legend = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("Legend");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.yAxis = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("YAxis");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.bar = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("Bar");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.line = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("Line");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.point = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("Point");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    WOQLChartConfig.prototype.area = function(...vars) {
      const woqlRule = new WOQLChartRule().scope("Area");
      woqlRule.setVariables(vars);
      this.rules.push(woqlRule);
      return woqlRule;
    };
    function WOQLChartRule() {
      Config.WOQLViewRule.call(this);
    }
    Object.setPrototypeOf(WOQLChartRule.prototype, Config.WOQLViewRule.prototype);
    WOQLChartRule.prototype.style = function(key, value) {
      if (value) {
        this.rule[key] = value;
        return this;
      }
      return this.rule[key];
    };
    WOQLChartRule.prototype.label = function(label) {
      if (label) {
        this.rule.label = label;
        return this;
      }
      return this.rule.label;
    };
    WOQLChartRule.prototype.legendType = function(legendType) {
      if (legendType) {
        this.rule.legendType = legendType;
        return this;
      }
      return this.rule.legendType;
    };
    WOQLChartRule.prototype.fillOpacity = function(fillOpacity) {
      if (fillOpacity || fillOpacity === 0) {
        this.rule.fillOpacity = fillOpacity;
        return this;
      }
      return this.rule.fillOpacity;
    };
    WOQLChartRule.prototype.fill = function(color) {
      if (color) {
        this.rule.fill = color;
        return this;
      }
      return this.rule.fill;
    };
    WOQLChartRule.prototype.stroke = function(color) {
      if (color) {
        this.rule.stroke = color;
        return this;
      }
      return this.rule.stroke;
    };
    WOQLChartRule.prototype.strokeWidth = function(size) {
      if (typeof size !== "undefined") {
        this.rule.strokeWidth = size;
        return this;
      }
      return this.rule.strokeWidth;
    };
    WOQLChartRule.prototype.dot = function(isVisible) {
      if (typeof isVisible !== "undefined") {
        this.rule.dot = isVisible;
        return this;
      }
      return this.rule.dot;
    };
    WOQLChartRule.prototype.labelRotate = function(angle) {
      if (angle !== void 0) {
        this.rule.labelRotate = angle;
        return this;
      }
      return this.rule.labelRotate;
    };
    WOQLChartRule.prototype.padding = function(paddingObj) {
      if (paddingObj) {
        this.rule.padding = paddingObj;
        return this;
      }
      return this.rule.padding;
    };
    WOQLChartRule.prototype.labelDateInput = function(labelDateInput) {
      if (labelDateInput) {
        this.rule.labelDateInput = labelDateInput;
        return this;
      }
      return this.rule.labelDateInput;
    };
    WOQLChartRule.prototype.labelDateOutput = function(labelDateOutput) {
      if (labelDateOutput) {
        this.rule.labelDateOutput = labelDateOutput;
        return this;
      }
      return this.rule.labelDateOutput;
    };
    WOQLChartRule.prototype.stackId = function(stackId) {
      if (stackId) {
        this.rule.stackId = stackId;
        return this;
      }
      return this.rule.stackId;
    };
    WOQLChartRule.prototype.type = function(type) {
      if (type) {
        this.rule.type = type;
        return this;
      }
      return this.rule.type;
    };
    WOQLChartRule.prototype.axisDomain = function(domainArr) {
      if (domainArr) {
        this.rule.domain = domainArr;
        return this;
      }
      return this.rule.domain;
    };
    WOQLChartRule.prototype.colorEntry = function(propValue) {
      if (propValue) {
        this.rule.colorEntry = propValue;
        return this;
      }
      return this.rule.colorEntry;
    };
    WOQLChartRule.prototype.customColors = function(colorsObj) {
      if (colorsObj) {
        this.rule.customColors = colorsObj;
        return this;
      }
      return this.rule.customColors;
    };
    WOQLChartRule.prototype.payload = function(payloadArr) {
      if (payloadArr) {
        this.rule.payload = payloadArr;
        return this;
      }
      return this.rule.payload;
    };
    WOQLChartRule.prototype.barSize = function(barSize) {
      if (barSize) {
        this.rule.barSize = barSize;
        return this;
      }
      return this.rule.barSize;
    };
    module2.exports = WOQLChartConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlStream.js
var require_woqlStream = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlStream.js"(exports2, module2) {
    var WOQLStreamConfig = require_streamConfig();
    function WOQLStream(client, config) {
      this.client = client;
      this.config = config || new WOQLStreamConfig();
      return this;
    }
    WOQLStream.prototype.options = function(config) {
      this.config = config;
      return this;
    };
    WOQLStream.prototype.setResult = function(wqrs) {
      this.result = wqrs;
    };
    module2.exports = WOQLStream;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/streamConfig.js
var require_streamConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/streamConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var WOQLStream = require_woqlStream();
    function WOQLStreamConfig() {
      Config.ViewConfig.call(this);
      this.type = "stream";
    }
    Object.setPrototypeOf(WOQLStreamConfig.prototype, Config.ViewConfig.prototype);
    WOQLStreamConfig.prototype.create = function(client) {
      const wqt = new WOQLStream(client, this);
      return wqt;
    };
    WOQLStreamConfig.prototype.row = function() {
      const wqt = new WOQLStreamRule().scope("row");
      this.rules.push(wqt);
      return wqt;
    };
    WOQLStreamConfig.prototype.template = function(template) {
      if (!template)
        return this.mtemplate;
      this.mtemplate = template;
      return this;
    };
    WOQLStreamConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.stream();\n";
      if (typeof this.template() !== "undefined") {
        str2 += `view.template(${JSON.stringify(this.template())})
`;
      }
      for (let i = 0; i < this.rules.length; i++) {
        str2 += `view.${this.rules[i].prettyPrint()}
`;
      }
      return str2;
    };
    WOQLStreamConfig.prototype.loadJSON = function(config, rules) {
      const jr = [];
      for (let i = 0; i < rules.length; i++) {
        const nr = new WOQLStreamRule();
        nr.json(rules[i]);
        jr.push(nr);
      }
      this.rules = jr;
      if (config.template) {
        this.mtemplate = config.template;
      }
    };
    WOQLStreamConfig.prototype.json = function() {
      const jr = [];
      for (let i = 0; i < this.rules.length; i++) {
        jr.push(this.rules[i].json());
      }
      const conf = {};
      if (this.mtemplate) {
        conf.template = this.mtemplate;
      }
      const mj = { stream: conf, rules: jr };
      return mj;
    };
    function WOQLStreamRule() {
      Config.WOQLViewRule.call(this);
    }
    Object.setPrototypeOf(WOQLStreamRule.prototype, Config.WOQLViewRule.prototype);
    WOQLStreamRule.prototype.template = function(template) {
      if (!template)
        return this.rule.template;
      this.rule.template = template;
      return this;
    };
    module2.exports = WOQLStreamConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/frameRule.js
var require_frameRule = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/frameRule.js"(exports2, module2) {
    var TerminusRule2 = require_terminusRule();
    function FrameRule() {
      TerminusRule2.TerminusRule.call(this);
      this.pattern = new FramePattern();
    }
    Object.setPrototypeOf(FrameRule.prototype, TerminusRule2.TerminusRule.prototype);
    FrameRule.prototype.testRules = function(rules, frame, onmatch) {
      const matched_rules = [];
      if (rules && rules.length) {
        for (let i = 0; i < rules.length; i++) {
          const match = !rules[i].pattern || this.patternMatchesFrame(rules[i].pattern, frame);
          if (match) {
            matched_rules.push(rules[i]);
            if (onmatch && typeof onmatch === "function") {
              onmatch(frame, rules[i]);
            }
          }
        }
      }
      return matched_rules;
    };
    FrameRule.prototype.patternMatchesFrame = function(pattern, frame) {
      if (pattern.checkFrame) {
        return pattern.checkFrame(frame);
      }
      const fp = new FramePattern().setPattern(pattern);
      return fp.checkFrame(frame);
    };
    FrameRule.prototype.property = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.property;
      this.pattern.property = prop;
      return this;
    };
    FrameRule.prototype.frame_type = function(...frame_type) {
      if (!frame_type || frame_type.length === 0)
        return this.pattern.frame_type;
      this.pattern.frame_type = frame_type;
      return this;
    };
    FrameRule.prototype.label = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.label;
      this.pattern.label = prop;
      return this;
    };
    FrameRule.prototype.subject = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.subject;
      this.pattern.subject = prop;
      return this;
    };
    FrameRule.prototype.subjectClass = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.subjectClass;
      this.pattern.subjectClass = prop;
      return this;
    };
    FrameRule.prototype.range = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.range;
      this.pattern.range = prop;
      return this;
    };
    FrameRule.prototype.value = function(...prop) {
      if (!prop || prop.length === 0)
        return this.pattern.value;
      this.pattern.value = prop;
      return this;
    };
    FrameRule.prototype.depth = function(depth) {
      if (typeof depth === "undefined")
        return this.pattern.depth;
      this.pattern.depth = depth;
      return this;
    };
    FrameRule.prototype.index = function(...index) {
      if (!index || index.length === 0)
        return this.pattern.index;
      this.pattern.index = index;
      return this;
    };
    FrameRule.prototype.status = function(...status) {
      if (!status || status.length === 0)
        return this.pattern.status;
      this.pattern.status = status;
      return this;
    };
    FrameRule.prototype.parent = function(par) {
      if (!par)
        return this.pattern.parent;
      this.pattern.parent = par;
      return this;
    };
    FrameRule.prototype.children = function(...children) {
      if (typeof children === "undefined" || children.length === 0)
        return this.pattern.children;
      if (typeof this.pattern.children === "undefined") {
        this.pattern.children = [];
      }
      for (let i = 0; i < children.length; i++) {
        this.pattern.children.push(children[i]);
      }
      return this;
    };
    function FramePattern() {
      TerminusRule2.TerminusPattern.call(this);
    }
    Object.setPrototypeOf(FramePattern.prototype, TerminusRule2.TerminusPattern.prototype);
    FramePattern.prototype.setPattern = function(pattern) {
      if (pattern.scope)
        this.scope = pattern.scope;
      if (typeof pattern.literal !== "undefined")
        this.literal = pattern.literal;
      if (typeof pattern.type !== "undefined")
        this.type = pattern.type;
      if (typeof pattern.label !== "undefined")
        this.label = pattern.label;
      if (typeof pattern.frame_type !== "undefined")
        this.frame_type = pattern.frame_type;
      if (typeof pattern.subject !== "undefined")
        this.subject = pattern.subject;
      if (typeof pattern.subjectClass !== "undefined")
        this.subjectClass = pattern.subjectClass;
      if (typeof pattern.range !== "undefined")
        this.range = pattern.range;
      if (typeof pattern.property !== "undefined")
        this.property = pattern.property;
      if (typeof pattern.value !== "undefined")
        this.value = pattern.value;
      if (typeof pattern.parent !== "undefined") {
        let { parent } = pattern;
        if (typeof parent.json !== "function") {
          parent = new FramePattern().setPattern(parent);
        }
        this.parent = parent;
      }
      if (pattern.children) {
        this.children = [];
        for (let i = 0; i < pattern.children.length; i++) {
          let kid = pattern.children[i];
          if (typeof kid.json !== "function") {
            kid = new FramePattern().setPattern(kid);
          }
          this.children.push(kid);
        }
      }
      if (typeof pattern.depth !== "undefined")
        this.depth = pattern.depth;
      if (typeof pattern.index !== "undefined")
        this.index = pattern.index;
      if (typeof pattern.status !== "undefined")
        this.status = pattern.status;
    };
    FramePattern.prototype.json = function() {
      const json2 = {};
      if (typeof this.literal !== "undefined")
        json2.literal = this.literal;
      if (this.type)
        json2.type = this.type;
      if (this.scope)
        json2.scope = this.scope;
      if (typeof this.value !== "undefined")
        json2.value = this.value;
      if (typeof this.label !== "undefined")
        json2.label = this.label;
      if (typeof this.frame_type !== "undefined")
        json2.frame_type = this.frame_type;
      if (typeof this.subject !== "undefined")
        json2.subject = this.subject;
      if (typeof this.subjectClass !== "undefined")
        json2.subjectClass = this.subjectClass;
      if (typeof this.range !== "undefined")
        json2.range = this.range;
      if (typeof this.property !== "undefined")
        json2.property = this.property;
      if (typeof this.parent !== "undefined")
        json2.parent = this.parent.json ? this.parent.json() : this.parent;
      if (typeof this.children !== "undefined") {
        json2.children = [];
        for (let i = 0; i < this.children.length; i++) {
          json2.children.push(this.children[i].json ? this.children[i].json() : this.children[i]);
        }
      }
      if (typeof this.depth !== "undefined")
        json2.depth = this.depth;
      if (typeof this.index !== "undefined")
        json2.index = this.index;
      if (typeof this.status !== "undefined")
        json2.status = this.status;
      return json2;
    };
    FramePattern.prototype.checkFrame = function(frame) {
      const rtype = this.getRendererType(frame);
      if (!rtype)
        return false;
      if (this.scope && this.scope !== rtype && this.scope !== "*")
        return false;
      if (this.illegalRuleType(rtype))
        return false;
      if (this.frame_type && !this.checkFrameType(rtype, frame))
        return false;
      if (this.label && !this.checkLabel(rtype, frame))
        return false;
      if (this.subject && !this.checkSubject(rtype, frame))
        return false;
      if (this.subjectClass && !this.checkSubjectClass(rtype, frame))
        return false;
      if (this.property && !this.checkProperty(rtype, frame))
        return false;
      if (typeof this.depth !== "undefined" && !this.checkDepth(rtype, frame))
        return false;
      if (this.range && !this.checkRange(rtype, frame))
        return false;
      if (typeof this.value !== "undefined" && !this.checkValue(rtype, frame))
        return false;
      if (this.type && !this.checkType(rtype, frame))
        return false;
      if (typeof this.literal !== "undefined" && !this.checkLiteral(rtype, frame))
        return false;
      if (this.parent && !this.checkParent(rtype, frame))
        return false;
      if (this.children && this.children.length && !this.checkChildren(rtype, frame))
        return false;
      if (this.index && !this.checkIndex(rtype, frame))
        return false;
      if (this.status && !this.checkStatus(rtype, frame))
        return false;
      return true;
    };
    FramePattern.prototype.prettyPrint = function() {
      if (this.scope === "*") {
        var str2 = "all()";
      } else {
        var str2 = `${this.scope}()`;
      }
      if (typeof this.literal !== "undefined") {
        str2 += `.literal(${this.literal})`;
      }
      if (typeof this.type !== "undefined") {
        str2 += `.type(${this.unpack(this.type)})`;
      }
      if (typeof this.range !== "undefined") {
        str2 += `.range(${this.unpack(this.range)})`;
      }
      if (typeof this.frame_type !== "undefined") {
        str2 += `.frame_type(${this.unpack(this.frameType)})`;
      }
      if (typeof this.label !== "undefined") {
        str2 += `.label(${this.unpack(this.label)})`;
      }
      if (typeof this.subject !== "undefined") {
        str2 += `.subject(${this.unpack(this.subject)})`;
      }
      if (typeof this.subjectClass !== "undefined") {
        str2 += `.subjectClass(${this.unpack(this.subjectClass)})`;
      }
      if (typeof this.property !== "undefined") {
        str2 += `.property(${this.unpack(this.property)})`;
      }
      if (typeof this.value !== "undefined") {
        str2 += `.value(${this.unpack(this.value, true)})`;
      }
      if (typeof this.children !== "undefined" && this.children.length > 0) {
        str2 += ".children(\n";
        const kids = this.children;
        for (let i = 0; i < kids.length; i++) {
          str2 += `View.pattern().${kids[i].prettyPrint()}`;
          if (i < kids.length - 1)
            str2 += ",";
          str2 += "\n";
        }
        str2 += ")";
      }
      if (typeof this.parent !== "undefined") {
        str2 += `.parent(View.pattern().${this.parent.prettyPrint()})`;
      }
      if (typeof this.depth !== "undefined") {
        str2 += `.depth(${this.unpack(this.depth, true)})`;
      }
      if (typeof this.index !== "undefined") {
        str2 += `.index(${this.unpack(this.index, true)})`;
      }
      if (typeof this.status !== "undefined") {
        str2 += `.status(${this.unpack(this.status)})`;
      }
      return str2;
    };
    FramePattern.prototype.illegalRuleType = function(rtype) {
      if (rtype === "data" && this.children && this.children.length)
        return true;
      if (rtype === "object" && this.range)
        return true;
      return false;
    };
    FramePattern.prototype.checkSubject = function(subject, frame) {
      if (typeof this.subject !== "object" || !this.subject.length)
        this.subject = [this.subject];
      const rsubj = frame.subject();
      for (let i = 0; i < this.subject.length; i++) {
        if (this.IDsMatch(subject[i], rsubj)) {
          return true;
        }
      }
      return false;
    };
    FramePattern.prototype.checkChildren = function(rtype, frame) {
      for (let i = 0; i < this.children.length; i++) {
        let found = false;
        if (rtype === "object") {
          for (const prop in frame.properties) {
            if (this.children[i].checkFrame(frame.properties[prop])) {
              found = true;
              continue;
            }
          }
        } else if (rtype === "property") {
          for (let j = 0; j <= renderer.values.length; j++) {
            if (this.children[j].checkFrame(frame.values[j])) {
              found = true;
              continue;
            }
          }
        }
        if (!found)
          return false;
      }
      return true;
    };
    FramePattern.prototype.checkStatus = function(rtype, frame) {
      if (typeof this.status !== "object" || this.status.length === 0)
        this.status = [this.status];
      for (let i = 0; i < this.status.length; i++) {
        if (this.status[i] === "updated" && !frame.isUpdated())
          return false;
        if (this.status[i] === "new" && !frame.isNew())
          return false;
        if (this.status[i] === "unchanged" && frame.isUpdated())
          return false;
      }
      return true;
    };
    FramePattern.prototype.checkDepth = function(rtype, frame) {
      return this.numberMatch(this.depth, frame.depth());
    };
    FramePattern.prototype.checkParent = function(rtype, frame) {
      return this.parent.checkFrame(frame.parent);
    };
    FramePattern.prototype.checkIndex = function(rtype, frame) {
      if (rtype === "data") {
        return this.index === frame.index;
      }
      return false;
    };
    FramePattern.prototype.checkProperty = function(rtype, frame) {
      if (typeof this.property !== "object" || !this.property.length)
        this.property = [this.property];
      for (let i = 0; i < this.property.length; i++) {
        if (this.propertyIDsMatch(frame.property(), this.property[i])) {
          return true;
        }
      }
      return false;
    };
    FramePattern.prototype.checkType = function(rtype, frame) {
      if (rtype === "object")
        var vs = frame.subjectClass();
      else
        var vs = typeof frame.range === "function" ? frame.range() : frame.range;
      if (!Array.isArray(this.type))
        this.type = [this.type];
      if (this.type.indexOf(vs) === -1)
        return false;
      return true;
    };
    FramePattern.prototype.checkLiteral = function(rtype, frame) {
      if (rtype === "object")
        return false;
      if (rtype === "property")
        return false;
      if (rtype === "data")
        return frame.isDatatypeProperty();
      return true;
    };
    FramePattern.prototype.checkValue = function(rtype, frame) {
      if (typeof this.value !== "object" || !this.value.length)
        this.value = [this.value];
      for (let i = 0; i < this.value.length; i++) {
        if (rtype === "data") {
          if (this.valuesMatch(frame.get(), this.value[i])) {
            return true;
          }
        } else if (rtype === "property") {
          for (let j = 0; j <= frame.values.length; j++) {
            if (this.getRendererType(frame.values[i]) === "data" && this.valuesMatch(frame.values[i].get(), this.value[i])) {
              return true;
            }
          }
        } else if (rtype === "object") {
          for (const prop in frame.properties) {
            if (this.checkValue(this.getRendererType(frame.properties[prop]), frame.properties[prop])) {
              return true;
            }
          }
        }
      }
      return false;
    };
    FramePattern.prototype.checkRange = function(rtype, frame) {
      if (typeof this.range !== "object" || !this.range.length)
        this.range = [this.range];
      for (let i = 0; i < this.range.length; i++) {
        if (this.rangeIDsMatch(frame.range(), this.range[i])) {
          return true;
        }
      }
      return false;
    };
    FramePattern.prototype.checkSubjectClass = function(rtype, frame) {
      if (typeof this.subjectClass !== "object" || !this.subjectClass.length)
        this.subjectClass = [this.subjectClass];
      const rcls = frame.subjectClass();
      for (let i = 0; i < this.subjectClass.length; i++) {
        if (this.classIDsMatch(this.subjectClass[i], rcls)) {
          return true;
        }
      }
      return false;
    };
    FramePattern.prototype.checkFrameType = function(rtype, frame) {
      if (rtype === "object")
        return this.frame_type.indexOf("object") !== -1;
      if (rtype === "data") {
        if (frame.frame) {
          return this.frame_type.indexOf(frame.frame.ftype()) !== -1;
        }
      }
      if (rtype === "property")
        return false;
    };
    FramePattern.prototype.checkLabel = function(rtype, frame) {
      if (typeof frame.getLabel !== "function") {
        console.log(new Error("Rule passed to check label with broken renderer object - no getLabel"));
        return false;
      }
      for (let i = 0; i < this.label.length; i++) {
        if (this.stringMatch(this.label[i], frame.getLabel()))
          return true;
      }
      return false;
    };
    FramePattern.prototype.getRendererType = function(frame) {
      if (frame.isProperty())
        return "property";
      if (frame.isObject())
        return "object";
      if (frame.isData())
        return "data";
      if (frame.renderer_type)
        return frame.renderer_type;
      console.log(frame, new Error(`frame configuration passed non-renderer type ${frame.constructor.name}`));
      return false;
    };
    module2.exports = { FrameRule, FramePattern };
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/objectFrame.js
var require_objectFrame = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/objectFrame.js"(exports2, module2) {
    var FrameHelper = require_utils();
    var { FrameRule } = require_frameRule();
    var WOQL2 = require_woql();
    function ObjectFrame(cls, jsonld, classframes, parent) {
      this.empty();
      this.cls = FrameHelper.unshorten(cls);
      if (classframes && typeof classframes === "object") {
        this.loadClassFrames(classframes);
      }
      if (jsonld && typeof jsonld === "object" && Object.keys(jsonld).length) {
        this.originalDocument = jsonld;
        this.loadJSONLDDocument(jsonld);
      } else {
        this.originalDocument = false;
      }
      this.parent = parent;
      this.newDoc = false;
    }
    ObjectFrame.prototype.loadClassFrames = function(classframes) {
      for (let j = 0; j < classframes.length; j += 1) {
        if (classframes[j]["@context"])
          this.jsonld_context = classframes[j]["@context"];
        const cf = new ClassFrame(classframes[j], this);
        if (cf.isValid()) {
          if (!this.classframes)
            this.classframes = {};
          this.classframes[classframes[j].property] = cf;
          if (cf.isObject() && this.properties[classframes[j].property]) {
            for (let i = 0; i < this.properties[classframes[j].property].values.length; i += 1) {
              this.properties[classframes[j].property].values[i].loadClassFrames(classframes[j].frame);
            }
          }
        } else {
          console.log("Invalid classframe", cf);
        }
      }
      return this;
    };
    ObjectFrame.prototype.hasSchema = function() {
      return !FrameHelper.empty(this.classframes);
    };
    ObjectFrame.prototype.loadJSONLDDocument = function(rdoc) {
      if (typeof rdoc !== "object")
        return void 0;
      const doc = FrameHelper.json_unshorten(rdoc);
      if (!this.originalDocument)
        this.originalDocument = doc;
      if (!this.subjid && doc["@id"]) {
        this.subjid = FrameHelper.unshorten(doc["@id"]);
      }
      if (doc["@context"])
        this.jsonld_context = doc["@context"];
      for (const prop in doc) {
        if (prop[0] === "@" || typeof doc[prop] === "object" && Object.keys(doc[prop]).length === 0)
          continue;
        let cframe = this.getPropertyClassFrame(prop, doc);
        if (cframe && cframe.isClassChoice()) {
          if (!cframe) {
            console.log(`no choice frame ${doc[prop]["@type"]}`);
          }
        } else if (cframe && cframe.isLogic()) {
          cframe = cframe.getChosenFrame(doc[prop]);
        }
        if (cframe) {
          if (typeof this.properties[prop] === "undefined") {
            this.properties[prop] = new PropertyFrame(prop, cframe, this);
          }
          this.properties[prop].addJSONLDDocument(doc[prop]);
        }
      }
      return this;
    };
    ObjectFrame.prototype.getAsFrame = function(prop, parent) {
      prop = FrameHelper.unshorten(prop);
      if (this.parentframe)
        return this.parentframe;
      const ff = { type: "objectProperty", property: prop };
      ff.range = this.cls;
      ff.domain = parent.cls;
      ff.domainValue = parent.subjid;
      ff.frame = [];
      for (const prop2 of Object.keys(this.properties)) {
        ff.frame = ff.frame.concat(ff.frame, this.properties[prop2].getAsFrames());
      }
      return ff;
    };
    ObjectFrame.prototype.getAsFrames = function(prop, parent) {
      let frames = [];
      for (const prop2 of Object.keys(this.properties)) {
        frames = frames.concat(frames, this.properties[prop2].getAsFrames());
      }
      return frames;
    };
    ObjectFrame.prototype.empty = function() {
      this.properties = {};
      this.restrictions = {};
      this.subjid = false;
      this.cls = false;
    };
    ObjectFrame.prototype.reset = function(prop) {
      if (prop) {
        prop = FrameHelper.unshorten(prop);
        const props = [];
        for (let i = 0; i < this.originalFrames.length; i += 1) {
          if (this.originalFrames[i].property === prop) {
            props.push(this.originalFrames[i]);
          }
        }
        if (this.properties[prop])
          this.properties[prop] = [];
        this.loadDataFrames(props);
      } else {
        this.restrictions = {};
        this.properties = {};
        this.loadDataFrames(this.originalFrames);
      }
    };
    ObjectFrame.prototype.clear = function() {
      for (const prop of Object.keys(this.properties)) {
        this.properties[prop].clear();
      }
      return this;
    };
    ObjectFrame.prototype.mfilter = function(rules, onmatch) {
      const hits = new FrameRule().testRules(rules, this, onmatch);
      for (const prop of Object.keys(this.properties)) {
        if (!this.properties[prop].mfilter) {
          console.log(prop, this.properties[prop]);
        } else {
          this.properties[prop].mfilter(rules, onmatch);
        }
      }
      return this;
    };
    ObjectFrame.prototype.getPropertyClassFrame = function(prop, jsonlddoc) {
      if (typeof prop === "object") {
        return new ClassFrame(prop);
      }
      prop = FrameHelper.unshorten(prop);
      if (this.classframes && typeof this.classframes === "object" && typeof this.classframes[prop] === "object") {
        return this.classframes[prop];
      }
      if (jsonlddoc) {
        const cf = new ClassFrame();
        cf.loadFromJSONLD(jsonlddoc, prop);
        return cf;
      }
      if (this.properties[prop]) {
        return new ClassFrame(this.properties[prop].values[0]);
      }
      return false;
    };
    ObjectFrame.prototype.getProperties = function(type) {
      if (type === "filled" || !this.classframes) {
        return Object.keys(this.properties);
      }
      if (type === "missing") {
        const filled = Object.keys(this.properties).map((item) => FrameHelper.unshorten(item));
        const all = Object.keys(this.classframes).map((item) => FrameHelper.unshorten(item));
        const missing = [];
        for (let i = 0; i < all.length; i++) {
          if (filled.indexOf(all[i]) === -1 && missing.indexOf(all[i]) === -1) {
            missing.push(all[i]);
          }
        }
        return missing;
      }
      return Object.keys(this.classframes);
    };
    ObjectFrame.prototype.getMissingPropertyList = function() {
      const missing = this.getProperties("missing");
      const nmissing = [];
      for (let i = 0; i < missing.length; i++) {
        const cframe = this.getPropertyClassFrame(missing[i]);
        if (cframe) {
          var newb = { label: cframe.getLabel(), value: missing[i] };
        } else {
          var newb = { label: missing[i], value: missing[i] };
        }
        nmissing.push(newb);
      }
      return nmissing;
    };
    ObjectFrame.prototype.getPossibleContainedClasses = function() {
      const cls = [];
      function efcf(frames) {
        if (frames.type && frames.type === "class_choice") {
          efcf(frames.operands);
        }
        if (!Array.isArray(frames))
          frames = [frames];
        for (let i = 0; i < frames.length; i++) {
          if (frames[i].domain && cls.indexOf(frames[i].domain) === -1)
            cls.push(frames[i].domain);
          if (frames[i].frame) {
            efcf(frames[i].frame);
          }
        }
      }
      efcf(Object.values(this.classframes));
      return cls;
    };
    ObjectFrame.prototype.getDocumentLinks = function() {
      const vals = [];
      const props = this.getProperties("filled");
      for (let i = 0; i < props.length; i++) {
        const mprop = this.properties[props[i]];
        for (let k = 0; k < mprop.values.length; k++) {
          const dval = mprop.values[k];
          if (dval.isObject() && dval.getDocumentLinks) {
            const nvals = dval.getDocumentLinks();
            for (let l = 0; l < nvals.length; l++) {
              if (vals.indexOf(nvals[l]) === -1) {
                vals.push(nvals[l]);
              }
            }
          } else if (dval.isDocument()) {
            const nv = dval.get();
            if (vals.indexOf(nv) === -1) {
              vals.push(nv);
            }
          }
        }
      }
      return vals;
    };
    ObjectFrame.prototype.getFilledPropertyList = function() {
      const props = this.getProperties("filled");
      const filled = [];
      for (let i = 0; i < props.length; i++) {
        const cframe = this.getPropertyClassFrame(props[i]);
        if (cframe) {
          var newb = { label: cframe.getLabel(), value: props[i] };
        } else {
          var newb = { label: props[i], value: props[i] };
        }
        filled.push(newb);
      }
      return filled;
    };
    ObjectFrame.prototype.fillFromSchema = function(newid) {
      if (newid)
        this.subjid = newid;
      newid = newid || FrameHelper.genBNID(`${FrameHelper.urlFragment(this.cls)}_`);
      const properties = {};
      if (this.classframes) {
        for (const prop of Object.keys(this.classframes)) {
          const pf = this.getPropertyClassFrame(prop);
          properties[prop] = new PropertyFrame(prop, pf, this);
          properties[prop].fillFromSchema(newid);
        }
      }
      this.properties = properties;
      this.originalFrames = [];
      for (const prop of Object.keys(this.properties)) {
        this.originalFrames.push(this.properties[prop].getAsFrames());
      }
      return this;
    };
    ObjectFrame.prototype.clone = function(newid) {
      const properties = {};
      const cloned = new ObjectFrame(this.cls, false, false, this.parent);
      cloned.classframes = this.classframes;
      cloned.subjid = newid;
      for (const prop of Object.keys(this.properties)) {
        properties[prop] = this.properties[prop].clone();
      }
      cloned.properties = properties;
      return cloned;
    };
    ObjectFrame.prototype.getChild = function(childid, prop) {
      let pframe = this.getProperty(prop);
      for (let i = 0; i < pframe.values.length; pframe++) {
        if (pframe.values[i].isObject() && pframe.values[i].subject === childid)
          return pframe.values[i];
      }
      if (!prop) {
        for (const key of Object.keys(this.properties)) {
          for (let i = 0; i < this.properties[key].values.length; i += 1) {
            if (this.properties[key].values[i].subject() === childid)
              return this.properties[key].values[i];
          }
        }
      }
      return false;
    };
    ObjectFrame.prototype.addProperty = function(prop, cls) {
      if (typeof prop !== "object")
        prop = FrameHelper.unshorten(prop);
      const cframe = this.getPropertyClassFrame(prop);
      let ndata = false;
      if (cframe) {
        const nprop = new PropertyFrame(prop, cframe, this);
        if (cframe.isObject()) {
          if (!cframe.isClassChoice()) {
            ndata = cframe.createEmpty(FrameHelper.genBNID(`${FrameHelper.urlFragment(cframe.range)}_`));
          }
          if (cls) {
            ndata = cframe.createEmptyChoice(cls, FrameHelper.genBNID(`${FrameHelper.urlFragment(cls)}_`));
          }
          const clss = cframe.getClassChoices();
          if (clss && clss.length) {
            ndata = cframe.createEmptyChoice(clss[0], FrameHelper.genBNID(`${FrameHelper.urlFragment(clss[0])}_`));
          }
        } else {
          ndata = cframe.createEmpty();
        }
        if (ndata) {
          nprop.addValueFrame(ndata);
        }
        if (typeof this.properties[prop] === "undefined") {
          if (typeof prop === "object")
            var p = prop.property;
          else
            var p = prop;
          this.properties[p] = nprop;
        }
        nprop.status = "new";
        return nprop;
      }
      return false;
    };
    ObjectFrame.prototype.addPropertyValue = function(prop, value) {
      prop = FrameHelper.unshorten(prop);
      if (this.properties[prop])
        return this.properties[prop].addValue(value);
      return null;
    };
    ObjectFrame.prototype.removeProperty = function(prop) {
      prop = FrameHelper.unshorten(prop);
      if (typeof this.properties[prop] !== "undefined") {
        delete this.properties[prop];
      }
    };
    ObjectFrame.prototype.removePropertyValue = function(prop, value, index) {
      prop = FrameHelper.unshorten(prop);
      const pframe = this.properties[prop];
      pframe.removeValue(value, index);
      if (pframe.values.length === 0) {
        this.removeProperty(prop);
      }
    };
    ObjectFrame.prototype.error = function(msg) {
      if (!this.errors)
        this.errors = [];
      this.errors.push({ type: "Internal Object Frame Error", msg });
    };
    ObjectFrame.prototype.extract = function() {
      const extracts = {};
      for (const prop in this.properties) {
        const extracted = this.properties[prop].extract();
        if (!FrameHelper.empty(extracted)) {
          if (typeof extracts[prop] === "undefined")
            extracts[prop] = [];
          extracts[prop] = extracts[prop].concat(extracted);
        }
        if (extracts[prop] && extracts[prop].length === 1)
          extracts[prop] = extracts[prop][0];
      }
      if (FrameHelper.empty(extracts) && this.parent) {
        return false;
      }
      const ext = this.extractJSONLD(extracts);
      return ext;
    };
    ObjectFrame.prototype.extractJSONLD = function(extracts) {
      extracts["@type"] = this.cls;
      if (this.subject() !== "_:")
        extracts["@id"] = this.subject();
      if (this.jsonld_context)
        extracts["@context"] = this.jsonld_context;
      return extracts;
    };
    ObjectFrame.prototype.subject = function() {
      return this.subjid || "";
    };
    ObjectFrame.prototype.get = ObjectFrame.prototype.subject;
    ObjectFrame.prototype.set = function(val) {
      this.subjid = val;
    };
    ObjectFrame.prototype.isObject = function() {
      return true;
    };
    ObjectFrame.prototype.isProperty = function() {
      return false;
    };
    ObjectFrame.prototype.isData = function() {
      return false;
    };
    ObjectFrame.prototype.isClassChoice = function() {
      return this.frame && this.frame.type === "class_choice";
    };
    ObjectFrame.prototype.subjectClass = function() {
      return this.cls;
    };
    ObjectFrame.prototype.depth = function() {
      if (this.parent)
        return this.parent.depth() + 1;
      return 0;
    };
    ObjectFrame.prototype.getProperty = function(prop) {
      return this.properties[prop];
    };
    ObjectFrame.prototype.first = function(prop) {
      if (this.properties && this.properties[prop]) {
        return this.properties[prop].first();
      }
    };
    ObjectFrame.prototype.property = function(prop) {
      if (this.parent)
        return this.parent.property();
      return false;
    };
    ObjectFrame.prototype.parentObject = function() {
      if (this.parent && this.parent.parent) {
        return this.parent.parent;
      }
      return false;
    };
    ObjectFrame.prototype.root = function() {
      if (this.parent)
        return false;
      return true;
    };
    ObjectFrame.prototype.renderProperties = function() {
      const props = this.sortProperties();
      const nprops = [];
      for (let i = 0; i < props.length; i++) {
        if (this.properties[props[i]].render) {
          const rend = this.properties[props[i]].render(this.properties[props[i]]);
          if (rend)
            nprops.push(rend);
        }
      }
      return nprops;
    };
    ObjectFrame.prototype.sortProperties = function() {
      const unsorted = Object.keys(this.properties);
      if (this.compare) {
        return unsorted.sort((a, b) => this.compare(a, b, this));
      }
      return unsorted.sort((a, b) => this.standardCompare(a, b, this));
    };
    ObjectFrame.prototype.standardCompare = function(a, b, doc) {
      if (FrameHelper.shorten(a) === "rdfs:label")
        return -1;
      if (FrameHelper.shorten(b) === "rdfs:label")
        return 1;
      if (FrameHelper.shorten(a) === "rdfs:comment")
        return -1;
      if (FrameHelper.shorten(b) === "rdfs:comment")
        return 1;
      if (doc.properties[a].isData() && doc.properties[b].isObject())
        return -1;
      if (doc.properties[b].isData() && doc.properties[a].isObject())
        return 1;
      return 0;
    };
    ObjectFrame.prototype.cardControlAllows = function(action) {
      if (!this.parent)
        return true;
      if (this.parent.cframe.hasRestriction()) {
        const rest = this.parent.cframe.restriction;
        const currentnum = this.parent.values.length;
        if (action === "add" || action === "clone") {
          if (rest.max && currentnum >= rest.max) {
            return false;
          }
        }
        if (action === "delete" && (rest.min && currentnum <= rest.min)) {
          return false;
        }
      }
      return true;
    };
    ObjectFrame.prototype.isUpdated = function() {
      let i = 0;
      for (const prop in this.properties) {
        if (this.originalFrames[i] !== prop)
          return true;
        if (this.properties[prop].isUpdated())
          return true;
        i++;
      }
      if (i !== this.originalFrames.length)
        return true;
      return false;
    };
    ObjectFrame.prototype.isNew = function() {
      return this.subject().substring(0, 2) === "_:";
    };
    ObjectFrame.prototype.getSummary = function() {
      const ret = { status: "ok" };
      if (this.isUpdated())
        ret.status = "updated";
      if (this.isNew())
        ret.status = "new";
      ret.propcount = 0;
      for (const prop in this.properties) {
        ret.propcount++;
      }
      ret.long = `${ret.propcount} properties`;
      return ret;
    };
    ObjectFrame.prototype.saveQuery = function() {
      const q = WOQL2.update_object(this.extract());
      this.pathToDoc(q);
      return q;
    };
    ObjectFrame.prototype.pathToDoc = function(q) {
      q.add_triple(this.subjid, "type", this.cls);
      if (this.parent) {
        q.add_triple(this.parent.subject(), "type", this.parent.subjectClass());
        q.add_triple(this.parent.subject(), this.parent.predicate, this.subjid);
        if (this.parent.parent) {
          this.parent.parent.pathToDoc(q);
        }
      }
    };
    ObjectFrame.prototype.deleteQuery = function() {
      const q = WOQL2.delete_object(this.subjid);
      if (this.parent) {
        q.delete_triple(this.parent.subject(), this.parent.predicate, this.subjid);
      }
      return q;
    };
    function PropertyFrame(property, cframe, parent) {
      this.predicate = property;
      this.cframe = cframe;
      this.parent = parent;
      this.values = [];
    }
    PropertyFrame.prototype.addJSONLDDocument = function(jsonld) {
      if (this.cframe.isData()) {
        if (Array.isArray(jsonld)) {
          for (var i = 0; i < jsonld.length; i++) {
            var df = new DataFrame(jsonld[i], this, this.values.length);
            this.values.push(df);
          }
        } else {
          var df = new DataFrame(jsonld, this, this.values.length);
          this.values.push(df);
        }
      } else if (Array.isArray(jsonld)) {
        for (var i = 0; i < jsonld.length; i++) {
          const kid = new ObjectFrame(FrameHelper.unshorten(jsonld[i]["@type"]), jsonld[i], this.cframe.frame, this);
          this.values.push(kid);
        }
      } else {
        const kid = new ObjectFrame(jsonld["@type"], jsonld, this.cframe.frame, this);
        this.values.push(kid);
      }
    };
    PropertyFrame.prototype.addFrame = function(frame) {
      if (this.cframe.isData()) {
        const df = new DataFrame(frame, this, this.values.length);
        this.values.push(df);
      } else {
        const kid = new ObjectFrame(this.range(), this.cframe.frame, frame.frame, this, frame);
        this.values.push(kid);
      }
    };
    PropertyFrame.prototype.addValueFrame = function(oframe) {
      if (oframe) {
        oframe.parent = this;
        oframe.index = this.values.length;
        this.values.push(oframe);
      }
    };
    PropertyFrame.prototype.addValue = function(val) {
      const nu = this.createEmpty();
      if (val)
        nu.set(val);
      this.addValueFrame(nu);
      return nu;
    };
    PropertyFrame.prototype.fillFromSchema = function(newid) {
      if (this.isData() || this.isObject() && !this.isClassChoice()) {
        const values = [];
        if (this.cframe.hasRestriction() && this.cframe.restriction.min) {
          for (let i = 0; i < this.cframe.restriction.min; i += 1) {
            var nframe = this.createEmpty(newid);
            nframe.parent = this;
            values.push(nframe);
          }
        } else {
          var nframe = this.createEmpty(newid);
          nframe.parent = this;
          values.push(nframe);
        }
        this.values = values;
      } else if (this.isClassChoice()) {
        const clss = this.cframe.getClassChoices();
        if (clss && clss.length) {
          const empty = this.cframe.createEmptyChoice(clss[0], FrameHelper.genBNID(`${FrameHelper.urlFragment(clss[0])}_`));
          empty.parent = this;
          this.values = [empty];
        }
      }
    };
    PropertyFrame.prototype.isData = function() {
      return this.cframe.isData();
    };
    PropertyFrame.prototype.isObject = function() {
      return this.cframe.isObject();
    };
    PropertyFrame.prototype.isProperty = function() {
      return true;
    };
    PropertyFrame.prototype.property = function() {
      return this.predicate;
    };
    PropertyFrame.prototype.extract = function() {
      const extracts = [];
      const hasVal = (val) => {
        if (val["@value"]) {
          for (var i = 0; i < extracts.length; i++) {
            if (extracts[i]["@value"] && extracts[i]["@value"] === val["@value"] && extracts[i]["@type"] && extracts[i]["@type"] === val["@type"])
              return true;
          }
          return false;
        }
        if (val["@id"]) {
          for (var i = 0; i < extracts.length; i++) {
            if (extracts[i]["@id"] && extracts[i]["@id"] === val["@id"])
              return true;
          }
          return false;
        }
      };
      for (let i = 0; i < this.values.length; i++) {
        const val = this.values[i].extract();
        if (val !== "" && val !== false && typeof val !== "undefined" && !hasVal(val))
          extracts.push(val);
      }
      return extracts;
    };
    PropertyFrame.prototype.subject = function() {
      return this.parent ? this.parent.subject() : false;
    };
    PropertyFrame.prototype.subjectClass = function() {
      return this.parent ? this.parent.subjectClass() : false;
    };
    PropertyFrame.prototype.depth = function() {
      return this.parent ? this.parent.depth() : false;
    };
    PropertyFrame.prototype.updated = function() {
      return this.parent ? this.parent.childUpdated() : false;
    };
    PropertyFrame.prototype.range = function() {
      return this.cframe ? this.cframe.range : "";
    };
    PropertyFrame.prototype.getLabel = function() {
      return (
        // this.cframe ? this.cframe.getLabel() : '');
        this.cframe ? this.cframe.getLabel() : this.predicate.getLabel()
      );
    };
    PropertyFrame.prototype.getComment = function() {
      return this.cframe ? this.cframe.getComment() : false;
    };
    PropertyFrame.prototype.hasCardinalityRestriction = function() {
      return this.cframe ? this.cframe.hasRestriction() : false;
    };
    PropertyFrame.prototype.getRestriction = function() {
      return this.cframe ? this.cframe.restriction : false;
    };
    PropertyFrame.prototype.isClassChoice = function() {
      return this.cframe ? this.cframe.isClassChoice() : false;
    };
    PropertyFrame.prototype.deletePropertyValue = function(value, index) {
      this.parent.removePropertyValue(this.property(), value, index);
    };
    PropertyFrame.prototype.removeValue = function(value, index) {
      const nvals = [];
      for (let i = 0; i < this.values.length; i++) {
        if (this.values[i].index !== value.index) {
          nvals.push(this.values[i]);
        }
      }
      this.values = nvals;
    };
    PropertyFrame.prototype.get = function() {
      const gets = [];
      for (let i = 0; i < this.values.length; i++) {
        if (this.values[i]) {
          const x = this.values[i].get();
          if (x)
            gets.push(x);
        }
      }
      return gets;
    };
    PropertyFrame.prototype.set = function(val) {
      for (let i = 0; i < this.values.length; i++) {
        if (this.values[i]) {
          this.values[i].set(val);
        }
      }
    };
    PropertyFrame.prototype.clear = function() {
      for (let i = 0; i < this.values.length; i++) {
        this.values[i].clear();
      }
    };
    PropertyFrame.prototype.clone = function() {
      const cvalues = [];
      const cloned = new PropertyFrame(this.predicate, this.cframe, this.parent);
      for (let i = 0; i < this.values.length; i++) {
        cvalues.push(this.values[i].clone());
      }
      cloned.values = cvalues;
      return cloned;
    };
    PropertyFrame.prototype.getAsFrames = function() {
      let fs = [];
      for (let i = 0; i < this.values.length; i++) {
        if (this.values[i]) {
          if (this.isData()) {
            fs.push(this.values[i].getAsFrame());
          } else {
            fs = fs.concat(this.values[i].getAsFrames());
          }
        }
      }
      return fs;
    };
    PropertyFrame.prototype.createEmpty = function() {
      if (this.cframe.isData()) {
        const df = this.cframe.copy(this.subject());
        df.set("");
        df.status = "new";
        return df;
      }
      if (this.cframe.isObject()) {
        if (!this.cframe.isClassChoice()) {
          const df2 = this.cframe.createEmpty(FrameHelper.genBNID(`${FrameHelper.urlFragment(this.cframe.range)}_`));
          df2.status = "new";
          return df2;
        }
        const df = new ClassFrame(this.cframe);
        df.status = "new";
        return df;
      }
    };
    PropertyFrame.prototype.mfilter = function(rules, onmatch) {
      const hits = new FrameRule().testRules(rules, this, onmatch);
      for (let i = 0; i < this.values.length; i++) {
        this.values[i].mfilter(rules, onmatch);
      }
      return this;
    };
    PropertyFrame.prototype.first = function() {
      if (this.values && this.values[0]) {
        return this.values[0].get();
      }
    };
    PropertyFrame.prototype.renderValues = function() {
      const sortedVals = this.sortValues();
      const vals = [];
      for (let i = 0; i < sortedVals.length; i++) {
        if (sortedVals[i] && sortedVals[i].render) {
          const rend = sortedVals[i].render(sortedVals[i]);
          if (rend)
            vals.push(rend);
        }
      }
      return vals;
    };
    PropertyFrame.prototype.sortValues = function() {
      if (this.compare) {
        return this.values.sort((a, b) => this.compare(a, b, this));
      }
      return this.values;
    };
    PropertyFrame.prototype.cardControlAllows = function(action) {
      if (this.cframe.hasRestriction()) {
        const rest = this.cframe.restriction;
        const currentnum = this.values.length;
        if (action === "add" || action === "clone") {
          if (rest.max && currentnum >= rest.max) {
            return false;
          }
        }
        if (action === "delete" && rest.min) {
          return false;
        }
      }
      return true;
    };
    PropertyFrame.prototype.isUpdated = function() {
      return true;
      if (this.values.length !== this.originalValues.length)
        return true;
      for (let i = 0; i < this.values.length; i++) {
        if (this.cframe && this.cframe.isData()) {
          if (this.values[i].value() !== this.originalValues[i]) {
            return true;
          }
        } else {
          if (this.values[i].subject() !== this.originalValues[i]) {
            return true;
          }
          if (this.values[i].isUpdated()) {
            return true;
          }
        }
      }
      return false;
    };
    PropertyFrame.prototype.saveQuery = function() {
      const q = WOQL2.query();
      for (let i = 0; i < this.values.length; i++) {
        q.and(this.values[i].saveQuery());
      }
      this.parent.pathToDoc(q);
      return q;
    };
    PropertyFrame.prototype.deleteQuery = function() {
      const q = WOQL2.query();
      for (let i = 0; i < this.values.length; i++) {
        q.and(this.values[i].deleteQuery());
      }
      return q;
    };
    function DataFrame(jsonld, parent, index) {
      this.err = false;
      this.index = index;
      if (parent) {
        this.loadParent(parent);
      }
      if (jsonld) {
        this.rangeValue = jsonld;
        if (jsonld["@type"])
          this.range = jsonld["@type"];
        if (jsonld["@language"])
          this.language = jsonld["@language"];
        if (!this.type)
          this.type = jsonld["@value"] ? "datatypeProperty" : "objectProperty";
      }
      return this;
    }
    DataFrame.prototype.loadParent = function(parent, index) {
      this.parent = parent;
      this.type = parent.type;
      this.domainValue = parent.subject();
      this.subjid = parent.subject() || false;
      this.domain = parent.cframe ? parent.cframe.domain : parent.subjectClass();
      this.predicate = parent.cframe ? parent.cframe.predicate : parent.property();
      this.frame = parent.cframe ? parent.cframe.frame : false;
      this.label = parent.getLabel();
      this.comment = parent.getComment();
      this.range = parent && parent.cframe ? parent.cframe.range : false;
      const restriction = parent.cframe ? parent.cframe.restriction : false;
      this.restriction = false;
      if (restriction && typeof restriction === "object") {
        this.restriction = new Restriction(restriction);
      }
    };
    DataFrame.prototype.copy = function(newid) {
      const copy = new DataFrame();
      copy.parent = this.parent;
      copy.range = this.range;
      copy.rangeValue = this.rangeValue;
      copy.index = this.index;
      copy.type = this.type;
      copy.domain = this.domain;
      copy.domainValue = newid || this.domainValue;
      copy.predicate = this.predicate;
      copy.frame = this.frame;
      copy.label = this.label;
      copy.comment = this.comment;
      if (this.restriction)
        copy.restriction = this.restriction;
      if (this.language)
        copy.language = this.language;
      return copy;
    };
    DataFrame.prototype.mfilter = function(rules, onmatch) {
      const hits = new FrameRule().testRules(rules, this, onmatch);
      return this;
    };
    DataFrame.prototype.reset = function() {
      this.set(this.originalValue);
    };
    DataFrame.prototype.clone = function() {
      const newb = this.parent.addPropertyValue(this.get());
    };
    DataFrame.prototype.depth = function() {
      return this.parent ? this.parent.depth() : false;
    };
    DataFrame.prototype.property = function() {
      return this.parent ? this.parent.property() : false;
    };
    DataFrame.prototype.subject = function() {
      return this.parent ? this.parent.subject() : false;
    };
    DataFrame.prototype.subjectClass = function() {
      return this.parent ? this.parent.subjectClass() : false;
    };
    DataFrame.prototype.type = function() {
      return this.range ? this.range : false;
    };
    DataFrame.prototype.isValidType = function(dt) {
      const vtypes = ["datatypeProperty", "objectProperty", "restriction "];
      if (vtypes.indexOf(dt) === -1)
        return false;
      return true;
    };
    DataFrame.prototype.getAsFrame = function() {
      const ff = { type: this.type, property: this.property() };
      if (this.range)
        ff.range = this.range;
      if (this.rangeValue)
        ff.rangeValue = this.rangeValue;
      if (this.domain)
        ff.domain = this.domain;
      if (this.domainValue)
        ff.domainValue = this.domainValue;
      if (this.frame)
        ff.frame = this.frame;
      if (this.label)
        ff.label = this.label;
      if (this.comment)
        ff.comment = this.comment;
      return ff;
    };
    DataFrame.prototype.hasRestriction = function() {
      if (this.restriction) {
        return this.restriction.hasCardRestriction();
      }
      return false;
    };
    DataFrame.prototype.getLabel = function() {
      let lab = "";
      if (FrameHelper.shorten(this.predicate) === "rdfs:label")
        return "Name";
      if (FrameHelper.shorten(this.predicate) === "rdfs:comment")
        return "Description";
      if (this.label && typeof this.label === "object")
        lab = this.label["@value"];
      if (this.label && typeof this.label === "string")
        lab = this.label;
      if (!lab && this.predicate) {
        lab = FrameHelper.labelFromURL(this.predicate);
      }
      if (!lab)
        lab = FrameHelper.labelFromURL(this.cls);
      return lab;
    };
    DataFrame.prototype.getType = function() {
      if (this.range)
        return this.range;
      if (this.rangeValue && this.rangeValue["@type"])
        return this.rangeValue["@type"];
      return false;
    };
    ObjectFrame.prototype.getLabel = DataFrame.prototype.getLabel;
    DataFrame.prototype.getComment = function() {
      let comment = "";
      if (this.comment && typeof this.comment === "object")
        comment = this.comment["@value"];
      if (this.comment && typeof this.comment === "string")
        comment = this.comment;
      return comment;
    };
    ObjectFrame.prototype.getComment = DataFrame.prototype.getComment;
    DataFrame.prototype.error = function(msg) {
      if (msg)
        this.err = msg;
      if (!this.errors)
        this.errors = [];
      this.errors.push({ type: "Internal Data Frame Error", msg });
      return this.err;
    };
    DataFrame.prototype.isValid = function() {
      if (!(this.type && this.isValidType(this.type))) {
        this.error(`Missing or Illegal Frame Type ${this.type}`);
        return false;
      }
      if (!this.predicate) {
        this.error("Missing Frame Property");
        return false;
      }
      if (!this.domain) {
        this.error("Missing Frame Domain");
        return false;
      }
      if (!this.range) {
        this.error("Missing Frame Range");
        return false;
      }
      if (this.isObjectProperty() && !(this.frame && typeof this.frame === "object")) {
        this.error("Missing Object Frame");
        return false;
      }
      return true;
    };
    DataFrame.prototype.isObjectProperty = function() {
      return this.type === "objectProperty";
    };
    DataFrame.prototype.isData = function() {
      return true;
    };
    DataFrame.prototype.isDatatypeProperty = function() {
      return this.type === "datatypeProperty";
    };
    DataFrame.prototype.isLogic = function() {
      if (this.type === "and" || this.type === "or" || this.type === "xor") {
        return true;
      }
      return false;
    };
    DataFrame.prototype.isRestriction = function() {
      return this.type === "restriction";
    };
    DataFrame.prototype.ftype = function() {
      if (this.isDocument())
        return "document";
      if (this.isDatatypeProperty())
        return "data";
      if (this.isChoice())
        return "oneOf";
      if (this.isObject())
        return "object";
      if (this.isLogic())
        return "logic";
      if (this.isClassChoice())
        return "class_choice";
      return void 0;
    };
    DataFrame.prototype.isClassChoice = function() {
      return this.frame && this.frame.type === "class_choice";
    };
    DataFrame.prototype.isString = function() {
      if (this.range === FrameHelper.getStdURL("xsd", "string")) {
        return true;
      }
      return false;
    };
    DataFrame.prototype.getChoiceOptions = function() {
      const opts2 = [];
      for (let i = 0; i < this.frame.elements.length; i += 1) {
        const option = {};
        if (this.frame.elements[i].label) {
          option.label = this.frame.elements[i].label["@value"];
        } else {
          option.label = FrameHelper.labelFromURL(this.frame.elements[i].class);
        }
        option.value = this.frame.elements[i].class;
        opts2.push(option);
      }
      return opts2;
    };
    DataFrame.prototype.lang = function() {
      return this.language || "en";
    };
    DataFrame.prototype.isChoice = function() {
      return this.frame && this.frame.type === "oneOf";
    };
    DataFrame.prototype.isDocument = function() {
      return this.frame && this.frame.type === "document";
    };
    DataFrame.prototype.isObject = function() {
      return this.isObjectProperty() && this.frame && !(this.isChoice() || this.isDocument());
    };
    DataFrame.prototype.isProperty = function() {
      return false;
    };
    DataFrame.prototype.getTypeShorthand = function() {
      if (this.isDocument())
        return "document";
      if (this.isChoice())
        return "choice";
      const sh = FrameHelper.getShorthand(this.getType());
      return sh || this.getType();
    };
    DataFrame.prototype.get = function() {
      if (this.contents) {
        return this.contents;
      }
      if (this.isDatatypeProperty() && this.rangeValue && typeof this.rangeValue["@value"] !== "undefined") {
        return this.rangeValue["@value"];
      }
      if (this.isChoice() || this.isDocument()) {
        return this.rangeValue && this.rangeValue["@id"] ? this.rangeValue["@id"] : "";
      }
      return "";
    };
    DataFrame.prototype.set = function(value, normalizer) {
      if (normalizer)
        value = normalizer(value, this);
      this.contents = value;
      if (this.isChoice() || this.isDocument()) {
        this.frame.domainValue = value;
      }
      if (this.isDatatypeProperty()) {
        if (!this.rangeValue)
          this.rangeValue = { "@type": this.range };
        this.rangeValue["@value"] = value;
      }
    };
    DataFrame.prototype.clear = function() {
      if (this.isDocument() || this.isChoice() || this.isDatatypeProperty()) {
        this.set("");
      }
    };
    DataFrame.prototype.cardControlAllows = function(action) {
      if (this.parent.cframe.hasRestriction()) {
        const rest = this.parent.cframe.restriction;
        const currentnum = this.parent.values.length;
        if (action === "add" || action === "clone") {
          if (rest.max && currentnum >= rest.max) {
            return false;
          }
        }
        if (action === "delete" && (rest.min && currentnum <= rest.min)) {
          return false;
        }
      }
      return true;
    };
    DataFrame.prototype.extract = function() {
      const val = this.get();
      if (val !== "" && val !== false) {
        const objlit = {};
        if (!this.isDocument()) {
          objlit["@type"] = this.getType();
        }
        if (this.isChoice() || this.isDocument()) {
          objlit["@id"] = val;
        } else {
          objlit["@value"] = val;
        }
        if (this.language) {
          objlit["@language"] = this.language;
        }
        return objlit;
      }
      return val;
    };
    DataFrame.prototype.saveQuery = function(newval, ntype, nlang) {
      let upd;
      if (newval === "")
        return false;
      if (this.isDocument() || this.isChoice()) {
        upd = WOQL2.iri(newval);
      } else {
        upd = { "@value": newval };
        upd["@type"] = ntype || this.range;
        if (nlang || this.language)
          upd["@language"] = nlang || this.language;
      }
      let q;
      if (this.get() !== "") {
        q = WOQL2.update_triple(this.parent.subject(), this.predicate, upd, this.extract());
      } else {
        q = WOQL2.add_triple(this.parent.subject(), this.predicate, upd);
      }
      if (this.parent.parent) {
        this.parent.parent.pathToDoc(q);
      }
      return q;
    };
    DataFrame.prototype.deleteQuery = function() {
      const q = WOQL2.delete_triple(this.subject(), this.predicate, this.extract());
      return q;
    };
    function ClassFrame(frame, parent, label) {
      this.err = false;
      this.parent = parent;
      this.subjid = parent ? parent.subjid : false;
      if (frame) {
        if (frame.label) {
          const dl = label || "";
          if (frame.label["@value"])
            this.label = frame.label["@value"];
          else if (frame.label)
            this.label = frame.label;
          else
            this.label = dl;
        }
        this.load(frame);
      }
    }
    ClassFrame.prototype = DataFrame.prototype;
    ClassFrame.prototype.load = function(frame) {
      if (typeof frame !== "object") {
        this.error("No frame passed to load");
        return;
      }
      this.domainValue = frame.domainValue;
      this.subjid = frame.domainValue ? frame.domainValue : false;
      this.type = frame.type;
      this.domain = frame.domain;
      this.predicate = frame.property;
      this.frame = frame.frame;
      this.comment = frame.comment ? frame.comment["@value"] : "";
      this.range = frame.range;
      this.rangeValue = frame.rangeValue;
    };
    ClassFrame.prototype.loadFromJSONLD = function(jsonld, prop) {
      if (jsonld[prop]) {
        this.predicate = FrameHelper.unshorten(prop);
        this.type = jsonld[prop]["@value"] ? "datatypeProperty" : "objectProperty";
        this.range = FrameHelper.unshorten(jsonld[prop]["@type"]);
        this.rangeValue = jsonld[prop]["@id"] ? jsonld[prop]["@id"] : jsonld[prop];
      }
      this.domain = FrameHelper.unshorten(jsonld["@type"]);
      this.domainValue = FrameHelper.unshorten(jsonld["@id"]);
      this.label = FrameHelper.labelFromURL(this.predicate);
      this.comment = "";
    };
    ClassFrame.prototype.isClassChoice = function() {
      return this.frame && this.frame.type === "class_choice";
    };
    ClassFrame.prototype.loadFromObjectFrame = function(par, child) {
      this.type = "objectProperty";
      this.predicate = par.property;
      this.range = par.range;
      this.rangeValue = child.subjid;
      this.domain = par.cls;
      this.domainValue = par.subjid;
      this.label = par.getLabel();
      this.comment = par.getComment();
      this.restriction = par.restriction;
    };
    ClassFrame.prototype.createEmptyChoice = function(cls, newid) {
      const cf = this.getChosenClassFrames(cls);
      const objframe = new ObjectFrame(cls);
      if (newid)
        objframe.subjid = newid;
      objframe.loadClassFrames(cf);
      const fframe = objframe.fillFromSchema();
      fframe.status = "new";
      return fframe;
    };
    ClassFrame.prototype.getChosenClassFrames = function(chosen) {
      let nc = [];
      for (let i = 0; i < this.frame.operands.length; i += 1) {
        for (let j = 0; j < this.frame.operands[i].length; j += 1) {
          if (chosen === this.frame.operands[i][j].domain) {
            nc = nc.concat(this.frame.operands[i][j]);
          }
        }
      }
      return nc;
    };
    ClassFrame.prototype.createEmpty = function(newid) {
      let objframe;
      if (this.isObject()) {
        if (this.isClassChoice()) {
          objframe = new ObjectFrame(this.predicate);
        } else {
          objframe = new ObjectFrame(this.range);
        }
        if (newid)
          objframe.subjid = newid;
        objframe.loadClassFrames(this.frame);
        const fframe = objframe.fillFromSchema();
        fframe.status = "new";
        return fframe;
      }
      if (this.isData()) {
        const dataframe = this.copy(newid);
        dataframe.set("");
        dataframe.status = "new";
        return dataframe;
      }
      return void 0;
    };
    ClassFrame.prototype.cloneDataFrame = function(newid, other) {
      return other;
    };
    ClassFrame.prototype.clone = function(newid, other) {
      if (this.isObject()) {
        console.log("cannot clone class frame");
      } else if (this.isData()) {
        return this.cloneDataFrame(newid, other);
      }
      return void 0;
    };
    ClassFrame.prototype.getClassChoices = function() {
      const choices = [];
      return choices;
    };
    ClassFrame.prototype.getChosenClassFrame = function(chosen, parent) {
      for (let i = 0; i < this.frame.operands.length; i += 1) {
        const operand = this.frame.operands[i];
        if (operand.class && chosen === operand.class) {
          const cf = new ClassFrame(operand, parent, this.label);
          return cf;
        }
      }
      return false;
    };
    ClassFrame.prototype.getChosenFrame = function(dataframe) {
      return dataframe;
    };
    ClassFrame.prototype.isData = function() {
      return this.isDocument() || this.isChoice() || this.isDatatypeProperty();
    };
    function Restriction(restriction) {
      this.min = 0;
      this.max = 0;
      if (restriction)
        this.loadRestriction(restriction);
    }
    Restriction.prototype.hasCardRestriction = function() {
      if (this.min === 0 && this.max === 0)
        return false;
      return true;
    };
    Restriction.prototype.loadRestriction = function(restriction) {
      if (typeof restriction.cardinality !== "undefined") {
        this.max = restriction.cardinality;
        this.min = restriction.cardinality;
      } else if (typeof restriction.maxCardinality !== "undefined") {
        this.max = restriction.maxCardinality;
      } else if (typeof restriction.minCardinality !== "undefined") {
        this.min = restriction.minCardinality;
      } else if (typeof restriction.type !== "undefined" && restriction.type === "and" && typeof restriction.operands === "object") {
        for (let i = 0; i < restriction.operands.length; i += 1) {
          const nrest = new Restriction(restriction.operands[i]);
          if (this.max === 0 && nrest.max > 0)
            this.max = nrest.max;
          else if (this.max > 0 && nrest.max > 0 && nrest.max < this.max)
            this.max = nrest.max;
          if (this.min === 0 && nrest.min > 0)
            this.min = nrest.min;
          else if (this.min > 0 && nrest.min > 0 && nrest.min > this.min)
            this.min = nrest.min;
        }
      }
    };
    module2.exports = ObjectFrame;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/documentFrame.js
var require_documentFrame = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/documentFrame.js"(exports2, module2) {
    var ObjectFrame = require_objectFrame();
    var FrameConfig = require_frameConfig();
    var FrameHelper = require_utils();
    function DocumentFrame(client, config) {
      this.client = client;
      this.config = config || new FrameConfig();
      this.load_schema = false;
    }
    DocumentFrame.prototype.options = function(opts2) {
      this.config = opts2;
      return this;
    };
    DocumentFrame.prototype.db = function(dburl) {
      this.client.db(dburl);
      return this;
    };
    DocumentFrame.prototype.loadDocument = function(url, encoding) {
      encoding = encoding || "system:frame";
      return this.client.getDocument(url, { "system:encoding": encoding }).then((response) => {
        encoding === "system:frame" ? this.loadDataFrames(response) : this.loadJSON(response);
      });
    };
    DocumentFrame.prototype.loadSchema = function(cls, dbURL) {
      const ncls = FrameHelper.unshorten(cls);
      return this.client.getClassFrame(dbURL, ncls).then((response) => this.loadSchemaFrames(response, ncls));
    };
    DocumentFrame.prototype.loadComplete = function(url, cls) {
      if (cls) {
        return Promise.all([this.loadDocument(url), this.loadDocumentSchema(cls)]);
      }
      return this.loadDocument(url).then(() => {
        this.loadSchema(this.document.cls);
      });
    };
    DocumentFrame.prototype.loadJSON = function(json2, type) {
      if (this.docid) {
        return this.loadDocument(this.docid);
      }
      if (this.clsid) {
        return this.loadDocumentSchema(this.clsid);
      }
      console.error("Either docid or clid must be set before load is called");
    };
    DocumentFrame.prototype.loadData = function(jsonld, cls, classframes) {
      if (!cls) {
        if (this.document)
          cls = this.document.cls;
        else if (jsonld && jsonld["@type"]) {
          cls = jsonld["@type"];
        }
      }
      if (cls) {
        if (!this.document) {
          this.document = new ObjectFrame(cls, jsonld, classframes);
        } else {
          this.document.loadJSONLDDocument(jsonld);
        }
      } else {
        console.log("Missing Class Failed to add dataframes due to missing class");
      }
    };
    DocumentFrame.prototype.load = function(classframes, doc) {
      this.document = new ObjectFrame(doc["@type"], doc, classframes);
    };
    DocumentFrame.prototype.loadSchemaFrames = function(classframes, cls) {
      if (!cls) {
        if (classframes && classframes.length && classframes[0] && classframes[0].domain) {
          cls = classframes[0].domain;
        }
      }
      if (cls) {
        if (!this.document) {
          this.document = new ObjectFrame(cls);
        }
        if (classframes) {
          this.document.loadClassFrames(classframes);
          if (!this.document.subjid) {
            this.document.newDoc = true;
            this.document.fillFromSchema(FrameHelper.genBNID(`${FrameHelper.urlFragment(cls)}_`));
          }
        }
      } else {
        console.log("Missing Class", "Failed to add class frames due to missing both class and classframes");
      }
    };
    DocumentFrame.prototype.filterFrame = function(loadRenderer) {
      const myfilt = function(frame, rule) {
        if (typeof rule.render() !== "undefined") {
          frame.render = rule.render();
        }
        if (rule.compare()) {
          frame.compare = rule.compare();
        }
        if (rule.errors()) {
          frame.errors = frame.errors ? frame.errors.concat(rule.errors()) : rule.errors();
        } else if (rule.errors() === false)
          delete frame.errors;
      };
      this.applyRules(false, false, myfilt);
    };
    DocumentFrame.prototype.setErrors = function(errors, frameconf) {
      this.clearErrors(frameconf);
      for (let i = 0; i < errors.length; i++) {
        addRuleForVio(frameconf, errors[i]);
      }
      const myfilt = function(frame, rule) {
        if (rule.errors()) {
          frame.errors = frame.errors ? frame.errors.concat(rule.errors()) : rule.errors();
        }
      };
      this.applyRules(false, frameconf, myfilt);
    };
    DocumentFrame.prototype.clearErrors = function(frameconf) {
      frameconf.all();
      const myfilt = function(frame, rule) {
        if (frame.errors)
          delete frame.errors;
      };
      this.applyRules(false, frameconf, myfilt);
      frameconf.rules = [];
    };
    function addRuleForVio(docview, error) {
      const prop = error["vio:property"] ? error["vio:property"]["@value"] : false;
      const subj = error["vio:subject"] ? error["vio:subject"]["@value"] : false;
      const msg = error["vio:message"] ? error["vio:message"]["@value"] : false;
      let val = error["api:value"] ? error["api:value"] : false;
      if (val && val[0] === '"' && val[val.length - 1] === '"')
        val = val.substring(1, val.length - 1);
      const type = error["api:type"] ? error["api:type"] : false;
      if (type && val) {
        docview.data().value(val).type(type).errors([error]);
      }
      if (prop && subj) {
        const shrt = FrameHelper.shorten(subj);
        if (shrt.substring(0, 5) === "woql:")
          shrt === shrt.substring(5);
        docview.data().property(prop).value(shrt, subj).errors([error]);
      }
    }
    DocumentFrame.prototype.applyRules = function(doc, config, mymatch) {
      doc = doc || this.document;
      if (!doc)
        return;
      config = config || this.config;
      const onmatch = function(frame, rule) {
        config.setFrameDisplayOptions(frame, rule);
        if (mymatch)
          mymatch(frame, rule);
      };
      doc.mfilter(config.rules, onmatch);
    };
    module2.exports = DocumentFrame;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/frameConfig.js
var require_frameConfig = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/frameConfig.js"(exports2, module2) {
    var Config = require_viewConfig();
    var { FrameRule } = require_frameRule();
    var DocumentFrame = require_documentFrame();
    function FrameConfig() {
      Config.ViewConfig.call(this);
      this.type = "document";
    }
    Object.setPrototypeOf(FrameConfig.prototype, Config.ViewConfig.prototype);
    FrameConfig.prototype.create = function(client) {
      const tf = new DocumentFrame(client, this);
      return tf;
    };
    FrameConfig.prototype.prettyPrint = function() {
      let str2 = "view = View.document();\n";
      str2 += this.getBasicPrettyPrint();
      if (typeof this.load_schema() !== "undefined") {
        str2 += `view.load_schema(${this.load_schema()})
`;
      }
      for (let i = 0; i < this.rules.length; i++) {
        str2 += `view.${this.rules[i].prettyPrint()}
`;
      }
      return str2;
    };
    FrameConfig.prototype.json = function() {
      const conf = this.getBasicJSON();
      if (typeof this.load_schema() !== "undefined") {
        conf.load_schema = this.load_schema();
      }
      const mj = { frame: conf, rules: this.getRulesJSON() };
      return mj;
    };
    FrameConfig.prototype.loadJSON = function(config, rules) {
      const jr = [];
      for (let i = 0; i < rules.length; i++) {
        const nr = new DocumentRule();
        nr.json(rules[i]);
        jr.push(nr);
      }
      this.rules = jr;
      this.loadBasicJSON(config);
      if (typeof config.load_schema !== "undefined") {
        this.load_schema(config.load_schema);
      }
      return this;
    };
    FrameConfig.prototype.json_rules = function() {
      const jr = [];
      for (let i = 0; i < this.rules.length; i++) {
        jr.push(this.rules[i].json());
      }
      return jr;
    };
    FrameConfig.prototype.load_schema = function(tf) {
      if (typeof tf === "undefined")
        return this.get_schema;
      this.get_schema = tf;
      return this;
    };
    FrameConfig.prototype.show_all = function(r) {
      this.all().renderer(r);
      return this;
    };
    FrameConfig.prototype.show_parts = function(o, p, d) {
      this.object().renderer(o);
      this.property().renderer(p);
      this.data().renderer(d);
      return this;
    };
    FrameConfig.prototype.object = function() {
      const fp = new DocumentRule().scope("object");
      this.rules.push(fp);
      return fp;
    };
    FrameConfig.prototype.property = function() {
      const fp = new DocumentRule().scope("property");
      this.rules.push(fp);
      return fp;
    };
    FrameConfig.prototype.scope = function(scope) {
      const fp = new DocumentRule().scope(scope);
      this.rules.push(fp);
      return fp;
    };
    FrameConfig.prototype.data = function() {
      const fp = new DocumentRule().scope("data");
      this.rules.push(fp);
      return fp;
    };
    FrameConfig.prototype.all = function() {
      const fp = new DocumentRule().scope("*");
      this.rules.push(fp);
      return fp;
    };
    FrameConfig.prototype.setFrameDisplayOptions = function(frame, rule) {
      if (typeof frame.display_options === "undefined")
        frame.display_options = {};
      if (typeof rule.mode() !== "undefined") {
        frame.display_options.mode = rule.mode();
      }
      if (typeof rule.view() !== "undefined")
        frame.display_options.view = rule.view();
      if (typeof rule.showDisabledButtons() !== "undefined")
        frame.display_options.show_disabled_buttons = rule.showDisabledButtons();
      if (typeof rule.hidden() !== "undefined")
        frame.display_options.hidden = rule.hidden();
      if (typeof rule.collapse() !== "undefined")
        frame.display_options.collapse = rule.collapse();
      if (typeof rule.style() !== "undefined")
        frame.display_options.style = rule.style();
      if (typeof rule.headerStyle() !== "undefined")
        frame.display_options.header_style = rule.headerStyle();
      if (typeof rule.features() !== "undefined") {
        frame.display_options.features = this.setFrameFeatures(frame.display_options.features, rule.features());
      }
      if (typeof rule.headerFeatures() !== "undefined")
        frame.display_options.header_features = this.setFrameFeatures(frame.display_options.header_features, rule.headerFeatures());
      if (typeof rule.header() !== "undefined")
        frame.display_options.header = rule.header();
      if (typeof rule.showEmpty() !== "undefined")
        frame.display_options.show_empty = rule.showEmpty();
      if (typeof rule.dataviewer() !== "undefined")
        frame.display_options.dataviewer = rule.dataviewer();
      if (typeof rule.args() !== "undefined")
        frame.display_options.args = this.setFrameArgs(frame.display_options.args, rule.args());
    };
    FrameConfig.prototype.setFrameFeatures = function(existing, fresh) {
      if (!existing || !existing.length)
        return fresh;
      if (!fresh || !fresh.length)
        return existing;
      const got = [];
      for (let i = 0; i < existing.length; i++) {
        const key = typeof existing[i] === "string" ? existing[i] : Object.keys(existing[i])[0];
        got.push(key);
      }
      for (let j = 0; j < fresh.length; j++) {
        const fkey = typeof fresh[j] === "string" ? fresh[j] : Object.keys(fresh[j])[0];
        const rep = got.indexOf(fkey);
        if (rep === -1)
          existing.push(fresh[j]);
        else if (typeof fresh[j] === "object") {
          const val = existing[rep];
          if (typeof val === "string")
            existing[rep] = fresh[j];
          else if (typeof val === "object") {
            const props = fresh[j][fkey];
            for (const p in props) {
              existing[rep][fkey][p] = props[p];
            }
          }
        }
      }
      return existing;
    };
    FrameConfig.prototype.setFrameArgs = function(existing, fresh) {
      if (!existing)
        return fresh;
      if (!fresh)
        return existing;
      for (const k in fresh) {
        existing[k] = fresh[k];
      }
      return existing;
    };
    function DocumentRule() {
      FrameRule.call(this);
      this.rule = {};
    }
    Object.setPrototypeOf(DocumentRule.prototype, FrameRule.prototype);
    DocumentRule.prototype.renderer = function(rend) {
      if (typeof rend === "undefined") {
        return this.rule.renderer;
      }
      this.rule.renderer = rend;
      return this;
    };
    DocumentRule.prototype.compare = function(func) {
      if (typeof func === "undefined") {
        return this.rule.compare;
      }
      this.rule.compare = func;
      return this;
    };
    DocumentRule.prototype.mode = function(mode) {
      if (typeof mode === "undefined") {
        return this.rule.mode;
      }
      this.rule.mode = mode;
      return this;
    };
    DocumentRule.prototype.collapse = function(func) {
      if (typeof func === "undefined") {
        return this.rule.collapse;
      }
      this.rule.collapse = func;
      return this;
    };
    DocumentRule.prototype.view = function(m) {
      if (!m)
        return this.rule.view;
      this.rule.view = m;
      return this;
    };
    DocumentRule.prototype.showDisabledButtons = function(m) {
      if (typeof m === "undefined")
        return this.rule.show_disabled_buttons;
      this.rule.show_disabled_buttons = m;
      return this;
    };
    DocumentRule.prototype.header = function(m) {
      if (!m)
        return this.rule.header;
      this.rule.header = m;
      return this;
    };
    DocumentRule.prototype.errors = function(errs) {
      if (!errs)
        return this.rule.errors;
      this.rule.errors = errs;
      return this;
    };
    DocumentRule.prototype.headerStyle = function(m) {
      if (!m)
        return this.rule.headerStyle;
      this.rule.headerStyle = m;
      return this;
    };
    DocumentRule.prototype.showEmpty = function(m) {
      if (!m)
        return this.rule.show_empty;
      this.rule.show_empty = m;
      return this;
    };
    DocumentRule.prototype.dataviewer = function(m) {
      if (!m)
        return this.rule.dataviewer;
      this.rule.dataviewer = m;
      return this;
    };
    DocumentRule.prototype.features = function(...m) {
      if (typeof m === "undefined" || m.length === 0)
        return this.rule.features;
      this.rule.features = m;
      return this;
    };
    DocumentRule.prototype.headerFeatures = function(...m) {
      if (typeof m === "undefined" || m.length === 0)
        return this.rule.header_features;
      this.rule.header_features = m;
      return this;
    };
    DocumentRule.prototype.render = function(func) {
      if (!func)
        return this.rule.render;
      const hf = this.headerFeatures();
      const f = this.features();
      if (hf && hf.length) {
        var feats = this.applyFeatureProperty(hf, "render", func);
        this.headerFeatures(...feats);
      } else if (f && f.length) {
        var feats = this.applyFeatureProperty(f, "render", func);
        this.features(...feats);
      } else {
        this.rule.render = func;
      }
      return this;
    };
    DocumentRule.prototype.style = function(style) {
      if (typeof style === "undefined")
        return this.rule.style;
      const hf = this.headerFeatures();
      const f = this.features();
      if (hf && hf.length) {
        var feats = this.applyFeatureProperty(hf, "style", style);
        this.headerFeatures(...feats);
      } else if (f && f.length) {
        var feats = this.applyFeatureProperty(f, "style", style);
        this.features(...feats);
      } else {
        this.rule.style = style;
      }
      return this;
    };
    DocumentRule.prototype.hidden = function(m) {
      if (typeof m === "undefined")
        return this.rule.hidden;
      const hf = this.headerFeatures();
      const f = this.features();
      if (hf && hf.length) {
        var feats = this.applyFeatureProperty(hf, "hidden", m);
        this.headerFeatures(...feats);
      } else if (f && f.length) {
        var feats = this.applyFeatureProperty(f, "hidden", m);
        this.features(...feats);
      } else {
        this.rule.hidden = m;
      }
      return this;
    };
    DocumentRule.prototype.args = function(json2) {
      if (!json2)
        return this.rule.args;
      const hf = this.headerFeatures();
      const f = this.features();
      if (hf && hf.length) {
        var feats = this.applyFeatureProperty(hf, "args", json2);
        this.headerFeatures(...feats);
      } else if (f && f.length) {
        var feats = this.applyFeatureProperty(f, "args", json2);
        this.features(...feats);
      } else {
        this.rule.args = json2;
      }
      return this;
    };
    DocumentRule.prototype.applyFeatureProperty = function(feats, prop, val) {
      const nfeats = [];
      for (let i = 0; i < feats.length; i++) {
        if (typeof feats[i] === "string") {
          var nfeat = {};
          nfeat[feats[i]] = {};
          nfeat[feats[i]][prop] = val;
          nfeats.push(nfeat);
        } else if (typeof feats[i] === "object") {
          const fkey = Object.keys(feats[i])[0];
          if (fkey) {
            var nfeat = feats[i];
            nfeat[fkey][prop] = val;
            nfeats.push(nfeat);
          }
        }
      }
      return nfeats;
    };
    DocumentRule.prototype.unpackFeatures = function(feats) {
      const extensions = {};
      let fstr = "";
      for (let i = 0; i < feats.length; i++) {
        if (typeof feats[i] === "string") {
          fstr += `"${feats[i]}"`;
        } else if (typeof feats[i] === "object") {
          const fid = Object.keys(feats[i])[0];
          fstr += `"${fid}"`;
          for (const prop in feats[i][fid]) {
            extensions[prop] = feats[i][fid][prop];
          }
        }
        if (i < feats.length - 1) {
          fstr += ", ";
        }
      }
      for (let k = 0; k < Object.keys(extensions).length; k++) {
        const ext = Object.keys(extensions)[k];
        const val = extensions[ext];
        fstr += `).${ext}(`;
        if (typeof val === "function") {
          fstr += val;
        } else if (typeof val === "string") {
          fstr += `"${val}"`;
        } else if (typeof val === "object") {
          fstr += JSON.stringify(val);
        }
      }
      return fstr;
    };
    DocumentRule.prototype.prettyPrint = function() {
      if (this.pattern) {
        str = this.pattern.prettyPrint();
      }
      if (typeof this.renderer() !== "undefined") {
        str += `.renderer('${this.renderer()}')`;
      }
      if (typeof this.render() !== "undefined") {
        str += `.render(${this.render})`;
      }
      if (typeof this.compare() !== "undefined") {
        str += `.compare(${this.compare()})`;
      }
      if (typeof this.mode() !== "undefined") {
        str += `.mode('${this.mode()}')`;
      }
      if (typeof this.collapse() !== "undefined") {
        str += `.collapse(${this.collapse()})`;
      }
      if (typeof this.hidden() !== "undefined") {
        str += `.hidden(${this.hidden()})`;
      }
      if (typeof this.view() !== "undefined") {
        str += `.view('${this.view()}')`;
      }
      if (typeof this.showDisabledButtons() !== "undefined") {
        str += `.showDisabledButtons(${this.showDisabledButtons()})`;
      }
      if (typeof this.header() !== "undefined") {
        str += `.header(${this.header()})`;
      }
      if (typeof this.style() !== "undefined") {
        str += `.style("${this.style()}")`;
      }
      if (typeof this.headerStyle() !== "undefined") {
        str += `.headerStyle("${this.headerStyle()}")`;
      }
      if (typeof this.args() !== "undefined") {
        str += `.args(${JSON.stringify(this.args())})`;
      }
      if (typeof this.errors() !== "undefined") {
        str += `.errors(${JSON.stringify(this.errors())})`;
      }
      if (typeof this.showEmpty() !== "undefined") {
        str += `.showEmpty(${this.show_empty()})`;
      }
      if (typeof this.dataviewer() !== "undefined") {
        str += `.dataviewer("${this.dataviewer()}")`;
      }
      if (typeof this.features() !== "undefined") {
        str += `.features(${this.unpackFeatures(this.features())})`;
      }
      if (typeof this.headerFeatures() !== "undefined") {
        str += `.headerFeatures(${this.unpackFeatures(this.headerFeatures())})`;
      }
      return str;
    };
    module2.exports = FrameConfig;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlView.js
var require_woqlView = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/viewer/woqlView.js"(exports2, module2) {
    var WOQLTableConfig = require_tableConfig();
    var WOQLChooserConfig = require_chooserConfig();
    var WOQLGraphConfig = require_graphConfig();
    var WOQLChartConfig = require_chartConfig();
    var WOQLStreamConfig = require_streamConfig();
    var FrameConfig = require_frameConfig();
    var { WOQLRule } = require_woqlRule();
    var { FrameRule } = require_frameRule();
    var View = {};
    View.table = function() {
      return new WOQLTableConfig();
    };
    View.chart = function() {
      return new WOQLChartConfig();
    };
    View.graph = function() {
      return new WOQLGraphConfig();
    };
    View.chooser = function() {
      return new WOQLChooserConfig();
    };
    View.stream = function() {
      return new WOQLStreamConfig();
    };
    View.document = function() {
      return new FrameConfig();
    };
    View.loadConfig = function(config) {
      let view;
      if (config.table) {
        view = new WOQLTableConfig();
        view.loadJSON(config.table, config.rules);
      } else if (config.chooser) {
        view = new WOQLChooserConfig();
        view.loadJSON(config.chooser, config.rules);
      } else if (config.graph) {
        view = new WOQLGraphConfig();
        view.loadJSON(config.graph, config.rules);
      } else if (config.chart) {
        view = new WOQLChartConfig();
        view.loadJSON(config.chart, config.rules);
      } else if (config.stream) {
        view = new WOQLStreamConfig();
        view.loadJSON(config.stream, config.rules);
      } else if (config.frame) {
        view = new FrameConfig();
        view.loadJSON(config.frame, config.rules);
      }
      return view;
    };
    View.rule = function(type) {
      if (type && type === "frame")
        return new FrameRule();
      return new WOQLRule();
    };
    View.pattern = function(type) {
      if (type && type === "woql")
        return new WOQLRule().pattern;
      return new FrameRule().pattern;
    };
    View.matchRow = function(rules, row2, rownum, action) {
      return new WOQLRule().matchRow(rules, row2, rownum, action);
    };
    View.matchColumn = function(rules, key, action) {
      return new WOQLRule().matchColumn(rules, key, action);
    };
    View.matchCell = function(rules, row2, key, rownum, action) {
      return new WOQLRule().matchCell(rules, row2, key, rownum, action);
    };
    View.matchNode = function(rules, row2, key, nid, action) {
      return new WOQLRule().matchNode(rules, row2, key, nid, action);
    };
    View.matchEdge = function(rules, row2, keya, keyb, action) {
      return new WOQLRule().matchPair(rules, row2, keya, keyb, action);
    };
    View.matchFrame = function(rules, frame, onmatch) {
      return new FrameRule().testRules(rules, frame, onmatch);
    };
    module2.exports = View;
  }
});

// node_modules/@terminusdb/terminusdb-client/lib/accessControl.js
var require_accessControl = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/lib/accessControl.js"(exports2, module2) {
    var DispatchRequest = require_dispatchRequest();
    var ErrorMessage = require_errorMessage();
    var CONST = require_const();
    var UTILS2 = require_utils();
    var typedef2 = require_typedef();
    function AccessControl(cloudAPIUrl, params) {
      this.baseURL = this.getAPIUrl(cloudAPIUrl);
      if (!params)
        return;
      if (params.jwt) {
        this.setJwtToken(params.jwt);
      } else if (params.token) {
        this.setApiToken(params.token);
      } else if (params.key) {
        this.setApiKey(params.key);
        this.user = params.user;
      }
      this.defaultOrganization = this.getDefaultOrganization(params);
    }
    AccessControl.prototype.getDefaultOrganization = function(params) {
      if (params && params.organization && typeof params.organization === "string") {
        return params.organization;
      }
      return void 0;
    };
    AccessControl.prototype.setJwtToken = function(jwt) {
      if (!jwt) {
        throw new Error("TerminusX Access token required");
      }
      this.apiKey = jwt;
      this.apiType = "jwt";
    };
    AccessControl.prototype.setApiToken = function(token) {
      if (!token) {
        throw new Error("TerminusX Access token required");
      }
      this.apiKey = token;
      this.apiType = "apikey";
    };
    AccessControl.prototype.setApiKey = function(key) {
      if (!key) {
        throw new Error("TerminusDB bacis authentication key required");
      }
      this.apiKey = key;
      this.apiType = "basic";
    };
    AccessControl.prototype.getAPIUrl = function(cloudAPIUrl) {
      if (!cloudAPIUrl || typeof cloudAPIUrl !== "string") {
        throw new Error("TerminusX api url required!");
      }
      if (cloudAPIUrl.lastIndexOf("/") !== cloudAPIUrl.length - 1) {
        cloudAPIUrl += "/";
      }
      return `${cloudAPIUrl}api`;
    };
    AccessControl.prototype.dispatch = function(requestUrl, action, payload) {
      if (!requestUrl) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              action,
              "Invalid request URL"
            )
          )
        );
      }
      return DispatchRequest(
        requestUrl,
        action,
        payload,
        { type: this.apiType, key: this.apiKey, user: this.user },
        null,
        this.customHeaders()
      );
    };
    AccessControl.prototype.customHeaders = function(customHeaders) {
      if (customHeaders)
        this._customHeaders = customHeaders;
      else
        return this._customHeaders;
    };
    AccessControl.prototype.getOrganization = function(org) {
      return this.dispatch(`${this.baseURL}/organizations/${org}`, CONST.GET);
    };
    AccessControl.prototype.getAllOrganizations = function() {
      return this.dispatch(`${this.baseURL}/organizations`, CONST.GET);
    };
    AccessControl.prototype.createOrganization = function(orgName) {
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(orgName)}`, CONST.POST, {});
    };
    AccessControl.prototype.deleteOrganization = function(orgName) {
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(orgName)}`, CONST.DELETE);
    };
    AccessControl.prototype.createRole = function(name, actions) {
      const payload = { name, action: actions };
      return this.dispatch(`${this.baseURL}/roles`, CONST.POST, payload);
    };
    AccessControl.prototype.deleteRole = function(name) {
      return this.dispatch(`${this.baseURL}/roles/${UTILS2.encodeURISegment(name)}`, CONST.DELETE);
    };
    AccessControl.prototype.getAllUsers = function() {
      return this.dispatch(`${this.baseURL}/users`, CONST.GET);
    };
    AccessControl.prototype.createUser = function(name, password) {
      const payload = { name, password };
      return this.dispatch(`${this.baseURL}/users`, CONST.POST, payload);
    };
    AccessControl.prototype.deleteUser = function(userId) {
      return this.dispatch(`${this.baseURL}/users/${UTILS2.encodeURISegment(userId)}`, CONST.DELETE);
    };
    AccessControl.prototype.manageCapability = function(userName, resourceName, rolesArr, operation, scopeType) {
      const payload = {
        operation,
        user: userName,
        roles: rolesArr,
        scope: resourceName,
        scope_type: scopeType
      };
      return this.dispatch(`${this.baseURL}/capabilities`, CONST.POST, payload);
    };
    AccessControl.prototype.getAccessRoles = function() {
      return this.dispatch(`${this.baseURL}/roles`, CONST.GET);
    };
    AccessControl.prototype.getOrgUsers = function(orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users`, CONST.GET);
    };
    AccessControl.prototype.getTeamUserRoles = function(userName, orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users/${UTILS2.encodeURISegment(userName)}`, CONST.GET);
    };
    AccessControl.prototype.ifOrganizationExists = function(orgName) {
      if (!orgName) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "HEAD",
              "Please provide a organization name"
            )
          )
        );
      }
      return this.dispatch(`${this.baseURL}/private/organizations/${UTILS2.encodeURISegment(orgName)}`, CONST.HEAD);
    };
    AccessControl.prototype.createOrganizationRemote = function(orgName) {
      const payload = { organization: orgName };
      return this.dispatch(`${this.baseURL}/private/organizations`, CONST.POST, payload);
    };
    AccessControl.prototype.getPendingOrgInvites = function(orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/invites`, CONST.GET);
    };
    AccessControl.prototype.sendOrgInvite = function(userEmail, role, note = "", orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!userEmail) {
        errorMessage = "Please provide a user email";
      } else if (!role) {
        errorMessage = "Please provide a role";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/invites`, CONST.POST, {
        email_to: userEmail,
        role,
        note
      });
    };
    AccessControl.prototype.getOrgInvite = function(inviteId, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!inviteId) {
        errorMessage = "Please provide a invite id";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const inviteHash = UTILS2.removeDocType(inviteId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/invites/${inviteHash}`, CONST.GET);
    };
    AccessControl.prototype.deleteOrgInvite = function(inviteId, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!inviteId) {
        errorMessage = "Please provide a invite id";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const inviteHash = UTILS2.removeDocType(inviteId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/invites/${inviteHash}`, CONST.DELETE);
    };
    AccessControl.prototype.updateOrgInviteStatus = function(inviteId, accepted, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!inviteId) {
        errorMessage = "Please provide a invite id";
      } else if (typeof accepted === "undefined") {
        errorMessage = "Please provide a accepted status";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "PUT",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const inviteHash = UTILS2.removeDocType(inviteId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/invites/${inviteHash}`, CONST.PUT, {
        accepted
      });
    };
    AccessControl.prototype.getTeamUserRole = function(orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/role`, CONST.GET);
    };
    AccessControl.prototype.removeUserFromOrg = function(userId, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!userId) {
        errorMessage = "Please provide a userId";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "DELETE",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const user = UTILS2.removeDocType(userId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users/${user}`, CONST.DELETE);
    };
    AccessControl.prototype.getDatabaseRolesOfUser = function(userId, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!userId) {
        errorMessage = "Please provide a user id";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const user = UTILS2.removeDocType(userId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users/${user}/databases`, CONST.GET);
    };
    AccessControl.prototype.createUserRole = function(userId, scope, role, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!userId) {
        errorMessage = "Please provide a user id";
      } else if (!scope) {
        errorMessage = "Please provide a scope";
      } else if (!role) {
        errorMessage = "Please provide a role";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const user = UTILS2.removeDocType(userId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users/${user}/capabilities`, CONST.POST, {
        scope,
        role
      });
    };
    AccessControl.prototype.updateUserRole = function(userId, capabilityId, scope, role, orgName) {
      let errorMessage;
      if (!orgName && !this.defaultOrganization) {
        errorMessage = "Please provide a organization name";
      } else if (!userId) {
        errorMessage = "Please provide a user id";
      } else if (!capabilityId) {
        errorMessage = "Please provide a capabilityId";
      } else if (!scope) {
        errorMessage = "Please provide a scope";
      } else if (!role) {
        errorMessage = "Please provide a role";
      }
      if (errorMessage) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "PUT",
              errorMessage
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      const user = UTILS2.removeDocType(userId);
      const capHash = UTILS2.removeDocType(capabilityId);
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/users/${user}/capabilities/${capHash}`, CONST.PUT, {
        scope,
        role
      });
    };
    AccessControl.prototype.accessRequestsList = function(orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "GET",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/access_requests`, CONST.GET);
    };
    AccessControl.prototype.sendAccessRequest = function(email, affiliation, note, orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              "Please provide a organization name"
            )
          )
        );
      }
      const payload = { email, affiliation, note };
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/access_requests`, CONST.POST, payload);
    };
    AccessControl.prototype.deleteAccessRequest = function(acceId, orgName) {
      if (!orgName && !this.defaultOrganization) {
        return Promise.reject(
          new Error(
            ErrorMessage.getInvalidParameterMessage(
              "POST",
              "Please provide a organization name"
            )
          )
        );
      }
      const org = orgName || this.defaultOrganization;
      return this.dispatch(`${this.baseURL}/organizations/${UTILS2.encodeURISegment(org)}/access_requests/${acceId}`, CONST.DELETE);
    };
    AccessControl.prototype.getUserInfo = function(userName) {
      const userNameUrl = userName || "info";
      return this.dispatch(`${this.baseURL}/users/${UTILS2.encodeURISegment(userNameUrl)}`, CONST.GET);
    };
    module2.exports = AccessControl;
  }
});

// node_modules/@terminusdb/terminusdb-client/index.js
var require_terminusdb_client = __commonJS({
  "node_modules/@terminusdb/terminusdb-client/index.js"(exports2, module2) {
    var { Var: Var2, Vars: Vars2, Doc: Doc2 } = require_woqlDoc();
    var WOQLClient2 = require_woqlClient();
    var UTILS2 = require_utils();
    var View = require_woqlView();
    var WOQL2 = require_woql();
    var WOQLResult = require_woqlResult();
    var WOQLTable = require_woqlTable();
    var WOQLGraph = require_woqlGraph();
    var axiosInstance = require_axiosInstance();
    var AccessControl = require_accessControl();
    var WOQLQuery2 = require_woqlBuilder();
    module2.exports = {
      Var: Var2,
      Doc: Doc2,
      Vars: Vars2,
      WOQLClient: WOQLClient2,
      UTILS: UTILS2,
      View,
      WOQL: WOQL2,
      WOQLResult,
      WOQLTable,
      WOQLGraph,
      axiosInstance,
      AccessControl,
      WOQLQuery: WOQLQuery2
    };
  }
});

// parse-woql.js
var TerminusClient = require_terminusdb_client();
var inputData = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  inputData += chunk;
});
process.stdin.on("end", () => {
  try {
    const queryString = inputData.trim();
    if (!queryString) {
      console.error("Error: No input provided");
      process.exit(2);
    }
    const WOQL = TerminusClient.WOQL;
    const prelude = WOQL.emerge();
    const woqlQuery = eval(prelude + "\n" + queryString);
    if (!woqlQuery) {
      console.error("Error: Query evaluation returned null/undefined");
      process.exit(1);
    }
    const jsonLD = woqlQuery.json();
    console.log(JSON.stringify(jsonLD));
    process.exit(0);
  } catch (error) {
    console.error("Parse error:", error.message);
    if (error.stack) {
      console.error(error.stack);
    }
    process.exit(1);
  }
});
process.stdin.on("error", (error) => {
  console.error("Input error:", error.message);
  process.exit(1);
});
/*! Bundled license information:

mime-db/index.js:
  (*!
   * mime-db
   * Copyright(c) 2014 Jonathan Ong
   * Copyright(c) 2015-2022 Douglas Christopher Wilson
   * MIT Licensed
   *)

mime-types/index.js:
  (*!
   * mime-types
   * Copyright(c) 2014 Jonathan Ong
   * Copyright(c) 2015 Douglas Christopher Wilson
   * MIT Licensed
   *)

@terminusdb/terminusdb-client/lib/utils.js:
  (**
   * @file Terminus Client Utility Functions
   * @license Apache Version 2
   * Object for bunding up common Terminus Utility Functions
   *)

@terminusdb/terminusdb-client/lib/const.js:
  (**
   * @file Terminus Constants
   * @license Apache Version 2
   * Constants representing API actions
   *)

axios/dist/node/axios.cjs:
  (*! Axios v1.13.2 Copyright (c) 2025 Matt Zabriskie and contributors *)

@terminusdb/terminusdb-client/lib/dispatchRequest.js:
  (**
   * @file Dispatch Request
   * @license Apache Version 2
   * @description Functions for dispatching API requests via the axios library.
   * @param {string} url  API endpoint URL
   * @param {string} action API action
   * @param {object} payload data to be transmitted to endpoint
   * @param {typedef.CredentialObj} local_auth the local authorization object
   * @param {typedef.CredentialObj} remote_auth the remote authoriuzation object
   * @param {object} customHeaders all the custom header to add to your call
   * @param {boolean} [getDataVersion] If true the function will return object having result
   * and dataVersion.
   * @param {boolean} [compress] If true, compress the data with gzip if its size is bigger than 1024
   *)

@terminusdb/terminusdb-client/lib/connectionConfig.js:
  (**
   * @file Terminus DB connection configuration
   * @license Apache Version 2
   * @description Object representing the state of a connection to a terminus db - these are:
   * provides methods for getting and setting connection parameters
   * @constructor
   * @param {string} serverUrl - the terminusdb server url
   * @param {typedef.ParamsObj} [params] - an object with the following connection parameters:
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

@terminusdb/terminusdb-client/lib/woql.js:
  (**
   * @license Apache Version 2
   * @module WOQL
   * @constructor WOQL
   * @description The WOQL object is a wrapper around the WOQLQuery object
   * Syntactic sugar to allow writing WOQL.triple()... instead of new WOQLQuery().triple()
   * Every function matches one of the public api functions of the woql query object
   *)

@terminusdb/terminusdb-client/lib/woqlClient.js:
  (**
   * @license Apache Version 2
   * @class
   * @classdesc The core functionality of the TerminusDB javascript client is
   * defined in the WOQLClient class - in the woqlClient.js file. This class provides
   * methods which allow you to directly get and set all of the configuration and API
   * endpoints of the client. The other parts of the WOQL core - connectionConfig.js
   * and connectionCapabilities.js - are used by the client to store internal state - they
   * should never have to be accessed directly. For situations where you want to communicate
   * with a TerminusDB server API, the WOQLClient class is all you will need.
   *)

@terminusdb/terminusdb-client/lib/viewer/terminusRule.js:
  (**
   * @file Terminus Rules
   * @license Apache Version 2
   * Abstract object to support applying matching rules to result sets and documents
   * sub-classes are FrameRule and WOQLRule - this just has common functions
   *)

@terminusdb/terminusdb-client/lib/viewer/woqlRule.js:
  (**
   * @file WOQL Rules
   * @license Apache Version 2
   * WOQL Rules support pattern matching on results of WOQL Queries
   *)

@terminusdb/terminusdb-client/lib/viewer/woqlResult.js:
  (**
   * @module WOQLResult
   * @license Apache Version 2
   * Object representing the result of a WOQL Query
   * @param {object} results result JSON object as returned by WOQL query
   * @param {WOQLQuery} query the query object that generated the result
   * @param {object} [config] optional result configuration options object
   * [config.no_compress] by default all URLs are compressed where possible (v:X rather than http://.../variable/x) set to true to return non-compressed values
   * [config.context] specify the JSON-LD context to use for compressing results - by default
   * the query context will be used
   *)

@terminusdb/terminusdb-client/lib/viewer/woqlTable.js:
  (**
   * @module WOQLTable
   * @license Apache Version 2
   * @param {WOQLClient} [client] we need an client if we do a server side pagination,sorting etc...
   * @param {WOQLTableConfig} [config]
   * @returns {WOQLTable}
   *)

@terminusdb/terminusdb-client/lib/viewer/frameRule.js:
  (**
   * @file Frame Rule
   * @license Apache Version 2
   *)

@terminusdb/terminusdb-client/lib/viewer/objectFrame.js:
  (**
   * @file Javascript Terminus Document Classes
   * @license Apache Version 2
   * Helper classes for accessing documents returned by the Terminus DB API programmatically
   *
   * @example
   * let doc = new TerminusDocument(client);
   *
   * //These set the objects document property and return promises:
   *
   * doc.loadDocument(URL).then(() => console.log(this.document));
   * doc.loadComplete(URL, CLS).then(() => console.log(this.document))
   * doc.loadSchema(cls).then(() => console.log(this.document))
   *
   * //These just set the object's document property
   * doc.loadJSON(json_frames, cls) //console.log(this.document)
   * doc.loadDataFrames(json_frames, cls)
   * doc.loadClassFrames(json_frames, cls)
   * @example
   *
   * @description Represents a frame for programmatic access to object frame,
   * anywhere within a document
   * Recursive data structure where this.children contains an indexed array of object frames
   * and this.dataframes contains a property indexed array of data frames
   * Every object frame carries a reference to its classframe
   * This gives us instructions as to how to create new frames according to the schema
   * After that it's turtles all the way down.
   * @param cls - ID of the class (URL)
   * @param classframe - an array of frames representing a class
   * @param archetypes list of class frames
   * @param parent parent object
   * @returns
   *)

@terminusdb/terminusdb-client/lib/viewer/documentFrame.js:
  (**
   * @file Document Frame
   * @license Apache Version 2
   *)

@terminusdb/terminusdb-client/lib/viewer/frameConfig.js:
  (**
   * @file Frame Config
   * @license Apache Version 2
   *)
  (**
   * @file Document Rule
   * @license Apache Version 2
   *)

@terminusdb/terminusdb-client/lib/accessControl.js:
  (**
   * @license Apache Version 2
   * @module AccessControl
   * @constructor AccessControl
   * @description The AccessControl is a driver to work with
   * TerminusDB and TerminusX access control api
   * for the credential you can use the JWT token, the API token or
   * the basic authentication with username and password
   * @example
   * //connect with the API token
   * //(to request a token create an account in  https://terminusdb.com/)
   * const accessContol = new AccessControl("https://servername.com",
   * {organization:"my_team_name",
   * token:"dGVybWludXNkYjovLy9kYXRhL2tleXNfYXB........"})
   * accessControl.getOrgUsers().then(result=>{
   *      console.log(result)
   * })
   *
   * //connect with the jwt token this type of connection is only for the dashboard
   * //or for application integrate with our login workflow
   * const accessContol = new AccessControl("https://servername.com",
   * {organization:"my_team_name",
   * jwt:"eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYyeTFORUd........"})
   * accessControl.getOrgUsers().then(result=>{
   *      console.log(result)
   * })
   *
   * //if the jwt is expired you can change it with
   * accessControl.setJwtToken("eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYy
   * eTFORUd.......")
   *
   * //connect with the base authentication this type of connection is only for the local installation
   * const accessContol = new AccessControl("http://127.0.0.1:6363",
   * {organization:"my_team_name", user:"admin"
   * key:"mykey"})
   * accessControl.getOrgUsers().then(result=>{
   *     console.log(result)
   * })
   *
   *)
*/
