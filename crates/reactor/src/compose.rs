//! Russian-doll decomposition of document-list responses.
//!
//! A `GET /api/document/<ns>?type=Note` response is one cache entry today, so a
//! field change to any single member throws the whole list away. This module is
//! the other half of the design doc's §5.4: it splits such a response into the
//! per-document byte ranges it is *made* of, and puts it back together.
//!
//! # Projection versus framing
//!
//! The split that makes this work is between
//!
//! * **projection** — which bytes one document renders as (`unfold`,
//!   `minimized`, `compress_ids`, `identifiers`, `raw_id`, `xsd`, `format`, …),
//!   which belongs to the *child* fragment's shape, and
//! * **framing** — how documents are joined (NDJSON versus `as_list`, plus the
//!   `skip`/`count` window and the origin's ordering), which belongs to the
//!   *parent*.
//!
//! Because the per-document bytes are the same in both contexts, a by-id read
//! and a list read can share one child fragment — which is the whole win: a
//! list read warms by-id reads and vice versa.
//!
//! # Framing is learned, never assumed
//!
//! [`decompose`] does not know what NDJSON or `as_list` look like. It finds the
//! document objects and keeps *everything else verbatim* as three byte strings:
//! a prefix, one separator, and a suffix. [`compose`] concatenates them back.
//! That makes recomposition byte-exact by construction for the membership the
//! framing was learned from, and it means `minimized=false`, `as_list=true`,
//! `unfold=true` and any future switch that only reshapes the wrapper need no
//! code here at all.
//!
//! The price is a rule: the separator must be *uniform*. If the origin ever
//! joins documents with varying bytes, [`decompose`] declines rather than
//! guessing, and the caller falls back to whole-response caching. Declining is
//! always safe; guessing would return subtly different bytes.
//!
//! # Byte fidelity
//!
//! Nothing here re-serializes. Document bytes are `Bytes` slices of the origin's
//! own body, framing is `Bytes` of the gaps between them, and composition is
//! concatenation. JSON is *parsed* only to read `@id` out of a segment — never
//! to re-emit it (invariant 1).

use std::ops::Range;

use bytes::{Bytes, BytesMut};

use crate::ids::DocId;

/// The wrapper bytes a list response puts around its members.
///
/// Learned from an actual origin response; see the module docs. All three
/// fields are verbatim origin bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framing {
    /// Bytes before the first document (`[\n` for `as_list`, empty for NDJSON).
    pub prefix: Bytes,
    /// Bytes between two adjacent documents (`,\n` for `as_list`, `\n` for
    /// NDJSON). Uniform by construction: [`decompose`] refuses a response whose
    /// separators differ.
    pub separator: Bytes,
    /// Bytes after the last document (`\n]\n` for `as_list`, `\n` for NDJSON).
    pub suffix: Bytes,
}

impl Framing {
    /// Framing for a body that contains no documents at all: the whole body is
    /// prefix.
    ///
    /// An empty list is still a cacheable, composable answer — it just has
    /// nothing but wrapper in it.
    pub fn whole(body: Bytes) -> Self {
        Self {
            prefix: body,
            separator: Bytes::new(),
            suffix: Bytes::new(),
        }
    }
}

/// One document found inside a list response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// The document's id, read from `@id` and normalized to short form.
    pub id: DocId,
    /// Its rendering, verbatim: no wrapper, no separator, no trailing newline.
    /// Exactly the bytes a by-id read of the same projection returns for it.
    pub bytes: Bytes,
}

/// A list response taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decomposed {
    /// The wrapper, to put it back together with.
    pub framing: Framing,
    /// The members, in the order the origin returned them. Order is part of the
    /// answer, so it is part of the parent fragment.
    pub members: Vec<Member>,
}

impl Decomposed {
    /// The member ids, in order.
    pub fn ids(&self) -> Vec<DocId> {
        self.members.iter().map(|m| m.id.clone()).collect()
    }
}

