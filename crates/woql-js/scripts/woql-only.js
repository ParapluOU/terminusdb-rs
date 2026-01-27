/**
 * WOQL-only module for QuickJS
 *
 * This extracts only the WOQL query building functionality without
 * the HTTP client dependencies.
 */

// Import only the query-related modules
const WOQLQuery = require('@terminusdb/terminusdb-client/lib/query/woqlBuilder');
const WOQLLibrary = require('@terminusdb/terminusdb-client/lib/query/woqlLibrary');
const { Vars, Var, Doc, VarsUnique, VarUnique, SetVarsUniqueCounter } = require('@terminusdb/terminusdb-client/lib/query/woqlDoc');

// Rebuild the WOQL object without the WOQLClient dependency
const WOQL = {};

// Copy all the WOQL functions from woqlBuilder via wrapper functions
WOQL.using = function (refPath, subquery) { return new WOQLQuery().using(refPath, subquery); };
WOQL.comment = function (comment, subquery) { return new WOQLQuery().comment(comment, subquery); };
WOQL.select = function (...varNames) { return new WOQLQuery().select(...varNames); };
WOQL.distinct = function (...varNames) { return new WOQLQuery().distinct(...varNames); };
WOQL.and = function (...subqueries) { return new WOQLQuery().and(...subqueries); };
WOQL.or = function (...subqueries) { return new WOQLQuery().or(...subqueries); };
WOQL.from = function (graphRef, query) { return new WOQLQuery().from(graphRef, query); };
WOQL.into = function (graphRef, subquery) { return new WOQLQuery().into(graphRef, subquery); };
WOQL.triple = function (subject, predicate, object) { return new WOQLQuery().triple(subject, predicate, object); };
WOQL.added_triple = function (subject, predicate, object) { return new WOQLQuery().added_triple(subject, predicate, object); };
WOQL.removed_triple = function (subject, predicate, object) { return new WOQLQuery().removed_triple(subject, predicate, object); };
WOQL.quad = function (subject, predicate, object, graphRef) { return new WOQLQuery().quad(subject, predicate, object, graphRef); };
WOQL.added_quad = function (subject, predicate, object, graphRef) { return new WOQLQuery().added_quad(subject, predicate, object, graphRef); };
WOQL.removed_quad = function (subject, predicate, object, graphRef) { return new WOQLQuery().removed_quad(subject, predicate, object, graphRef); };
WOQL.sub = function (classA, classB) { return new WOQLQuery().sub(classA, classB); };
WOQL.subsumption = function (classA, classB) { return new WOQLQuery().sub(classA, classB); };
WOQL.eq = function (varName, varValue) { return new WOQLQuery().eq(varName, varValue); };
WOQL.equals = function (varName, varValue) { return new WOQLQuery().eq(varName, varValue); };
WOQL.substr = function (string, before, length, after, substring) { return new WOQLQuery().substr(string, before, length, after, substring); };
WOQL.substring = function (string, before, length, after, substring) { return new WOQLQuery().substr(string, before, length, after, substring); };
WOQL.get = function (asvars, queryResource) { return new WOQLQuery().get(asvars, queryResource); };
WOQL.put = function (varsToExp, query, fileResource) { return new WOQLQuery().put(varsToExp, query, fileResource); };
WOQL.as = function (source, target, type) { return new WOQLQuery().as(source, target, type); };
WOQL.remote = function (remoteObj, formatObj) { return new WOQLQuery().remote(remoteObj, formatObj); };
WOQL.post = function (url, formatObj, source) { return new WOQLQuery().post(url, formatObj, source); };
WOQL.delete_triple = function (subject, predicate, object) { return new WOQLQuery().delete_triple(subject, predicate, object); };
WOQL.delete_quad = function (subject, predicate, object, graphRef) { return new WOQLQuery().delete_quad(subject, predicate, object, graphRef); };
WOQL.add_triple = function (subject, predicate, object) { return new WOQLQuery().add_triple(subject, predicate, object); };
WOQL.add_quad = function (subject, predicate, object, graphRef) { return new WOQLQuery().add_quad(subject, predicate, object, graphRef); };
WOQL.trim = function (inputStr, resultVarName) { return new WOQLQuery().trim(inputStr, resultVarName); };
WOQL.evaluate = function (arithExp, resultVarName) { return new WOQLQuery().eval(arithExp, resultVarName); };
WOQL.eval = function (arithExp, resultVarName) { return new WOQLQuery().eval(arithExp, resultVarName); };
WOQL.plus = function (...args) { return new WOQLQuery().plus(...args); };
WOQL.minus = function (...args) { return new WOQLQuery().minus(...args); };
WOQL.times = function (...args) { return new WOQLQuery().times(...args); };
WOQL.divide = function (...args) { return new WOQLQuery().divide(...args); };
WOQL.div = function (...args) { return new WOQLQuery().div(...args); };
WOQL.exp = function (varNum, expNum) { return new WOQLQuery().exp(varNum, expNum); };
WOQL.floor = function (varNum) { return new WOQLQuery().floor(varNum); };
WOQL.isa = function (instanceIRI, classId) { return new WOQLQuery().isa(instanceIRI, classId); };
WOQL.like = function (stringA, stringB, distance) { return new WOQLQuery().like(stringA, stringB, distance); };
WOQL.less = function (varNum01, varNum02) { return new WOQLQuery().less(varNum01, varNum02); };
WOQL.greater = function (varNum01, varNum02) { return new WOQLQuery().greater(varNum01, varNum02); };
WOQL.opt = function (subquery) { return new WOQLQuery().opt(subquery); };
WOQL.optional = function (subquery) { return new WOQLQuery().opt(subquery); };
WOQL.unique = function (prefix, inputVarList, resultVarName) { return new WOQLQuery().unique(prefix, inputVarList, resultVarName); };
WOQL.idgen = function (prefix, inputVarList, resultVarName) { return new WOQLQuery().idgen(prefix, inputVarList, resultVarName); };
WOQL.idgenerator = function (prefix, inputVarList, resultVarName) { return new WOQLQuery().idgen(prefix, inputVarList, resultVarName); };
WOQL.upper = function (inputVarName, resultVarName) { return new WOQLQuery().upper(inputVarName, resultVarName); };
WOQL.lower = function (inputVarName, resultVarName) { return new WOQLQuery().lower(inputVarName, resultVarName); };
WOQL.pad = function (inputVarName, pad, len, resultVarName) { return new WOQLQuery().pad(inputVarName, pad, len, resultVarName); };
WOQL.split = function (inputVarName, separator, resultVarName) { return new WOQLQuery().split(inputVarName, separator, resultVarName); };
WOQL.member = function (element, list) { return new WOQLQuery().member(element, list); };
WOQL.concat = function (varList, resultVarName) { return new WOQLQuery().concat(varList, resultVarName); };
WOQL.join = function (varList, glue, resultVarName) { return new WOQLQuery().join(varList, glue, resultVarName); };
WOQL.sum = function (subquery, total) { return new WOQLQuery().sum(subquery, total); };
WOQL.start = function (start, subquery) { return new WOQLQuery().start(start, subquery); };
WOQL.limit = function (limit, subquery) { return new WOQLQuery().limit(limit, subquery); };
WOQL.re = function (pattern, inputVarName, resultVarList) { return new WOQLQuery().re(pattern, inputVarName, resultVarList); };
WOQL.regexp = function (pattern, inputVarName, resultVarList) { return new WOQLQuery().re(pattern, inputVarName, resultVarList); };
WOQL.length = function (inputVarList, resultVarName) { return new WOQLQuery().length(inputVarList, resultVarName); };
WOQL.not = function (subquery) { return new WOQLQuery().not(subquery); };
WOQL.once = function (subquery) { return new WOQLQuery().once(subquery); };
WOQL.immediately = function (subquery) { return new WOQLQuery().immediately(subquery); };
WOQL.count = function (countVarName, subquery) { return new WOQLQuery().count(countVarName, subquery); };
WOQL.typecast = function (varName, varType, resultVarName) { return new WOQLQuery().typecast(varName, varType, resultVarName); };
WOQL.cast = function (varName, varType, resultVarName) { return new WOQLQuery().typecast(varName, varType, resultVarName); };
WOQL.order_by = function (...varNames) { return new WOQLQuery().order_by(...varNames); };
WOQL.group_by = function (varList, patternVars, resultVarName, subquery) { return new WOQLQuery().group_by(varList, patternVars, resultVarName, subquery); };
WOQL.true = function () { return new WOQLQuery().true(); };
WOQL.path = function (subject, pattern, object, resultVarName) { return new WOQLQuery().path(subject, pattern, object, resultVarName); };
WOQL.size = function (resourceId, resultVarName) { return new WOQLQuery().size(resourceId, resultVarName); };
WOQL.triple_count = function (resourceId, tripleCount) { return new WOQLQuery().triple_count(resourceId, tripleCount); };
WOQL.type_of = function (elementId, elementType) { return new WOQLQuery().type_of(elementId, elementType); };
WOQL.star = function (graph, subject, predicate, object) { return new WOQLQuery().star(graph, subject, predicate, object); };
WOQL.all = function (subject, predicate, object, graphRef) { return new WOQLQuery().all(subject, predicate, object, graphRef); };
WOQL.node = function (nodeid, chainType) { return new WOQLQuery().node(nodeid, chainType); };
WOQL.insert = function (classId, classType, graphRef) { return new WOQLQuery().insert(classId, classType, graphRef); };
WOQL.graph = function (graphRef) { return new WOQLQuery().graph(graphRef); };
WOQL.nuke = function (graphRef) { return new WOQLQuery().nuke(graphRef); };
WOQL.query = function () { return new WOQLQuery(); };
WOQL.json = function (JSON_LD) { return new WOQLQuery().json(JSON_LD); };
WOQL.lib = function () { return new WOQLLibrary(); };
WOQL.string = function (val) { return new WOQLQuery().string(val); };
WOQL.literal = function (val, type) { return new WOQLQuery().literal(val, type); };
WOQL.date = function (date) { return new WOQLQuery().literal(date, 'xsd:date'); };
WOQL.datetime = function (datetime) { return new WOQLQuery().literal(datetime, 'xsd:dateTime'); };
WOQL.boolean = function (bool) { return new WOQLQuery().boolean(bool); };
WOQL.iri = function (val) { return new WOQLQuery().iri(val); };
WOQL.vars = function (...varNames) { return varNames.map((item) => new Var(item)); };
WOQL.vars_unique = function (...varNames) { return varNames.map((item) => new VarUnique(item)); };
WOQL.vars_unique_reset_start = function (start) { SetVarsUniqueCounter(start ?? 0); };
WOQL.doc = function (object) { return new Doc(object); };
WOQL.Vars = function (...varNames) { return new Vars(...varNames); };
WOQL.VarsUnique = function (...varNames) { return new VarsUnique(...varNames); };
WOQL.read_document = function (IRI, output) { return new WOQLQuery().read_document(IRI, output); };
WOQL.insert_document = function (docjson, IRI) { return new WOQLQuery().insert_document(docjson, IRI); };
WOQL.update_document = function (docjson, IRI) { return new WOQLQuery().update_document(docjson, IRI); };
WOQL.delete_document = function (IRI) { return new WOQLQuery().delete_document(IRI); };
WOQL.update_triple = function (subject, predicate, newObjValue, oldObjValue) { return new WOQLQuery().update_triple(subject, predicate, newObjValue, oldObjValue); };
WOQL.update_quad = function (subject, predicate, newObject, graphRef) { return new WOQLQuery().update_quad(subject, predicate, newObject, graphRef); };
WOQL.value = function (subject, predicate, objValue) { return new WOQLQuery().value(subject, predicate, objValue); };
WOQL.link = function (subject, predicate, object) { return new WOQLQuery().link(subject, predicate, object); };
WOQL.dot = function (document, field, value) { return new WOQLQuery().dot(document, field, value); };

// The emerge function that generates the prelude
WOQL.emerge = function (auto_eval) {
  const unemerged = ['emerge', 'true', 'eval'];
  function _emerge_str(k) {
    const str = `function ${k}(...args){
            return WOQL.${k}(...args)
        }`;
    return str;
  }
  const funcs = [_emerge_str('Vars')];
  for (const k in this) {
    if (typeof this[k] === 'function') {
      if (unemerged.indexOf(k) === -1) {
        funcs.push(_emerge_str(k));
      }
    }
  }
  const str = funcs.join(';\n');
  if (auto_eval) eval(str);
  return str;
};

module.exports = WOQL;
