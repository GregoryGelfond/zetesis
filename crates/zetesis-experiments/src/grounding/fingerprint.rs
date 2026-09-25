//! Versioned, framed execution-subject evidence outside admission timing.
//!
//! Hashing borrows the admitted subject and uses fixed-size scratch. Every byte
//! is charged before hashing; the inclusive encoding ceiling bounds the expanded
//! subject without retaining it. Canonical preorder navigation can revisit
//! ancestors, so its worst-case work is quadratic in encoded size on deep combs.
//! This is collision-resistant evidence, not an
//! injective mathematical identity or an interchangeable serialized program.

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use themelios_base::span::Location;
use zetesis_core::Sign;
use zetesis_core::catalog::{AtomRef, PredicateRef};
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmittedFormulaBundle, PreparedProjection, SourceDirective, SourceMetadata,
};

use super::Error;

mod value;

/// Why complete cross-executable execution-subject evidence is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintUnavailable {
    /// Observation query plans have no complete public borrowed representation.
    TermObservations,
    /// This encoding version covers only the driver's objective-free domain.
    Objectives,
}

/// A bounded SHA-256 record, or an explicit unsupported evidence domain.
/// This covers ordered execution data, not source analysis caches, allocation
/// capacity, pointer identity or machine code. Original source files have their
/// own separate sealed catalog. Equality retains SHA-256's collision assumption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubjectFingerprint {
    /// Complete supported execution data were framed and hashed.
    Available {
        /// Immutable byte-grammar identifier, independent of the report schema.
        format: &'static str,
        /// Number of framed bytes passed to SHA-256, including the domain prefix.
        bytes: usize,
        /// Digest of those exact bytes, serialized as 64 lowercase hex digits.
        #[serde(serialize_with = "serialize_digest")]
        sha256: [u8; 32],
    },
    /// Internal exact subject comparison still runs; this is not cross-binary evidence.
    Unavailable {
        /// Typed domain restriction; no digest of a truncated subject is emitted.
        reason: FingerprintUnavailable,
    },
}

const FORMAT: &str = "zetesis-execution-subject-v2";

pub(super) fn subject(
    subject: &AdmittedFormulaBundle,
    limit: usize,
) -> Result<SubjectFingerprint, Error> {
    if !subject.metadata().observations().is_empty() {
        return Ok(SubjectFingerprint::Unavailable {
            reason: FingerprintUnavailable::TermObservations,
        });
    }
    if subject.objectives().is_present()
        || !subject.objectives().templates().is_empty()
        || !subject.objective_declarations().is_empty()
        || !subject.objective_origins().is_empty()
    {
        return Ok(SubjectFingerprint::Unavailable {
            reason: FingerprintUnavailable::Objectives,
        });
    }
    let mut encoding = Encoding::new(limit);
    encoding.text(FORMAT)?;
    encoding.count(subject.atoms().len())?;
    for atom in subject.atoms() {
        encoding.atom(atom)?;
    }
    encoding.theory(subject.theory())?;
    encoding.origins(subject.formula_origins())?;
    // All four objective dimensions are checked above, then encoded explicitly.
    encoding.tag(0)?; // objective presence
    encoding.count(0)?; // templates
    encoding.origins(subject.objective_origins())?;
    encoding.locations(subject.objective_declarations())?;
    encoding.metadata(subject.metadata())?;
    encoding.projection(subject.projection())?;
    Ok(SubjectFingerprint::Available {
        format: FORMAT,
        bytes: encoding.bytes,
        sha256: encoding.hash.finalize().into(),
    })
}