/// Why a body could not be decomposed.
///
/// Every variant means "fall back to whole-response caching", which is always
/// correct. They are distinguished only so that an operator can see *which*
/// assumption a route broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undecomposable {
    /// Braces or a string literal did not close: not a body we understand.
    Unbalanced,
    /// A segment that looked like a document did not parse as a JSON object.
    NotAnObject,
    /// A segment had no usable `@id`, so it cannot be keyed as a document.
    /// `counted=true` lands here, which is why that parameter needs no special
    /// case.
    NoId,
    /// Adjacent documents were joined by differing bytes, so one separator
    /// cannot reproduce the body.
    RaggedSeparators,
}

/// Split a document-list response into its members and its framing.
///
/// Returns `Err` when the body is not a sequence of `@id`-bearing JSON objects
/// joined by a uniform separator. That is not a failure: the caller caches the
/// whole response instead, which is what Reactor did before this module existed.
pub fn decompose(body: &Bytes) -> Result<Decomposed, Undecomposable> {
    let ranges = object_ranges(body).ok_or(Undecomposable::Unbalanced)?;
    if ranges.is_empty() {
        return Ok(Decomposed {
            framing: Framing::whole(body.clone()),
            members: Vec::new(),
        });
    }

    // One separator must reproduce every gap, or concatenation cannot be
    // byte-exact.
    let mut separator = Bytes::new();
    for (i, pair) in ranges.windows(2).enumerate() {
        let gap = body.slice(pair[0].end..pair[1].start);
        if i == 0 {
            separator = gap;
        } else if gap != separator {
            return Err(Undecomposable::RaggedSeparators);
        }
    }

    let mut members = Vec::with_capacity(ranges.len());
    for range in &ranges {
        let bytes = body.slice(range.clone());
        members.push(Member {
            id: document_id(&bytes)?,
            bytes,
        });
    }

    let first = ranges[0].start;
    let last = ranges[ranges.len() - 1].end;
    Ok(Decomposed {
        framing: Framing {
            prefix: body.slice(..first),
            separator,
            suffix: body.slice(last..),
        },
        members,
    })
}

/// Put a list response back together from member bytes and framing.
///
/// Pure concatenation, which is the reason NDJSON lists are the one
/// decomposition worth doing first.
pub fn compose(framing: &Framing, members: &[Bytes]) -> Bytes {
    let total = framing.prefix.len()
        + framing.suffix.len()
        + members.iter().map(Bytes::len).sum::<usize>()
        + framing.separator.len() * members.len().saturating_sub(1);
    let mut out = BytesMut::with_capacity(total);
    out.extend_from_slice(&framing.prefix);
    for (i, member) in members.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(&framing.separator);
        }
        out.extend_from_slice(member);
    }
    out.extend_from_slice(&framing.suffix);
    out.freeze()
}

/// Read `@id` out of one document's bytes.
///
/// Parsing to *inspect*: the value is read and discarded, and the bytes we
/// store and serve are the origin's own.
fn document_id(bytes: &Bytes) -> Result<DocId, Undecomposable> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Undecomposable::NotAnObject)?;
    let object = value.as_object().ok_or(Undecomposable::NotAnObject)?;
    let raw = object
        .get("@id")
        .and_then(serde_json::Value::as_str)
        .ok_or(Undecomposable::NoId)?;
    let id = DocId::parse(raw);
    if id.as_str().is_empty() {
        return Err(Undecomposable::NoId);
    }
    Ok(id)
}

/// Byte ranges of the top-level JSON objects in a body.
///
/// A hand-rolled scan rather than `serde_json`'s stream deserializer because
/// the same routine has to work for an NDJSON sequence *and* for the single
/// top-level array `as_list=true` returns. Brackets, commas and whitespace
/// outside an object are framing and are simply skipped, which is what makes
/// both shapes one case.
///
/// `None` means the body is not balanced (truncated, or an unterminated string),
/// and therefore not something we may take apart.
fn object_ranges(body: &[u8]) -> Option<Vec<Range<usize>>> {
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (i, &byte) in body.iter().enumerate() {
        if in_string {
            // Escapes are tracked only so that a `\"` does not end the string;
            // what the escape *means* is irrelevant here.
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            // Strings are skipped at every depth, including zero: a brace
            // inside a string literal must not open a document.
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    ranges.push(start..i + 1);
                }
            }
            _ => {}
        }
    }

    if depth != 0 || in_string {
        return None;
    }
    Some(ranges)
}

