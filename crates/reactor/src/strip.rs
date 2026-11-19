//! Taking back out of a response the fields we put into the query.
//!
//! Reactor rewrites a GraphQL query, adding `_id`/`_type` to every object
//! selection, because a GraphQL response is a tree of anonymous objects and
//! without an id there is no way to say *which document* a node came from — the
//! difference between depending on `Doc(Person/alice)` and depending on every
//! instance of `Person`. The client asked for neither field, so the client must
//! not receive them: a proxy that adds keys to a response is not transparent,
//! and a typed client (codegen, or `deny_unknown_fields`) breaks on it.
//!
//! That leaves a tension with invariant 1 — *never re-serialize a response* —
//! because v12 returns numbers at full rational precision and a round trip
//! through `serde_json::Value` would reorder keys and could reshape values. So
//! this module does not parse-and-print. It **scans** the bytes, works out the
//! exact byte ranges the injected members occupy (key, colon, value, and the one
//! comma that joined them), and copies everything else through untouched. Every
//! byte the client receives is a byte the origin sent.
//!
//! Two consequences worth stating:
//!
//! * **A parse failure is not a fallback.** If the scan does not reach a clean
//!   end of input, [`strip`] returns `None` and the caller must *not* serve the
//!   body — it has to go back to the origin with the client's own query. A
//!   half-stripped body is the one outcome worse than an over-full one.
//! * **The strip is per request, not per fragment.** `{Person{name}}` and
//!   `{Person{name _id}}` normalize to the same shape and share one cached
//!   fragment — which is a feature, since the stored bytes are identical — but
//!   the first client gets `_id` removed and the second does not. So the spec
//!   comes from *this* request's analysis, and it is mixed into the `ETag`
//!   ([`StripSpec::fingerprint`]) so an `ETag` still names the bytes the client
//!   actually got.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use bytes::{Bytes, BytesMut};

use crate::analyze::InjectedField;

/// Which fields to remove, at which response paths.
///
/// Paths are response keys from the root of the response object, outermost
/// first, with arrays contributing nothing — `data.Person` addresses every
/// element of the `Person` list, which is exactly the level injection happened
/// at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StripSpec {
    at: BTreeMap<Vec<String>, BTreeSet<String>>,
}

impl StripSpec {
    /// The spec for what one request's analysis injected.
    pub fn new(injected: &[InjectedField]) -> Self {
        let mut at: BTreeMap<Vec<String>, BTreeSet<String>> = BTreeMap::new();
        for field in injected {
            // `InjectedField::path` is relative to the response's `data` object,
            // which is where a GraphQL result lives.
            let mut path = Vec::with_capacity(field.path.len() + 1);
            path.push("data".to_owned());
            path.extend(field.path.iter().cloned());
            at.entry(path).or_default().insert(field.field.clone());
        }
        Self { at }
    }

    /// Nothing to remove, so nothing to scan.
    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }

    /// A stable fingerprint of the spec.
    ///
    /// Two clients sharing a fragment but stripping differently must not be
    /// handed the same `ETag`, or one client's `If-None-Match` could match the
    /// other's bytes.
    pub fn fingerprint(&self) -> u64 {
        // FNV-1a over the canonical rendering. Not cryptographic, and does not
        // need to be: it distinguishes specs, it does not authenticate them.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |bytes: &[u8]| {
            for &byte in bytes {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x1000_0000_01b3);
            }
        };
        for (path, fields) in &self.at {
            for segment in path {
                eat(segment.as_bytes());
                eat(b"/");
            }
            eat(b":");
            for field in fields {
                eat(field.as_bytes());
                eat(b",");
            }
            eat(b";");
        }
        hash
    }

    /// The fields to remove from an object at this path, if any.
    fn fields_at(&self, path: &[String]) -> Option<&BTreeSet<String>> {
        self.at.get(path)
    }

    /// Could any object below this path need stripping? Lets the scanner skip
    /// whole subtrees without descending them.
    fn relevant_below(&self, path: &[String]) -> bool {
        self.at
            .keys()
            .any(|target| target.len() >= path.len() && target.starts_with(path))
    }
}

