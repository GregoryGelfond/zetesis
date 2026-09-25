//! Filter-valid instances over one immutable relation snapshot.
//!
//! The public scan exhausts its supplied relation snapshot without deciding
//! per-world truth or frozen gates. A union can offer an instance, but a consumer
//! must check every antecedent against its own world. The private masked scan
//! narrows that offering to positive bindings with some current-world witness.
//! A callback error or source stop establishes neither coverage contract.

use std::{fmt, iter::Copied, slice};
use zetesis_core::{
    AtomKey, Model, PatternRef, Program, TemplateRef,
    catalog::{AtomRef, TermRef},
};

use super::{Relations, Work, visit, worlds};
use crate::{Cancellation, Stop};

/// Bounds on a source scan, including one offered instance's referenced identity.
#[derive(Clone, Copy, Debug)]
pub struct ScanLimits {
    /// Shared scan work, using the existing lazy join operation units.
    pub max_work: u64,
    /// Maximum atoms in one instance, counting repeated antecedent occurrences.
    pub max_instance_atoms: usize,
    /// Maximum logical key metadata and referenced typed encoding bytes in one
    /// instance, counting repeated occurrences and reserved key metadata. This
    /// is not retained canonical memory; keys borrow payload and catalog imports
    /// have their own bounds.
    pub max_instance_bytes: usize,
    /// Named borrowed-row metadata, assignment/cursor/undo scratch and one
    /// offered instance's actual key capacity, including buffer-growth overlap.
    /// This independent 128 MiB default excludes the borrowed Program/catalog,
    /// world-membership masks, callback storage and allocator/Arc bookkeeping.
    pub max_scan_bytes: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_work: 10_000_000,
            max_instance_atoms: 4096,
            max_instance_bytes: 1024 * 1024,
            max_scan_bytes: 128 * 1024 * 1024,
        }
    }
}

/// One ground source instance borrowed for a consumer callback.
/// Gates remain unevaluated. The instance cannot outlive its template or the
/// immutable binding frame. Only checked key metadata is allocated; the keys
/// borrow all typed payload and never revalidate the binding during iteration.
#[derive(Debug)]
pub struct Instance<'a> {
    head: Option<AtomKey<'a>>,
    atoms: Vec<AtomKey<'a>>,
    ends: [usize; 3],
}

impl<'a> Instance<'a> {
    /// Head key, or absence for a constraint. No atom or value is copied.
    #[must_use]
    pub const fn head(&self) -> Option<AtomKey<'a>> {
        self.head
    }

    /// Positive antecedent keys whose truth must be checked per world.
    #[must_use]
    pub fn positive(&self) -> InstanceAtoms<'_> {
        InstanceAtoms {
            keys: &self.atoms[..self.ends[0]],
        }
    }

    /// Keys required true in the immutable seed.
    #[must_use]
    pub fn gate_true(&self) -> InstanceAtoms<'_> {
        InstanceAtoms {
            keys: &self.atoms[self.ends[0]..self.ends[1]],
        }
    }

    /// Keys required false in the immutable seed.
    #[must_use]
    pub fn gate_false(&self) -> InstanceAtoms<'_> {
        InstanceAtoms {
            keys: &self.atoms[self.ends[1]..self.ends[2]],
        }
    }
}

/// Original atom occurrences in one side of a bound source instance.
/// Repeated patterns remain repeated keys; this view owns no tuple payload.
#[derive(Clone, Copy, Debug)]
pub struct InstanceAtoms<'a> {
    keys: &'a [AtomKey<'a>],
}

impl<'a> InstanceAtoms<'a> {
    /// Number of original occurrences.
    #[must_use]
    pub const fn len(self) -> usize {
        self.keys.len()
    }

    /// Whether there are no occurrences.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.keys.is_empty()
    }

    /// Read each already checked key in source order, without allocation or
    /// payload navigation. Consumers charge their own key inspections/imports.
    pub fn iter(self) -> InstanceAtomIter<'a> {
        self.into_iter()
    }
}