/// The parent fragment's content: its framing and its ordered membership.
///
/// This is what a parent fragment *stores in place of document bytes*, and it is
/// deliberately the parent's whole content identity. The engine hashes a
/// fragment's body to decide whether a refill actually changed anything
/// (backdating), so putting both the framing and the ordered ids in here — and
/// nothing else — makes "the parent is unchanged" mean exactly "same members, in
/// the same order, in the same wrapper". A child's bytes changing does not touch
/// it, which is revision R4 of the design doc: russian doll by reference, not by
/// transitive dirtying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spine {
    /// The wrapper to compose with.
    pub framing: Framing,
    /// Member ids, in origin order.
    pub ids: Vec<DocId>,
}

/// Magic so a body that is not a spine cannot be mistaken for one — e.g. after
/// a configuration change flips decomposition off and back on while old
/// whole-response entries are still cached.
const SPINE_MAGIC: &[u8] = b"rxspine1";

impl Spine {
    /// The spine of a decomposed response.
    pub fn of(decomposed: &Decomposed) -> Self {
        Self {
            framing: decomposed.framing.clone(),
            ids: decomposed.ids(),
        }
    }

    /// Encode to the bytes a parent fragment holds.
    ///
    /// Length-prefixed and therefore injective: no two distinct spines encode to
    /// the same bytes, which is what lets the engine use the hash of this
    /// encoding as the parent's content identity.
    pub fn encode(&self) -> Bytes {
        let mut out = BytesMut::new();
        out.extend_from_slice(SPINE_MAGIC);
        push(&mut out, &self.framing.prefix);
        push(&mut out, &self.framing.separator);
        push(&mut out, &self.framing.suffix);
        for id in &self.ids {
            push(&mut out, id.as_str().as_bytes());
        }
        out.freeze()
    }

    /// Decode a parent fragment's bytes. `None` for anything that is not a
    /// spine, which the caller treats as a plain whole-response entry.
    ///
    /// The framing comes back as `Bytes` slices of the same buffer, so decoding
    /// a parent on the hit path copies nothing.
    pub fn decode(bytes: &Bytes) -> Option<Spine> {
        if !bytes.starts_with(SPINE_MAGIC) {
            return None;
        }
        let mut cursor = SPINE_MAGIC.len();
        let prefix = pull(bytes, &mut cursor)?;
        let separator = pull(bytes, &mut cursor)?;
        let suffix = pull(bytes, &mut cursor)?;
        let mut ids = Vec::new();
        while cursor < bytes.len() {
            let id = pull(bytes, &mut cursor)?;
            ids.push(DocId::parse(std::str::from_utf8(&bytes[id]).ok()?));
        }
        Some(Spine {
            framing: Framing {
                prefix: bytes.slice(prefix),
                separator: bytes.slice(separator),
                suffix: bytes.slice(suffix),
            },
            ids,
        })
    }
}

fn push(out: &mut BytesMut, field: &[u8]) {
    out.extend_from_slice(field.len().to_string().as_bytes());
    out.extend_from_slice(b":");
    out.extend_from_slice(field);
}

/// Read one length-prefixed field, returning its range in `buf` and advancing
/// the cursor past it.
fn pull(buf: &[u8], cursor: &mut usize) -> Option<Range<usize>> {
    let colon = buf.get(*cursor..)?.iter().position(|&b| b == b':')? + *cursor;
    let len: usize = std::str::from_utf8(buf.get(*cursor..colon)?)
        .ok()?
        .parse()
        .ok()?;
    let start = colon + 1;
    let end = start.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    *cursor = end;
    Some(start..end)
}