/// Remove the injected members from `body`, copying every other byte.
///
/// `None` means the body was not a single well-formed JSON value, or the scan
/// produced cuts that do not make sense; the caller must then fetch the client's
/// own query from the origin rather than serve something half-edited.
pub fn strip(body: &[u8], spec: &StripSpec) -> Option<Bytes> {
    if spec.is_empty() {
        return Some(Bytes::copy_from_slice(body));
    }

    let mut scanner = Scanner {
        bytes: body,
        pos: 0,
        spec,
        cuts: Vec::new(),
    };
    scanner.space();
    let mut path: Vec<String> = Vec::new();
    scanner.value(&mut path)?;
    scanner.space();
    if scanner.pos != body.len() {
        // Trailing content: not one JSON value, so our idea of the structure is
        // wrong and the cuts cannot be trusted.
        return None;
    }

    let mut cuts = scanner.cuts;
    cuts.sort_by_key(|cut| cut.start);
    let mut out = BytesMut::with_capacity(body.len());
    let mut copied = 0usize;
    for cut in &cuts {
        if cut.start < copied || cut.end > body.len() || cut.start > cut.end {
            // Overlapping or impossible: a bug in the scanner, and not something
            // to paper over by serving the result.
            return None;
        }
        out.extend_from_slice(&body[copied..cut.start]);
        copied = cut.end;
    }
    out.extend_from_slice(&body[copied..]);
    Some(out.freeze())
}

/// A byte scanner that records ranges to delete and never allocates a value.
struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    spec: &'a StripSpec,
    cuts: Vec<Range<usize>>,
}