impl<'a> IntoIterator for InstanceAtoms<'a> {
    type Item = AtomKey<'a>;
    type IntoIter = InstanceAtomIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.keys.iter().copied()
    }
}

/// Exact double-ended iteration over checked keys in source occurrence order.
pub type InstanceAtomIter<'a> = Copied<slice::Iter<'a, AtomKey<'a>>>;

/// Progress retained for both complete and interrupted scans.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanStatistics {
    /// Charged template, tuple, filter and key-admission work, including any
    /// private world-membership construction/intersections and the lazy batch's
    /// checked identity preparation and callback interning charges.
    pub work: u64,
    /// Fully matched bindings offered to the consumer, including its failure.
    pub bindings: u64,
    /// Membership probes, initialization and intersections charged within work.
    pub mask_words: u64,
    /// Matched positive prefixes with no possible current world.
    /// Their unvisited extensions are not counted as offered instances.
    pub pruned_prefixes: u64,
    /// Peak retained ordered-ID capacity plus requested membership/frame payload
    /// reached by round preparation. Interner storage is bounded separately;
    /// symbolic atoms and other allocator overhead are excluded. This is not RSS.
    pub mask_bytes: usize,
}

/// An incomplete scan, preserving the cause and already charged source work.
#[derive(Debug)]
pub struct ScanFailure<E> {
    /// Source interruption or consumer failure.
    pub cause: ScanCause<E>,
    /// Source work through the interruption.
    pub statistics: ScanStatistics,
}

/// Source and execution failures remain separate from logical rejection.
#[derive(Debug)]
pub enum ScanCause<E> {
    /// A source limit, cancellation, allocation or admitted invariant failed.
    Source(Stop),
    /// The injected consumer failed; snapshot coverage is incomplete.
    Consumer(E),
}

impl<E> From<Stop> for ScanCause<E> {
    fn from(stop: Stop) -> Self {
        Self::Source(stop)
    }
}

impl<E: fmt::Display> fmt::Display for ScanCause<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(stop) => stop.fmt(f),
            Self::Consumer(error) => write!(f, "source instance consumer failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScanCause<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(stop) => Some(stop),
            Self::Consumer(error) => Some(error),
        }
    }
}

impl<E: fmt::Display> fmt::Display for ScanFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScanFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Visit all filter-valid instances over `snapshot`, retaining only a relation
/// index and the current join frame. Offered instances borrow that frame; gates
/// remain symbolic, and the callback cannot retain the instance after returning.
/// The snapshot is borrowed immutably for the whole scan. Repeated source
/// instances remain repeated; a consumer may coalesce consequences.
///
/// A successful return proves exhaustion only of this supplied snapshot, not
/// closure or candidate-stream exhaustion. Source work is bounded by `limits`;
/// consumer work and retained chunk storage need their own bounds. Relations
/// borrow atoms in canonical storage order and share the CPU prefix-window join.
///
/// # Errors
/// Returns the source or callback cause and charged progress. No completion
/// receipt is produced after any failed callback, including the final binding.
pub fn scan<E>(
    program: &Program,
    snapshot: &Model,
    limits: ScanLimits,
    cancellation: &Cancellation,
    mut consume: impl FnMut(Instance<'_>) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut work = Work::source(cancellation, limits.max_work);
    scan_rows(
        program,
        snapshot.atoms().iter(),
        limits,
        &mut work,
        |instance, _| consume(instance),
    )
}

/// Visit a canonical borrowed row selection. The batch's catalog and source
/// visitor share one work owner; callback charges are retained on every exit.
pub(crate) fn scan_rows<'a, E>(
    program: &Program,
    atoms: impl Iterator<Item = AtomRef<'a>>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    mut consume: impl FnMut(Instance<'_>, &mut Work<'_>) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut offered = 0;
    let result = scan_inner(
        program,
        atoms,
        None,
        limits,
        work,
        &mut offered,
        &mut consume,
    );
    let statistics = work.source_statistics(offered);
    result
        .map(|()| statistics)
        .map_err(|cause| ScanFailure { cause, statistics })
}