/// Framings learned from the origin, keyed by the request shape they came from.
///
/// Framing is a pure function of a request's parameters: `as_list=true` wraps
/// documents in an array whatever the documents are. So once one response of a
/// given shape has been seen, we know how to wrap any other — and a
/// single-document read whose child fragment is already cached then needs **no
/// upstream request at all**. That is what makes a list read genuinely warm a
/// by-id read rather than merely save it a byte or two.
///
/// Only **single-document** shapes are remembered, and the restriction is
/// load-bearing rather than cautious: the origin's suffix after a one-element
/// `as_list` body is not the suffix after a three-element one, so a framing
/// learned from N members may not be applied to M. A `?id=` read has exactly
/// one member by construction, so for that shape there is nothing to get wrong.
///
/// Nothing here is ever invalidated, because nothing a commit can do changes
/// what `[` means. The map holds one small entry per (namespace, shape).
#[derive(Default)]
pub struct FramingCache {
    learned: dashmap::DashMap<(crate::ids::Namespace, crate::ids::ShapeId), Framing>,
}

impl FramingCache {
    /// Record the framing a fill of this shape revealed.
    pub fn remember(
        &self,
        ns: &crate::ids::Namespace,
        shape: crate::ids::ShapeId,
        framing: &Framing,
    ) {
        self.learned
            .entry((ns.clone(), shape))
            .or_insert_with(|| framing.clone());
    }

    /// The framing for a shape, if one has been seen.
    pub fn get(&self, ns: &crate::ids::Namespace, shape: crate::ids::ShapeId) -> Option<Framing> {
        self.learned.get(&(ns.clone(), shape)).map(|f| f.clone())
    }

    /// How many framings are remembered, for metrics and tests.
    pub fn len(&self) -> usize {
        self.learned.len()
    }

    /// True when nothing has been learned yet.
    pub fn is_empty(&self) -> bool {
        self.learned.is_empty()
    }

    /// Forget everything, for an operator-triggered purge.
    pub fn clear(&self) {
        self.learned.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str) -> Bytes {
        Bytes::from(s.to_owned())
    }

    /// The default minimized NDJSON shape, as the 12.1 origin returns it.
    const NDJSON: &str = "{\"@id\":\"Note/1\",\"text\":\"a\"}\n\
                          {\"@id\":\"Note/2\",\"text\":\"b\"}\n\
                          {\"@id\":\"Note/3\",\"text\":\"c\"}\n";

    /// The `as_list=true` shape.
    const AS_LIST: &str = "[\n{\"@id\":\"Note/1\"},\n{\"@id\":\"Note/2\"}\n]\n";

    #[test]
    fn ndjson_round_trips_byte_for_byte() {
        let body = b(NDJSON);
        let parts = decompose(&body).expect("a plain NDJSON list");
        assert_eq!(
            parts.ids().iter().map(DocId::as_str).collect::<Vec<_>>(),
            vec!["Note/1", "Note/2", "Note/3"]
        );
        assert_eq!(parts.framing.prefix, Bytes::new());
        assert_eq!(parts.framing.separator, b("\n"));
        assert_eq!(parts.framing.suffix, b("\n"));
        let members: Vec<Bytes> = parts.members.iter().map(|m| m.bytes.clone()).collect();
        assert_eq!(compose(&parts.framing, &members), body);
    }

    #[test]
    fn as_list_round_trips_byte_for_byte() {
        let body = b(AS_LIST);
        let parts = decompose(&body).expect("an as_list list");
        assert_eq!(parts.framing.prefix, b("[\n"));
        assert_eq!(parts.framing.separator, b(",\n"));
        assert_eq!(parts.framing.suffix, b("\n]\n"));
        let members: Vec<Bytes> = parts.members.iter().map(|m| m.bytes.clone()).collect();
        assert_eq!(compose(&parts.framing, &members), body);
    }

    #[test]
    fn a_member_is_the_bare_document_with_no_wrapper() {
        // The sharing property the whole feature rests on: the bytes we store
        // for `Note/1` out of a list are the bytes a by-id read returns, so one
        // child fragment serves both.
        let list = decompose(&b(AS_LIST)).unwrap();
        let single = decompose(&b("{\"@id\":\"Note/1\"}")).unwrap();
        assert_eq!(list.members[0].bytes, single.members[0].bytes);
    }