/// One member of an object, as byte positions.
struct Member {
    /// Key, decoded only far enough to compare against a field name.
    key: String,
    /// The opening quote of the key.
    start: usize,
    /// One past the last byte of the value.
    end: usize,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        (self.peek()? == byte).then(|| {
            self.pos += 1;
        })
    }

    /// Scan any JSON value, leaving `pos` one past its last byte.
    fn value(&mut self, path: &mut Vec<String>) -> Option<()> {
        match self.peek()? {
            b'{' => self.object(path),
            b'[' => self.array(path),
            b'"' => self.string().map(|_| ()),
            _ => self.atom(),
        }
    }

    /// A number, `true`, `false` or `null`: everything up to a structural byte.
    ///
    /// We never interpret these — the whole point is that the bytes travel
    /// unexamined — so "where does it end" is all we need.
    fn atom(&mut self) -> Option<()> {
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if matches!(byte, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                break;
            }
            self.pos += 1;
        }
        (self.pos > start).then_some(())
    }

    /// A string literal, returned decoded *only* for the simple case.
    ///
    /// Object keys we care about (`_id`, `_type`) contain no escapes, so an
    /// escaped key simply cannot match one and its exact text does not matter.
    fn string(&mut self) -> Option<String> {
        self.expect(b'"')?;
        let start = self.pos;
        let mut escaped = false;
        let mut simple = true;
        loop {
            let byte = self.peek()?;
            self.pos += 1;
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    simple = false;
                }
                b'"' => break,
                _ => {}
            }
        }
        let text = &self.bytes[start..self.pos - 1];
        if simple {
            std::str::from_utf8(text).ok().map(str::to_owned)
        } else {
            // Deliberately not a name any injected field can have.
            Some(String::new())
        }
    }

    fn array(&mut self, path: &mut Vec<String>) -> Option<()> {
        self.expect(b'[')?;
        let mut first = true;
        loop {
            self.space();
            if self.peek()? == b']' {
                self.pos += 1;
                return Some(());
            }
            if !first {
                self.expect(b',')?;
                self.space();
            }
            first = false;
            // An array adds no path segment: the elements of `data.Person` are
            // addressed as `data.Person`, which is the level ids were injected
            // at.
            self.value(path)?;
        }
    }

    fn object(&mut self, path: &mut Vec<String>) -> Option<()> {
        self.expect(b'{')?;
        let remove = self.spec.fields_at(path).cloned().unwrap_or_default();
        // Nothing here and nothing under here: skip the subtree wholesale.
        let descend = self.spec.relevant_below(path);
        let mut members: Vec<Member> = Vec::new();

        loop {
            self.space();
            if self.peek()? == b'}' {
                self.pos += 1;
                break;
            }
            if !members.is_empty() {
                self.expect(b',')?;
                self.space();
            }
            let start = self.pos;
            let key = self.string()?;
            self.space();
            self.expect(b':')?;
            self.space();
            let mark = self.cuts.len();
            if descend {
                path.push(key.clone());
                self.value(path)?;
                path.pop();
            } else {
                self.value(&mut Vec::new())?;
            }
            if remove.contains(&key) {
                // Anything we found inside a member we are about to delete is
                // inside the deleted range; keeping both would double-cut.
                self.cuts.truncate(mark);
            }
            members.push(Member {
                key,
                start,
                end: self.pos,
            });
        }

        if !remove.is_empty() {
            self.cut_members(&members, &remove);
        }
        Some(())
    }

    /// Turn "these members go" into byte ranges, one per *run* of consecutive
    /// doomed members.
    ///
    /// A member is `"key": value`, joined to its neighbours by commas, so the
    /// range has to swallow exactly one comma — the one before the run if
    /// anything survives above it, otherwise the one after. Taking runs rather
    /// than single members is what keeps two adjacent removals from producing
    /// overlapping ranges, and injected fields are always appended together.
    fn cut_members(&mut self, members: &[Member], remove: &BTreeSet<String>) {
        let doomed: Vec<bool> = members.iter().map(|m| remove.contains(&m.key)).collect();
        let mut i = 0;
        while i < members.len() {
            if !doomed[i] {
                i += 1;
                continue;
            }
            let run_start = i;
            while i + 1 < members.len() && doomed[i + 1] {
                i += 1;
            }
            let run_end = i;
            let kept_before = (0..run_start).rev().find(|&k| !doomed[k]);
            let kept_after = (run_end + 1..members.len()).find(|&k| !doomed[k]);
            let cut = match (kept_before, kept_after) {
                // Take the comma that joined us to what came before.
                (Some(previous), _) => members[previous].end..members[run_end].end,
                // Nothing before: take the comma that joins us to what follows.
                (None, Some(next)) => members[run_start].start..members[next].start,
                // The object becomes empty, which is `{}` plus whatever
                // whitespace the origin had inside it.
                (None, None) => members[run_start].start..members[run_end].end,
            };
            self.cuts.push(cut);
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(paths: &[(&[&str], &str)]) -> StripSpec {
        StripSpec::new(
            &paths
                .iter()
                .map(|(path, field)| InjectedField {
                    path: path.iter().map(|s| (*s).to_owned()).collect(),
                    field: (*field).to_owned(),
                })
                .collect::<Vec<_>>(),
        )
    }

    fn ids_at(path: &[&str]) -> StripSpec {
        spec(&[(path, "_id"), (path, "_type")])
    }

    fn stripped(body: &str, spec: &StripSpec) -> String {
        String::from_utf8(strip(body.as_bytes(), spec).expect("a clean scan").to_vec()).unwrap()
    }

    #[test]
    fn an_empty_spec_copies_the_body() {
        let body = r#"{"data":{"Person":[{"name":"Ann"}]}}"#;
        assert_eq!(stripped(body, &StripSpec::default()), body);
    }

    #[test]
    fn the_injected_ids_leave_and_nothing_else_moves() {
        let body = r#"{"data":{"Person":[{"name":"Ann","_id":"Person/alice","_type":"Person"}]}}"#;
        assert_eq!(
            stripped(body, &ids_at(&["Person"])),
            r#"{"data":{"Person":[{"name":"Ann"}]}}"#
        );
    }

    #[test]
    fn every_element_of_a_list_is_stripped() {
        let body = r#"{"data":{"Person":[{"_id":"Person/a","name":"Ann"},{"_id":"Person/b","name":"Bo"}]}}"#;
        assert_eq!(
            stripped(body, &spec(&[(&["Person"], "_id")])),
            r#"{"data":{"Person":[{"name":"Ann"},{"name":"Bo"}]}}"#
        );
    }

    #[test]
    fn nested_objects_are_stripped_at_their_own_level() {
        let body = concat!(
            r#"{"data":{"Person":[{"name":"Ann","_id":"Person/a","_type":"Person","#,
            r#""friend":{"name":"Bo","_id":"Person/b","_type":"Person"}}]}}"#
        );
        assert_eq!(
            stripped(body, &spec(&[(&["Person", "friend"], "_id")])),
            concat!(
                r#"{"data":{"Person":[{"name":"Ann","_id":"Person/a","_type":"Person","#,
                r#""friend":{"name":"Bo","_type":"Person"}}]}}"#
            ),
            "a level with no record keeps its ids: the client asked for those"
        );
    }

    #[test]
    fn a_field_the_client_selected_survives_at_a_level_we_did_not_touch() {
        // The analysis records nothing for `Person`, because the client asked
        // for `_id` there itself.
        let body = r#"{"data":{"Person":[{"_id":"Person/a","name":"Ann"}]}}"#;
        assert_eq!(stripped(body, &ids_at(&["Company"])), body);
    }

    #[test]
    fn a_string_that_merely_looks_like_a_key_is_untouched() {
        let body = r#"{"data":{"Person":[{"name":"\"_id\": not a key","_id":"Person/a"}]}}"#;
        assert_eq!(
            stripped(body, &spec(&[(&["Person"], "_id")])),
            r#"{"data":{"Person":[{"name":"\"_id\": not a key"}]}}"#
        );
    }

    #[test]
    fn the_first_member_takes_the_comma_that_follows_it() {
        let body = r#"{"data":{"Person":[{"_id":"Person/a","_type":"Person","name":"Ann"}]}}"#;
        assert_eq!(
            stripped(body, &ids_at(&["Person"])),
            r#"{"data":{"Person":[{"name":"Ann"}]}}"#
        );
    }

    #[test]
    fn an_object_of_nothing_but_injected_fields_becomes_empty() {
        let body = r#"{"data":{"Person":[{"_id":"Person/a","_type":"Person"}]}}"#;
        assert_eq!(
            stripped(body, &ids_at(&["Person"])),
            r#"{"data":{"Person":[{}]}}"#
        );
    }

    #[test]
    fn whitespace_elsewhere_is_preserved_exactly() {
        let body = "{\n  \"data\": {\n    \"Person\": [\n      {\n        \"name\": \"Ann\",\n        \"_id\": \"Person/a\"\n      }\n    ]\n  }\n}";
        assert_eq!(
            stripped(body, &spec(&[(&["Person"], "_id")])),
            "{\n  \"data\": {\n    \"Person\": [\n      {\n        \"name\": \"Ann\"\n      }\n    ]\n  }\n}"
        );
    }

    #[test]
    fn a_high_precision_number_is_copied_not_reinterpreted() {
        let body = r#"{"data":{"Ledger":[{"amount":0.1234567890123456789012345,"_id":"L/1"}]}}"#;
        assert_eq!(
            stripped(body, &spec(&[(&["Ledger"], "_id")])),
            r#"{"data":{"Ledger":[{"amount":0.1234567890123456789012345}]}}"#,
            "invariant 1: digits survive because they are never parsed"
        );
    }

    #[test]
    fn nulls_and_scalars_at_the_stripped_level_do_not_confuse_the_scan() {
        let body = r#"{"data":{"Person":[{"name":null,"age":30,"ok":true,"_id":"P/a"}]}}"#;
        assert_eq!(
            stripped(body, &spec(&[(&["Person"], "_id")])),
            r#"{"data":{"Person":[{"name":null,"age":30,"ok":true}]}}"#
        );
    }

    #[test]
    fn a_null_where_an_object_was_expected_is_fine() {
        let body = r#"{"data":{"Person":null}}"#;
        assert_eq!(stripped(body, &ids_at(&["Person"])), body);
    }

    #[test]
    fn an_errors_envelope_passes_through() {
        let body = r#"{"data":null,"errors":[{"message":"nope","path":["Person","_id"]}]}"#;
        assert_eq!(stripped(body, &ids_at(&["Person"])), body);
    }

    #[test]
    fn a_truncated_body_is_refused_rather_than_half_edited() {
        let body = r#"{"data":{"Person":[{"name":"Ann","_id":"Person/a""#;
        assert!(strip(body.as_bytes(), &ids_at(&["Person"])).is_none());
    }

    #[test]
    fn trailing_rubbish_is_refused() {
        let body = r#"{"data":{"Person":[]}} and then some"#;
        assert!(strip(body.as_bytes(), &ids_at(&["Person"])).is_none());
    }

    #[test]
    fn an_ndjson_body_is_refused_because_it_is_not_one_value() {
        let body = "{\"@id\":\"Person/a\"}\n{\"@id\":\"Person/b\"}\n";
        assert!(strip(body.as_bytes(), &ids_at(&["Person"])).is_none());
    }

    #[test]
    fn the_fingerprint_separates_specs_and_is_stable() {
        let one = ids_at(&["Person"]);
        let other = ids_at(&["Company"]);
        assert_ne!(one.fingerprint(), other.fingerprint());
        assert_eq!(one.fingerprint(), ids_at(&["Person"]).fingerprint());
        assert_ne!(
            one.fingerprint(),
            spec(&[(&["Person"], "_id")]).fingerprint(),
            "stripping one field is not stripping two"
        );
        assert_eq!(
            StripSpec::default().fingerprint(),
            StripSpec::new(&[]).fingerprint()
        );
    }

    #[test]
    fn an_unrelated_subtree_is_skipped_but_still_scanned_for_structure() {
        // `relevant_below` stops us descending into `Company`, and the body is
        // still copied whole — which is the point of the optimisation.
        let body = r#"{"data":{"Company":[{"_id":"C/1","staff":[{"_id":"P/1"}]}],"Person":[{"_id":"P/1","name":"Ann"}]}}"#;
        assert_eq!(
            stripped(body, &spec(&[(&["Person"], "_id")])),
            r#"{"data":{"Company":[{"_id":"C/1","staff":[{"_id":"P/1"}]}],"Person":[{"name":"Ann"}]}}"#
        );
    }
}
