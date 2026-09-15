//! Framing tests distinguish concrete semantic and provenance changes.

use sha2::Digest;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_themelios::{AtomSelection, SourceMetadata};

use super::{Encoding, Error, SubjectFingerprint};

fn digest(write: impl FnOnce(&mut Encoding) -> Result<(), Error>) -> [u8; 32] {
    let mut encoding = Encoding::new(65_536);
    write(&mut encoding).unwrap();
    encoding.hash.finalize().into()
}
fn structure(nodes: Vec<ValueNode>) -> Value {
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}
fn function(sign: Sign, name: &str, children: Vec<ValueNode>) -> Value {
    let mut nodes = vec![ValueNode::Function {
        name: name.into(),
        sign,
        arity: children.len(),
    }];
    nodes.extend(children);
    structure(nodes)
}

#[test]
fn value_tags_preserve_distinct_logical_kinds() {
    let values = [
        Value::Infimum,
        Value::Supremum,
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("a".into()),
        Value::String("a".into()),
        Value::String("#inf".into()),
        Value::String("#sup".into()),
        structure(vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)]),
        function(Sign::Positive, "f", vec![ValueNode::Number(1)]),
        function(Sign::Negative, "f", vec![ValueNode::Number(1)]),
        function(Sign::Negative, "f", vec![]),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for value in &values {
        assert!(seen.insert(digest(|encoding| encoding.value(value))));
    }
}

#[test]
fn constructor_child_order_changes_the_fingerprint() {
    let left = function(
        Sign::Positive,
        "f",
        vec![ValueNode::Number(1), ValueNode::Number(2)],
    );
    let right = function(
        Sign::Positive,
        "f",
        vec![ValueNode::Number(2), ValueNode::Number(1)],
    );
    assert_ne!(digest(|e| e.value(&left)), digest(|e| e.value(&right)));
}

#[test]
fn text_frames_prevent_adjacent_name_aliases() {
    let left = function(Sign::Positive, "ab", vec![ValueNode::Symbol("c".into())]);
    let right = function(Sign::Positive, "a", vec![ValueNode::Symbol("bc".into())]);
    assert_ne!(digest(|e| e.value(&left)), digest(|e| e.value(&right)));
}

#[test]
fn predicate_sign_changes_the_fingerprint() {
    let atom = |sign| {
        Atom::new(
            Predicate::with_sign("p", 1, sign).unwrap(),
            vec![Value::Number(1)],
        )
        .unwrap()
    };
    assert_ne!(
        digest(|e| e.atom(&atom(Sign::Positive))),
        digest(|e| e.atom(&atom(Sign::Negative)))
    );
}

fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(2, nodes, roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn connective_identity_changes_the_fingerprint() {
    let mut seen = std::collections::BTreeSet::new();
    for node in [Node::And(0, 1), Node::Or(0, 1), Node::Implies(0, 1)] {
        let subject = theory(vec![Node::Atom(0), Node::Atom(1), node], vec![2]);
        assert!(seen.insert(digest(|e| e.theory(&subject))));
    }
}

#[test]
fn edge_order_changes_the_fingerprint() {
    let left = theory(
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![2],
    );
    let right = theory(
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(1, 0)],
        vec![2],
    );
    assert_ne!(digest(|e| e.theory(&left)), digest(|e| e.theory(&right)));
}

#[test]
fn root_order_changes_the_fingerprint() {
    let left = theory(vec![Node::Atom(0), Node::Atom(1)], vec![0, 1]);
    let right = theory(vec![Node::Atom(0), Node::Atom(1)], vec![1, 0]);
    assert_ne!(digest(|e| e.theory(&left)), digest(|e| e.theory(&right)));
}

fn location(source: u32, start: u32, end: u32) -> Location {
    Location {
        source: SourceId::new(source),
        span: Span::new(ByteOffset::new(start), ByteOffset::new(end)).unwrap(),
    }
}

#[test]
fn provenance_fields_change_the_fingerprint() {
    let mut seen = std::collections::BTreeSet::new();
    for span in [
        location(1, 2, 4),
        location(0, 2, 4),
        location(1, 1, 4),
        location(1, 2, 5),
    ] {
        assert!(seen.insert(digest(|e| e.location(span))));
    }
}

#[test]
fn provenance_grouping_changes_the_fingerprint() {
    let a = location(1, 2, 3);
    let b = location(1, 4, 5);
    assert_ne!(
        digest(|e| e.origins(&[vec![a, b], vec![]])),
        digest(|e| e.origins(&[vec![a], vec![b]]))
    );
}

#[test]
fn empty_selection_differs_from_implicit_all_atoms() {
    assert_ne!(
        digest(|e| e.metadata(&SourceMetadata::default())),
        digest(|e| e.metadata(&SourceMetadata::for_atoms(AtomSelection::none())))
    );
}

#[test]
fn projection_domains_change_subject_evidence() {
    let admit = |condition: &str| {
        zetesis_themelios::admit_formula(
            format!("{{p}}. #project p:{condition}."),
            zetesis_themelios::AdmissionOptions::default(),
            zetesis_themelios::ExpansionLimits::default(),
            zetesis_themelios::FormulaLimits::default(),
        )
        .unwrap()
    };
    let enabled = admit("1=1");
    let disabled = admit("1=2");
    // Same rule data and directive layout; only the completed domain differs.
    assert_eq!(enabled.atoms(), disabled.atoms());
    assert_eq!(enabled.theory().nodes(), disabled.theory().nodes());
    assert_eq!(enabled.metadata(), disabled.metadata());
    assert_ne!(
        digest(|e| e.projection(enabled.projection())),
        digest(|e| e.projection(disabled.projection())),
    );
}

#[test]
fn refused_chunk_is_not_partially_hashed() {
    let mut encoding = Encoding::new(3);
    encoding.write(b"abc").unwrap();
    assert!(matches!(
        encoding.write(b"d"),
        Err(Error::Limit {
            resource: "subject_encoding_bytes",
            limit: 3
        })
    ));
    assert_eq!(encoding.bytes, 3);
    assert_eq!(encoding.hash.finalize(), sha2::Sha256::digest(b"abc"));
}

#[test]
fn digest_serialization_retains_leading_zeroes() {
    let record = SubjectFingerprint::Available {
        format: super::FORMAT,
        bytes: 1,
        sha256: [0; 32],
    };
    let json = serde_json::to_value(record).unwrap();
    assert_eq!(json["sha256"], "0".repeat(64));
}

#[test]
fn atom_framing_matches_the_version_one_layout() {
    // p(1): a framed UTF-8 name, arity, sign, value count, numeric tag and i32.
    // This independent fixed byte layout prevents a silent framing revision.
    let mut expected = [0_u8; 55];
    expected[15] = 1;
    expected[16] = b'p';
    expected[32] = 1;
    expected[49] = 1;
    expected[50] = 1;
    expected[54] = 1;
    let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(1)]).unwrap();
    assert_eq!(
        digest(|e| e.atom(&atom)).as_slice(),
        sha2::Sha256::digest(expected).as_slice()
    );
}

#[test]
fn nested_value_kinds_do_not_alias_in_subject_evidence() {
    let mut seen = std::collections::BTreeSet::new();
    for node in [
        ValueNode::Infimum,
        ValueNode::Supremum,
        ValueNode::String("#inf".into()),
        ValueNode::String("#sup".into()),
        ValueNode::String("a".into()),
        ValueNode::Symbol("a".into()),
    ] {
        let value = function(Sign::Positive, "f", vec![node]);
        assert!(seen.insert(digest(|encoding| encoding.value(&value))));
    }
}