    #[test]
    fn a_pretty_printed_list_round_trips_too() {
        // `minimized=false`: each document spans several lines, still joined by
        // one newline. Nothing in the splitter knows that.
        let body =
            b("{\n  \"@id\": \"Note/1\",\n  \"text\": \"a\"\n}\n{\n  \"@id\": \"Note/2\"\n}\n");
        let parts = decompose(&body).expect("a pretty-printed list");
        assert_eq!(parts.members.len(), 2);
        assert_eq!(parts.framing.separator, b("\n"));
        let members: Vec<Bytes> = parts.members.iter().map(|m| m.bytes.clone()).collect();
        assert_eq!(compose(&parts.framing, &members), body);
    }

    #[test]
    fn nested_objects_do_not_split_a_document() {
        // `unfold=true` embeds whole documents inside their referrer. The
        // embedded object must not become a member of its own.
        let body = b("{\"@id\":\"Note/1\",\"author\":{\"@id\":\"Person/a\"}}\n");
        let parts = decompose(&body).unwrap();
        assert_eq!(parts.members.len(), 1, "one top-level document");
        assert_eq!(parts.members[0].id.as_str(), "Note/1");
    }

    #[test]
    fn a_brace_inside_a_string_is_not_a_document() {
        let body = b("{\"@id\":\"Note/1\",\"text\":\"a { b } \\\" c\"}\n");
        let parts = decompose(&body).unwrap();
        assert_eq!(parts.members.len(), 1);
        let members: Vec<Bytes> = parts.members.iter().map(|m| m.bytes.clone()).collect();
        assert_eq!(compose(&parts.framing, &members), body);
    }

    #[test]
    fn a_single_document_round_trips() {
        for body in [
            "{\"@id\":\"Note/1\"}",
            "{\"@id\":\"Note/1\"}\n",
            "[\n{\"@id\":\"Note/1\"}\n]",
        ] {
            let body = b(body);
            let parts = decompose(&body).unwrap();
            assert_eq!(parts.members.len(), 1, "{body:?}");
            let members: Vec<Bytes> = parts.members.iter().map(|m| m.bytes.clone()).collect();
            assert_eq!(compose(&parts.framing, &members), body, "{body:?}");
        }
    }

    #[test]
    fn an_empty_list_is_all_framing() {
        for body in ["", "\n", "[]\n", "[\n]\n"] {
            let parts = decompose(&b(body)).unwrap();
            assert!(parts.members.is_empty(), "{body:?}");
            assert_eq!(compose(&parts.framing, &[]), b(body), "{body:?}");
        }
    }

    #[test]
    fn a_full_iri_id_normalizes_so_a_child_key_is_shared() {
        let parts = decompose(&b("{\"@id\":\"terminusdb:///data/Note/1\"}\n")).unwrap();
        assert_eq!(parts.members[0].id.as_str(), "Note/1");
    }

    #[test]
    fn an_object_without_an_id_is_refused() {
        // `counted=true` returns a count envelope, not documents. Refusing it
        // here is why that parameter needs no special case anywhere else.
        assert_eq!(
            decompose(&b("{\"api:count\":3}")),
            Err(Undecomposable::NoId)
        );
    }

    #[test]
    fn a_truncated_body_is_refused() {
        assert_eq!(
            decompose(&b("{\"@id\":\"Note/1\"")),
            Err(Undecomposable::Unbalanced)
        );
        assert_eq!(
            decompose(&b("{\"@id\":\"Note/1\",\"t\":\"unterminated")),
            Err(Undecomposable::Unbalanced)
        );
    }

    #[test]
    fn ragged_separators_are_refused_rather_than_guessed_at() {
        // If the origin ever joined documents with differing bytes, one
        // separator could not reproduce the body. We decline; the caller caches
        // the whole response.
        let body = b("{\"@id\":\"Note/1\"}\n{\"@id\":\"Note/2\"},{\"@id\":\"Note/3\"}");
        assert_eq!(decompose(&body), Err(Undecomposable::RaggedSeparators));
    }