struct Encoding {
    hash: Sha256,
    bytes: usize,
    limit: usize,
}
impl Encoding {
    fn new(limit: usize) -> Self {
        Self {
            hash: Sha256::new(),
            bytes: 0,
            limit,
        }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let required = self
            .bytes
            .checked_add(bytes.len())
            .filter(|&required| required <= self.limit)
            .ok_or(Error::Limit {
                resource: "subject_encoding_bytes",
                limit: self.limit,
            })?;
        self.hash.update(bytes);
        self.bytes = required;
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<(), Error> {
        self.write(&[tag])
    }
    fn count(&mut self, count: usize) -> Result<(), Error> {
        self.write(&(count as u128).to_be_bytes())
    }
    fn text(&mut self, text: &str) -> Result<(), Error> {
        self.count(text.len())?;
        self.write(text.as_bytes())
    }
    fn sign(&mut self, sign: Sign) -> Result<(), Error> {
        self.tag(match sign {
            Sign::Positive => 0,
            Sign::Negative => 1,
        })
    }
    fn predicate<'a>(&mut self, predicate: impl Into<PredicateRef<'a>>) -> Result<(), Error> {
        let predicate = predicate.into();
        self.text(predicate.name())?;
        self.count(predicate.arity())?;
        self.sign(predicate.sign())
    }
    fn atom<'a>(&mut self, atom: impl Into<AtomRef<'a>>) -> Result<(), Error> {
        let atom = atom.into();
        self.predicate(atom.predicate())?;
        self.count(atom.values().len())?;
        for value in atom.values() {
            self.value(value)?;
        }
        Ok(())
    }
    fn theory(&mut self, theory: &Theory) -> Result<(), Error> {
        self.count(theory.atom_count())?;
        self.count(theory.nodes().len())?;
        for node in theory.nodes() {
            match *node {
                Node::False => self.tag(0)?,
                Node::Atom(atom) => {
                    self.tag(1)?;
                    self.count(atom)?;
                }
                Node::And(left, right) | Node::Or(left, right) | Node::Implies(left, right) => {
                    self.tag(match node {
                        Node::And(..) => 2,
                        Node::Or(..) => 3,
                        _ => 4,
                    })?;
                    self.count(left)?;
                    self.count(right)?;
                }
            }
        }
        self.count(theory.roots().len())?;
        for &root in theory.roots() {
            self.count(root)?;
        }
        Ok(())
    }
    fn location(&mut self, location: Location) -> Result<(), Error> {
        self.write(&location.source.get().to_be_bytes())?;
        self.write(&location.span.start().get().to_be_bytes())?;
        self.write(&location.span.end().get().to_be_bytes())
    }
    fn locations(&mut self, locations: &[Location]) -> Result<(), Error> {
        self.count(locations.len())?;
        for &location in locations {
            self.location(location)?;
        }
        Ok(())
    }
    fn origins(&mut self, origins: &[Vec<Location>]) -> Result<(), Error> {
        self.count(origins.len())?;
        for locations in origins {
            self.locations(locations)?;
        }
        Ok(())
    }
    fn metadata(&mut self, metadata: &SourceMetadata) -> Result<(), Error> {
        self.count(metadata.directives().len())?;
        for located in metadata.directives().iter() {
            match located.directive() {
                SourceDirective::Defined(predicate) => {
                    self.tag(0)?;
                    self.predicate(predicate)?;
                }
                SourceDirective::ShowSignature(predicate) => {
                    self.tag(1)?;
                    self.predicate(predicate)?;
                }
                SourceDirective::ShowEmpty => self.tag(2)?,
                SourceDirective::ShowTerm => self.tag(3)?,
                SourceDirective::ProjectSignature(predicate) => {
                    self.tag(4)?;
                    self.predicate(predicate)?;
                }
                SourceDirective::ProjectAtom => self.tag(5)?,
            }
            self.location(located.location())?;
        }
        self.tag(u8::from(metadata.atom_selection().is_explicit()))?;
        self.count(metadata.atom_selection().signatures().len())?;
        for predicate in metadata.atom_selection().signatures().iter() {
            self.predicate(predicate)?;
        }
        self.count(0) // complete term-observation plan count, checked before encoding
    }
    fn projection(&mut self, domain: &PreparedProjection) -> Result<(), Error> {
        self.tag(u8::from(domain.is_explicit()))?;
        self.count(domain.atoms().len())?;
        for atom in domain.atoms() {
            self.atom(atom)?;
        }
        Ok(())
    }
}

fn serialize_digest<S: Serializer>(digest: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
    struct Hex<'a>(&'a [u8; 32]);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(output, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    serializer.collect_str(&Hex(digest))
}

#[cfg(test)]
mod tests;