pub(crate) fn scan_worlds<E>(
    program: &Program,
    snapshot: &mut worlds::Snapshot<'_>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    mut consume: impl FnMut(Instance<'_>, &mut Work<'_>) -> Result<(), E>,
) -> Result<ScanStatistics, ScanFailure<E>> {
    let mut offered = 0;
    let (atoms, mut membership) = snapshot.parts();
    let result = scan_inner(
        program,
        atoms,
        Some(&mut membership),
        limits,
        work,
        &mut offered,
        &mut consume,
    );
    let statistics = work.source_statistics(offered);
    result
        .map(|()| statistics)
        .map_err(|cause| ScanFailure { cause, statistics })
}

fn scan_inner<'a, E>(
    program: &Program,
    atoms: impl Iterator<Item = AtomRef<'a>>,
    mut membership: Option<&mut worlds::Join<'_>>,
    limits: ScanLimits,
    work: &mut Work<'_>,
    offered: &mut u64,
    consume: &mut impl FnMut(Instance<'_>, &mut Work<'_>) -> Result<(), E>,
) -> Result<(), ScanCause<E>> {
    work.cancellation.poll()?;
    let mut relations = Relations::new();
    if relations.retained_bytes() > limits.max_scan_bytes as u128 {
        return Err(Stop::StorageLimit.into());
    }
    let mut peak = 0;
    for atom in atoms {
        relations.push_with(atom, 0, limits.max_scan_bytes, &mut peak, work)?;
    }
    for template in program.templates() {
        work.tick()?;
        visit(
            template,
            &relations,
            super::Gates::Unjudged,
            membership.as_deref_mut(),
            super::QueryStorage {
                retained_bytes: relations.retained_bytes(),
                max_bytes: limits.max_scan_bytes,
            },
            work,
            |assignment, scratch, work| {
                let live = relations
                    .retained_bytes()
                    .checked_add(scratch)
                    .ok_or(Stop::StorageLimit)?;
                let instance = instance(template, assignment, limits, live, work)?;
                *offered += 1;
                consume(instance, work).map_err(ScanCause::Consumer)
            },
        )?;
    }
    work.cancellation.poll()?;
    Ok(())
}