    #[test]
    fn composition_is_insensitive_to_how_the_members_were_obtained() {
        // The refill path gets a member's bytes out of a *different* upstream
        // response (`?ids=`) than the parent was learned from. Composition must
        // not care.
        let list = decompose(&b(NDJSON)).unwrap();
        let refetched = decompose(&b("{\"@id\":\"Note/2\",\"text\":\"b\"}\n")).unwrap();
        let members = vec![
            list.members[0].bytes.clone(),
            refetched.members[0].bytes.clone(),
            list.members[2].bytes.clone(),
        ];
        assert_eq!(compose(&list.framing, &members), b(NDJSON));
    }

    #[test]
    fn a_spine_round_trips_through_its_encoding() {
        let parts = decompose(&b(AS_LIST)).unwrap();
        let spine = Spine::of(&parts);
        let encoded = spine.encode();
        let decoded = Spine::decode(&encoded).expect("our own encoding");
        assert_eq!(decoded, spine);
        assert_eq!(decoded.framing.prefix, b("[\n"));
        assert_eq!(
            decoded.ids.iter().map(DocId::as_str).collect::<Vec<_>>(),
            vec!["Note/1", "Note/2"]
        );
    }

    #[test]
    fn a_spine_encoding_is_injective_in_order_and_framing() {
        // The engine hashes this encoding to decide whether a parent actually
        // changed, so two different parents must never encode alike.
        let one = Spine {
            framing: Framing {
                prefix: Bytes::new(),
                separator: b("\n"),
                suffix: b("\n"),
            },
            ids: vec![DocId::parse("Note/1"), DocId::parse("Note/2")],
        };
        let reordered = Spine {
            ids: vec![DocId::parse("Note/2"), DocId::parse("Note/1")],
            ..one.clone()
        };
        let reframed = Spine {
            framing: Framing {
                prefix: b("[\n"),
                separator: b(",\n"),
                suffix: b("\n]\n"),
            },
            ..one.clone()
        };
        assert_ne!(one.encode(), reordered.encode(), "order is content");
        assert_ne!(one.encode(), reframed.encode(), "framing is content");

        // And an ambiguous concatenation cannot be forged out of the ids: the
        // length prefixes make `["a:b"]` and `["a", "b"]` distinct.
        let colonful = Spine {
            ids: vec![DocId::parse("Note/1:2")],
            ..one.clone()
        };
        let split = Spine {
            ids: vec![DocId::parse("Note/1"), DocId::parse("2")],
            ..one
        };
        assert_ne!(colonful.encode(), split.encode());
    }

    #[test]
    fn a_framing_cache_keeps_one_entry_per_namespace_and_shape() {
        use crate::ids::{Namespace, ShapeId};
        let cache = FramingCache::default();
        let ns = Namespace::new("admin", "mydb", "main");
        let other = Namespace::new("admin", "mydb", "dev");
        let bare = decompose(&b("{\"@id\":\"Note/1\"}\n")).unwrap().framing;
        let listed = decompose(&b("[\n{\"@id\":\"Note/1\"}\n]")).unwrap().framing;
        let plain = ShapeId::of(b"plain");
        let wrapped = ShapeId::of(b"wrapped");

        assert!(cache.is_empty());
        assert_eq!(cache.get(&ns, plain), None);
        cache.remember(&ns, plain, &bare);
        cache.remember(&ns, wrapped, &listed);
        assert_eq!(cache.get(&ns, plain), Some(bare.clone()));
        assert_eq!(cache.get(&ns, wrapped), Some(listed));
        assert_eq!(
            cache.get(&other, plain),
            None,
            "branches do not share learned framing"
        );

        // Re-learning is a no-op, not an overwrite: the framing for a shape
        // cannot change, so the first observation stands.
        cache.remember(&ns, plain, &Framing::whole(b("surprise")));
        assert_eq!(cache.get(&ns, plain), Some(bare));
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn a_response_body_is_not_mistaken_for_a_spine() {
        assert!(Spine::decode(&b(NDJSON)).is_none());
        assert!(Spine::decode(&b("")).is_none());
        assert!(
            Spine::decode(&b("rxspine1")).is_none(),
            "magic but no fields"
        );
        assert!(
            Spine::decode(&b("rxspine10:0:9:")).is_none(),
            "a length that runs off the end"
        );
    }
}
