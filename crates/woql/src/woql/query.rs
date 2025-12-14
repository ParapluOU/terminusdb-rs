use crate::*;

/// A named parametric query which names a specific query for later retrieval and re-use and allows the specification of bindings for a specific set of variables in the query.
ast_struct!(
    NamedQuery as named_query {
        /// A named query names a specific query for later retrieval and re-use.
        name: String,
        /// The name of the NamedQuery to be retrieved
        query: Query
    }
);

ast_struct!(
    NamedParametricQuery as named_param_query {
        /// The name of the NamedParametricQuery to be retrieved.
        name: String,
        /// The query AST as WOQL JSON.
        query: Query,
        /// Variable name list for auxilliary bindings.
        parameters: (Vec<Variable>) // todo: type
    }
);

ast_struct!(
    @transparent
    Query {
        using(Using),
        select(Select),
        distinct(Distinct),
        and(And),
        or(Or),
        from(super::control::from::From),
        into(super::control::into::Into),
        triple(Triple),
        addTriple(AddTriple),
        addedTriple(AddedTriple),
        deleteTriple(DeleteTriple),
        deletedTriple(DeletedTriple),
        link(Link),
        data(Data),
        subsumption(Subsumption),
        equals(Equals),
        substring(Substring),
        readDocument(ReadDocument),
        updateDocument(UpdateDocument),
        insertDocument(InsertDocument),
        deleteDocument(DeleteDocument),
        // Get(Get),
        addData(AddData),
        addedData(AddedData),
        addLink(AddLink),
        addedLink(AddedLink),
        deleteLink(DeleteLink),
        deletedLink(DeletedLink),
        r#if(If),
        trim(Trim),
        eval(Eval),
        isA(IsA),
        like(Like),
        less(Less),
        greater(Greater),
        optional(Optional),
        lexicalKey(LexicalKey),
        randomKey(RandomKey),
        hashKey(HashKey),
        upper(Upper),
        lower(Lower),
        pad(Pad),
        split(Split),
        member(Member),
        concatenate(Concatenate),
        join(Join),
        sum(Sum),
        start(Start),
        limit(Limit),
        regexp(Regexp),
        orderBy(OrderBy),
        groupBy(GroupBy),
        length(Length),
        not(Not),
        once(Once),
        immediately(Immediately),
        count(Count),
        typeCast(TypeCast),
        path(Path),
        dot(Dot),
        size(Size),
        tripleCount(TripleCount),
        typeOf(TypeOf)
    }
);

// todo: derive
impl ToCLIQueryAST for Query {
    fn to_ast(&self) -> String {
        match self {
            Query::using(inner) => inner.to_ast(),
            Query::select(inner) => inner.to_ast(),
            Query::distinct(inner) => inner.to_ast(),
            Query::and(inner) => inner.to_ast(),
            Query::or(inner) => inner.to_ast(),
            Query::from(inner) => inner.to_ast(),
            Query::into(inner) => inner.to_ast(),
            Query::triple(inner) => inner.to_ast(),
            Query::addTriple(inner) => inner.to_ast(),
            Query::addedTriple(inner) => inner.to_ast(),
            Query::deleteTriple(inner) => inner.to_ast(),
            Query::deletedTriple(inner) => inner.to_ast(),
            Query::link(inner) => inner.to_ast(),
            Query::data(inner) => inner.to_ast(),
            Query::subsumption(inner) => inner.to_ast(),
            Query::equals(inner) => inner.to_ast(),
            Query::substring(inner) => inner.to_ast(),
            Query::readDocument(inner) => inner.to_ast(),
            Query::updateDocument(inner) => inner.to_ast(),
            Query::insertDocument(inner) => inner.to_ast(),
            Query::deleteDocument(inner) => inner.to_ast(),
            Query::addData(inner) => inner.to_ast(),
            Query::addedData(inner) => inner.to_ast(),
            Query::addLink(inner) => inner.to_ast(),
            Query::addedLink(inner) => inner.to_ast(),
            Query::deleteLink(inner) => inner.to_ast(),
            Query::deletedLink(inner) => inner.to_ast(),
            Query::r#if(inner) => inner.to_ast(),
            Query::trim(inner) => inner.to_ast(),
            Query::eval(inner) => inner.to_ast(),
            Query::isA(inner) => inner.to_ast(),
            Query::like(inner) => inner.to_ast(),
            Query::less(inner) => inner.to_ast(),
            Query::greater(inner) => inner.to_ast(),
            Query::optional(inner) => inner.to_ast(),
            Query::lexicalKey(inner) => inner.to_ast(),
            Query::randomKey(inner) => inner.to_ast(),
            Query::hashKey(inner) => inner.to_ast(),
            Query::upper(inner) => inner.to_ast(),
            Query::lower(inner) => inner.to_ast(),
            Query::pad(inner) => inner.to_ast(),
            Query::split(inner) => inner.to_ast(),
            Query::member(inner) => inner.to_ast(),
            Query::concatenate(inner) => inner.to_ast(),
            Query::join(inner) => inner.to_ast(),
            Query::sum(inner) => inner.to_ast(),
            Query::start(inner) => inner.to_ast(),
            Query::limit(inner) => inner.to_ast(),
            Query::regexp(inner) => inner.to_ast(),
            Query::orderBy(inner) => inner.to_ast(),
            Query::groupBy(inner) => inner.to_ast(),
            Query::length(inner) => inner.to_ast(),
            Query::not(inner) => inner.to_ast(),
            Query::once(inner) => inner.to_ast(),
            Query::immediately(inner) => inner.to_ast(),
            Query::count(inner) => inner.to_ast(),
            Query::typeCast(inner) => inner.to_ast(),
            Query::path(inner) => inner.to_ast(),
            Query::dot(inner) => inner.to_ast(),
            Query::size(inner) => inner.to_ast(),
            Query::tripleCount(inner) => inner.to_ast(),
            Query::typeOf(inner) => inner.to_ast(),
        }
    }
}