fn instance<'a>(
    template: TemplateRef<'a>,
    assignment: &'a [Option<TermRef<'a>>],
    limits: ScanLimits,
    retained_bytes: u128,
    work: &mut Work<'_>,
) -> Result<Instance<'a>, Stop> {
    let positive = template.positive().len();
    let gate_true = positive
        .checked_add(template.gate_true().len())
        .ok_or(Stop::Allocation)?;
    let count = gate_true
        .checked_add(template.gate_false().len())
        .ok_or(Stop::Allocation)?;
    let occurrences = count
        .checked_add(usize::from(template.head().is_some()))
        .ok_or(Stop::Allocation)?;
    if occurrences > limits.max_instance_atoms {
        return Err(Stop::CarrierLimit);
    }
    let mut remaining_atoms = limits.max_instance_atoms;
    let mut remaining_bytes = limits
        .max_instance_bytes
        .checked_sub(size_of::<Instance<'_>>())
        .ok_or(Stop::Allocation)?;
    let requested = count
        .checked_mul(size_of::<AtomKey<'_>>())
        .ok_or(Stop::Allocation)?;
    if requested > remaining_bytes {
        return Err(Stop::Allocation);
    }
    let headers = retained_bytes
        .checked_add(size_of::<Instance<'_>>() as u128)
        .ok_or(Stop::StorageLimit)?;
    if headers + requested as u128 > limits.max_scan_bytes as u128 {
        return Err(Stop::StorageLimit);
    }
    work.tick()?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    let reserved = atoms
        .capacity()
        .checked_mul(size_of::<AtomKey<'_>>())
        .ok_or(Stop::Allocation)?;
    if reserved > remaining_bytes {
        return Err(Stop::Allocation);
    }
    if headers + reserved as u128 > limits.max_scan_bytes as u128 {
        return Err(Stop::StorageLimit);
    }
    // admit_key counts each occupied key. Charge allocator slack separately so
    // actual temporary metadata remains inside the instance allowance as well.
    remaining_bytes -= reserved - requested;
    let head = template
        .head()
        .map(|pattern| {
            admit_key(
                pattern,
                assignment,
                &mut remaining_atoms,
                &mut remaining_bytes,
                work,
            )
        })
        .transpose()?;
    for pattern in template
        .positive()
        .iter()
        .chain(template.gate_true())
        .chain(template.gate_false())
    {
        let key = admit_key(
            pattern,
            assignment,
            &mut remaining_atoms,
            &mut remaining_bytes,
            work,
        )?;
        work.tick()?;
        atoms.push(key);
    }
    Ok(Instance {
        head,
        atoms,
        ends: [positive, gate_true, count],
    })
}

/// Validate and admit a borrowed occurrence before exposing it to a consumer.
/// The byte measure counts repeated logical identities, not allocated storage.
pub(super) fn admit_key<'a>(
    pattern: PatternRef<'a>,
    assignment: &'a [Option<TermRef<'a>>],
    remaining_atoms: &mut usize,
    remaining_bytes: &mut usize,
    work: &mut Work<'_>,
) -> Result<AtomKey<'a>, Stop> {
    *remaining_atoms = remaining_atoms.checked_sub(1).ok_or(Stop::CarrierLimit)?;
    work.charge(pattern.terms().len())?;
    let key = pattern.key(assignment).map_err(|_| Stop::InvalidProgram)?;
    let mut bytes = size_of::<AtomKey<'_>>()
        .checked_add(pattern.predicate().name().len())
        .ok_or(Stop::Allocation)?;
    for column in 0..key.predicate().arity() {
        work.tick()?;
        let value = key.value(column).ok_or(Stop::InvalidProgram)?;
        let encoding = value.canonical_bytes_with(|| work.tick())?;
        bytes = bytes
            .checked_add(size_of::<TermRef<'_>>())
            .and_then(|n| n.checked_add(encoding))
            .ok_or(Stop::Allocation)?;
    }
    *remaining_bytes = remaining_bytes.checked_sub(bytes).ok_or(Stop::Allocation)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zetesis_core::{
        AdmissionLimits, Atom, AtomPattern, Filter, Predicate, Template, Term, Value,
    };

    fn fixture() -> (Program, Model, Atom, Atom) {
        let value = Value::String("shared payload".into());
        let pattern = |name: &str, terms: Vec<Term>| {
            AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
        };
        let row = pattern("row", vec![Term::Variable(0)]);
        let gate = pattern("gate", vec![Term::Variable(0)]);
        let template = Template::new(
            Some(pattern("head", vec![Term::Variable(0), Term::Variable(0)])),
            vec![row.clone(), row],
            vec![gate.clone()],
            vec![gate],
            vec![Filter::Eq(Term::Variable(0), Term::Constant(value.clone()))],
        );
        let atom = |name: &str, values: Vec<Value>| {
            Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
        };
        let snapshot = Model::new([atom("row", vec![value.clone()])]).unwrap();
        let head = atom("head", vec![value.clone(), value.clone()]);
        let gate = atom("gate", vec![value]);
        (
            Program::new(vec![template], AdmissionLimits::default()).unwrap(),
            snapshot,
            head,
            gate,
        )
    }

    #[test]
    fn borrowed_instances_keep_repeated_occurrences_and_unjudged_gates() {
        let (program, snapshot, expected_head, expected_gate) = fixture();
        let mut offered = 0;
        let statistics = scan(
            &program,
            &snapshot,
            ScanLimits::default(),
            &Cancellation::default(),
            |instance| {
                assert!(instance.head().unwrap().compare(&expected_head).is_eq());
                assert_eq!(instance.positive().len(), 2);
                let mut positive = instance.positive().iter();
                assert_eq!(positive.next(), positive.next_back());
                assert!(positive.next().is_none());
                assert!(
                    instance
                        .gate_true()
                        .iter()
                        .next()
                        .unwrap()
                        .compare(&expected_gate)
                        .is_eq()
                );
                assert!(
                    instance
                        .gate_false()
                        .iter()
                        .next()
                        .unwrap()
                        .compare(&expected_gate)
                        .is_eq()
                );
                offered += 1;
                Ok::<(), Stop>(())
            },
        )
        .unwrap();
        assert_eq!(offered, 1);
        assert_eq!(statistics.bindings, 1);
    }

    #[test]
    fn instance_identity_refusal_precedes_the_consumer() {
        let (program, snapshot, _, _) = fixture();
        for limits in [
            ScanLimits {
                max_instance_atoms: 4,
                ..ScanLimits::default()
            },
            ScanLimits {
                max_instance_bytes: 0,
                ..ScanLimits::default()
            },
        ] {
            let mut offered = 0;
            let failure = scan(
                &program,
                &snapshot,
                limits,
                &Cancellation::default(),
                |_| {
                    offered += 1;
                    Ok::<(), Stop>(())
                },
            )
            .unwrap_err();
            assert_eq!(offered, 0);
            assert_eq!(failure.statistics.bindings, 0);
            assert!(matches!(
                failure.cause,
                ScanCause::Source(Stop::CarrierLimit | Stop::Allocation)
            ));
        }
    }
    #[test]
    fn scan_storage_refusal_precedes_the_consumer() {
        let (program, snapshot, _, _) = fixture();
        let failure = scan(
            &program,
            &snapshot,
            ScanLimits {
                max_scan_bytes: 0,
                ..ScanLimits::default()
            },
            &Cancellation::default(),
            |_| -> Result<(), Stop> { panic!("no instance fits a refused scan") },
        )
        .unwrap_err();
        assert!(matches!(
            failure.cause,
            ScanCause::<Stop>::Source(Stop::StorageLimit)
        ));
        assert_eq!(failure.statistics.bindings, 0);
    }

    #[test]
    fn empty_truth_still_admits_query_scratch() {
        let body = AtomPattern::new(
            Predicate::new("wide", 256).unwrap(),
            vec![Term::Variable(0); 256],
        )
        .unwrap();
        let head =
            AtomPattern::new(Predicate::new("head", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
        let program = Program::new(
            vec![Template::new(
                Some(head),
                vec![body],
                vec![],
                vec![],
                vec![],
            )],
            AdmissionLimits {
                max_predicate_arity: 256,
                ..AdmissionLimits::default()
            },
        )
        .unwrap();
        let failure = scan(
            &program,
            &Model::default(),
            ScanLimits {
                max_scan_bytes: 128,
                max_instance_bytes: usize::MAX,
                ..ScanLimits::default()
            },
            &Cancellation::default(),
            |_| Ok::<(), Stop>(()),
        )
        .unwrap_err();
        assert!(matches!(
            failure.cause,
            ScanCause::Source(Stop::StorageLimit)
        ));
        assert_eq!(failure.statistics.bindings, 0);
        let completed = scan(
            &program,
            &Model::default(),
            ScanLimits::default(),
            &Cancellation::default(),
            |_| Ok::<(), Stop>(()),
        )
        .unwrap();
        assert_eq!(completed.bindings, 0);
    }
}
