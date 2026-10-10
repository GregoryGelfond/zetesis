//! Closure-local consequences and completed rule scans over exact decision masks.
//!
//! A batch contains only unit consequences proved against one immutable region.
//! It survives only extensions of both masks. A completed rule stays clean
//! while its possible reads are unchanged or affected body occurrences become false.
//! A bounded set of newly held positive atoms can instead restrict its next
//! scan to all their original occurrences. Generated rules decline either change.
//! Pending deltas accumulate until the rule is scanned. A completed productive
//! scan becomes negative evidence only after every emitted unit is confirmed
//! applied and all intervening changes merely disable occurrences.

use super::{ConstraintConsequence, PreparedConstraints, literal_atom, scan_selected_rule};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation, RuleIr};
use crate::formula_support::{Computation, Context, Counters, StorageLease, reserve_exact};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::catalog::PredicateRef;
use zetesis_cpu::regions::Region;

mod wakeups;
use wakeups::Change;

type ReadContext<'a, 'b, 'c> = Context<'a, &'a Computation<'b, 'c>>;

/// Reuse only successfully captured source prefixes. Capture exhausts the
/// unfiltered Indexed join, including scalar-rejected rows. Runtime uses those
/// exact retained rows/postings and the same empty-binding plan and binding
/// schedule; its region filter only removes rows. For these expression nodes,
/// both lanes use Selected coverage: no partial-expression projection or domain
/// generation introduces another prefix. Every constructor intermediate was
/// therefore checked and retained during capture with the same default logical
/// term limits that frozen replay uses. This is a property of this admitted
/// partition, not a claim that arbitrary constructor expressions cannot fail.
///
/// Actual construction, owner, work, storage and cancellation checks stay in
/// place. A rule with sums or differences can additionally use complete source
/// columns to establish defined arithmetic. Every rule must pass: one partial
/// rule retains the original first-result order for the entire partition, since
/// a core consequence can prevent a later exceptional scalar prefix.
pub(super) fn eligible(
    prepared: &mut PreparedConstraints<'_>,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for index in 0..prepared.source.rules.len() {
        let rule = &prepared.source.rules[index];
        if captured_rule(rule, &prepared.limits, counters)? {
            continue;
        }
        if !bounded_rule(rule, &prepared.limits, counters)? {
            return Ok(false);
        }
        // This is the same immutable preparation subsequently borrowed by
        // execution. A declined proof retains the checked first-result route.
        prepared.prepare_rule(index, budget, counters)?;
        if !prepared.plans[index]
            .as_ref()
            .expect("prepared above")
            .has_totality_certificate()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn captured_rule(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        let total = match literal {
            LiteralIr::Atom(..) | LiteralIr::PatternAtom(_) => true,
            LiteralIr::Compare(left, _, right) => {
                total_expression(left, limits, counters, rule.location)?
                    && total_expression(right, limits, counters, rule.location)?
            }
            LiteralIr::TupleCompare(left, _, right) => {
                let mut total = true;
                for value in left.iter().chain(right) {
                    total &= total_expression(value, limits, counters, rule.location)?;
                }
                total
            }
            LiteralIr::Bind { value, .. } => {
                total_expression(value, limits, counters, rule.location)?
            }
            _ => false,
        };
        if !total {
            return Ok(false);
        }
    }
    Ok(true)
}

/// No new binding schedule, constructed expression or partial operator is
/// admitted by the column proof. Ordinary comparisons retain their term order.
fn bounded_rule(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        let expressions = match literal {
            LiteralIr::Atom(..) => continue,
            LiteralIr::Compare(left, _, right) => std::slice::from_ref(left)
                .iter()
                .chain(std::slice::from_ref(right)),
            LiteralIr::TupleCompare(left, _, right) => left.iter().chain(right),
            _ => return Ok(false),
        };
        for expression in expressions {
            for node in &expression.nodes {
                counters.work(limits, rule.location)?;
                if !matches!(
                    node,
                    Operation::Constant(_)
                        | Operation::Variable(_)
                        | Operation::Unary(UnaryOp::Negate, _)
                        | Operation::Binary(BinaryOp::Add | BinaryOp::Sub, _, _)
                ) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn total_expression(
    expression: &Expression,
    limits: &FormulaLimits,
    counters: &mut Counters,
    site: ProgramSite,
) -> Result<bool, FormulaFailure> {
    for node in &expression.nodes {
        counters.work(limits, site)?;
        if !matches!(
            node,
            Operation::Constant(_) | Operation::Variable(_) | Operation::Constructor(_)
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Copy)]
struct Decision {
    atom: usize,
    held: bool,
    site: ProgramSite,
}

/// Bound the inline metadata and the number of anchored traversals per rule.
/// Larger changes use the complete scan; this is an execution tradeoff, not
/// an admission limit. Each atom can still anchor several original occurrences.
const DELTA_ATOMS: usize = 4;

/// Distinct dense atoms in first-observed order. The unused suffix has no
/// meaning. Embedding the array keeps each checker's metadata under its existing
/// storage lease, with no allocation or shared state when a region changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChangedAtoms {
    atoms: [usize; DELTA_ATOMS],
    len: usize,
}

impl ChangedAtoms {
    fn one(atom: usize) -> Self {
        let mut atoms = [0; DELTA_ATOMS];
        atoms[0] = atom;
        Self { atoms, len: 1 }
    }

    fn as_slice(&self) -> &[usize] {
        &self.atoms[..self.len]
    }

    /// A false result declines bounded reuse without changing this set.
    fn include(&mut self, atom: usize) -> bool {
        if self.as_slice().contains(&atom) {
            return true;
        }
        if self.len == self.atoms.len() {
            return false;
        }
        self.atoms[self.len] = atom;
        self.len += 1;
        true
    }
}

/// Evidence from one completed unproductive rule scan. A positive delta
/// retains every relevant newly held atom since that scan, including across
/// passes that drain an earlier rule's units before reaching this rule. Signed
/// disabling changes only remove witnesses and need no anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scan {
    Full,
    Clean,
    PositiveDelta(ChangedAtoms),
}

impl Scan {
    fn include(&mut self, change: Self) {
        *self = match (*self, change) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (prior, Self::Clean) => prior,
            (Self::Clean, delta) => delta,
            (Self::PositiveDelta(mut prior), Self::PositiveDelta(change)) => {
                if change.as_slice().iter().all(|&atom| prior.include(atom)) {
                    Self::PositiveDelta(prior)
                } else {
                    Self::Full
                }
            }
        };
    }
}

/// Only integer coordinates of this core's immutable rule and atom owners.
/// Predicate borrows are construction scratch, never stored beside their owner.
pub(in crate::formula_hybrid) struct Plan {
    dependencies: Vec<usize>,
    offsets: Vec<usize>,
    atom_predicates: Vec<Option<usize>>,
    predicate_count: usize,
}

struct Preparation<'source> {
    lease: StorageLease,
    predicates: Vec<PredicateRef<'source>>,
    dependencies: Vec<usize>,
    offsets: Vec<usize>,
    atom_predicates: Vec<Option<usize>>,
}

pub(in crate::formula_hybrid) struct Incremental<'source> {
    lease: StorageLease,
    plan: &'source Plan,
    changed: Vec<Change>,
    scans: Vec<Scan>,
    held: Vec<u64>,
    cut: Vec<u64>,
    queued: Vec<Option<bool>>,
    pending: Vec<Decision>,
    /// The current batch came from an exhausted rule scan, and every observed
    /// change since that scan merely disables occurrences. The rule stays Full
    /// until every emitted decision, including the delivered prefix, is checked
    /// against the region. Enabling or uncovered changes discard this receipt.
    completed_batch: Option<usize>,
    next: usize,
    valid: bool,
}

// Every buffer belongs to one lease. The common reservation checks replacement
// overlap, actual allocator capacity and accepted copy work before publication.
macro_rules! reserve_field {
    ($state:expr, $field:ident, $count:expr, $context:expr) => {{
        let other =
            $state.lease.bytes() - $state.$field.capacity() * size_of_val_type(&$state.$field);
        reserve_exact(
            &mut $state.$field,
            $count,
            &mut $state.lease,
            other,
            $context,
        )?;
    }};
}
fn size_of_val_type<T>(_: &[T]) -> usize {
    size_of::<T>()
}

impl Plan {
    fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.dependencies.capacity() as u128 * size_of::<usize>() as u128
            + self.offsets.capacity() as u128 * size_of::<usize>() as u128
            + self.atom_predicates.capacity() as u128 * size_of::<Option<usize>>() as u128
    }

    fn new(
        prepared: &PreparedConstraints<'_>,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let site = prepared.source.location;
        let limits = &prepared.limits;
        prepared.completed.admit_workspace(
            size_of::<Preparation<'_>>() as u128,
            limits,
            counters,
            site,
        )?;
        let mut preparation = Preparation {
            lease: counters.lease(),
            predicates: Vec::new(),
            dependencies: Vec::new(),
            offsets: Vec::new(),
            atom_predicates: Vec::new(),
        };
        preparation
            .lease
            .observe(size_of::<Preparation<'_>>(), site)?;
        let queries =
            prepared
                .completed
                .queries(crate::JoinStrategy::Indexed, limits, counters, site)?;
        let computation = queries.computation(site)?;
        preparation.prepare_dependencies(prepared, &computation, counters)?;
        preparation.prepare_atoms(prepared, &computation, counters)?;
        // No scratch lease enters the core. Every actual vector capacity was
        // admitted above; the borrowing checker next accounts the shared plan
        // as retained support, just as for the catalog index and row positions.
        let predicate_count = preparation.predicates.len();
        Ok(Self {
            dependencies: preparation.dependencies,
            offsets: preparation.offsets,
            atom_predicates: preparation.atom_predicates,
            predicate_count,
        })
    }
}

impl<'source> PreparedConstraints<'source> {
    fn prepare_incremental_plan(
        &mut self,
        counters: &mut Counters,
    ) -> Result<&'source Plan, FormulaFailure> {
        if let Some(plan) = self.incremental_plan {
            return Ok(plan);
        }
        let site = self.source.location;
        counters.work(&self.limits, site)?;
        if self.shared_incremental.get().is_none() {
            let plan = Plan::new(self, counters)?;
            // Independent race builds keep their own controls and accepted
            // receipts. A refused build publishes nothing; the loser drops.
            let _ = self.shared_incremental.set(plan);
        }
        let plan = self
            .shared_incremental
            .get()
            .expect("published above or by a racing checker");
        let bytes = plan.retained_bytes();
        self.completed
            .admit_workspace(bytes, &self.limits, counters, site)?;
        self.completed
            .retain_workspace(usize::try_from(bytes).expect("admitted plan bytes fit usize"));
        self.incremental_plan = Some(plan);
        Ok(plan)
    }
}

impl<'source> Preparation<'source> {
    fn prepare_dependencies(
        &mut self,
        prepared: &PreparedConstraints<'source>,
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let site = prepared.source.location;
        let limits = &prepared.limits;
        let mut occurrences = 0_usize;
        for rule in &prepared.source.rules {
            for literal in &rule.body {
                counters.work(limits, rule.location)?;
                if literal_atom(literal).is_some() {
                    occurrences = occurrences.checked_add(1).ok_or(FormulaFailure::Limit {
                        resource: crate::FormulaResource::SupportBytes,
                        observed: usize::MAX as u128 + 1,
                        limit: limits.max_support_bytes as u128,
                        location: site,
                    })?;
                }
            }
        }
        reserve_field!(
            self,
            dependencies,
            occurrences,
            Context::new(computation, limits, counters, site)
        );
        reserve_field!(
            self,
            predicates,
            occurrences,
            Context::new(computation, limits, counters, site)
        );
        reserve_field!(
            self,
            offsets,
            prepared.source.rules.len() + 1,
            Context::new(computation, limits, counters, site)
        );
        for rule in &prepared.source.rules {
            counters.work(limits, rule.location)?;
            self.offsets.push(self.dependencies.len());
            for literal in &rule.body {
                counters.work(limits, rule.location)?;
                let Some((_, pattern)) = literal_atom(literal) else {
                    continue;
                };
                let components = prepared
                    .completed
                    .components()
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let predicate = pattern
                    .get(components, limits, counters, rule.location)?
                    .predicate();
                let mut found = None;
                for (group, prior) in self.predicates.iter().enumerate() {
                    if prior
                        .compare_ref_with(predicate, || counters.work(limits, rule.location))?
                        .is_eq()
                    {
                        found = Some(group);
                        break;
                    }
                }
                let group = if let Some(group) = found {
                    group
                } else {
                    counters.work(limits, rule.location)?;
                    self.predicates.push(predicate);
                    self.predicates.len() - 1
                };
                counters.work(limits, rule.location)?;
                self.dependencies.push(group);
            }
        }
        counters.work(limits, site)?;
        self.offsets.push(self.dependencies.len());
        Ok(())
    }

    fn prepare_atoms(
        &mut self,
        prepared: &PreparedConstraints<'source>,
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let site = prepared.source.location;
        let limits = &prepared.limits;
        let index = prepared.index.expect("prepared before source scan");
        let atoms = index.catalog().atoms().len();
        let lookup = index.lookup();
        reserve_field!(
            self,
            atom_predicates,
            atoms,
            Context::new(computation, limits, counters, site)
        );
        for _ in 0..atoms {
            counters.work(limits, site)?;
            self.atom_predicates.push(None);
        }
        for group in 0..self.predicates.len() {
            counters.work(limits, site)?;
            for row in
                lookup.predicate_with(self.predicates[group], || counters.work(limits, site))?
            {
                counters.work(limits, site)?;
                self.atom_predicates[row.position()] = Some(group);
            }
        }
        Ok(())
    }
}

impl<'source> Incremental<'source> {
    pub(super) fn new(
        prepared: &mut PreparedConstraints<'source>,
        counters: &mut Counters,
        region: &Region,
    ) -> Result<Self, FormulaFailure> {
        let plan = prepared.prepare_incremental_plan(counters)?;
        let site = prepared.source.location;
        let limits = &prepared.limits;
        prepared
            .completed
            .admit_workspace(size_of::<Self>() as u128, limits, counters, site)?;
        let mut state = Self {
            lease: counters.lease(),
            plan,
            changed: Vec::new(),
            scans: Vec::new(),
            held: Vec::new(),
            cut: Vec::new(),
            queued: Vec::new(),
            pending: Vec::new(),
            completed_batch: None,
            next: 0,
            valid: false,
        };
        state.lease.observe(size_of::<Self>(), site)?;
        let queries =
            prepared
                .completed
                .queries(crate::JoinStrategy::Indexed, limits, counters, site)?;
        let computation = queries.computation(site)?;
        state.prepare_state(prepared, &computation, counters, region)?;
        Ok(state)
    }

    fn prepare_state(
        &mut self,
        prepared: &PreparedConstraints<'_>,
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
        region: &Region,
    ) -> Result<(), FormulaFailure> {
        let site = prepared.source.location;
        let limits = &prepared.limits;
        reserve_field!(
            self,
            scans,
            prepared.source.rules.len(),
            Context::new(computation, limits, counters, site)
        );
        for rule in &prepared.source.rules {
            counters.work(limits, rule.location)?;
            self.scans.push(Scan::Full);
        }
        reserve_field!(
            self,
            queued,
            region.len(),
            Context::new(computation, limits, counters, site)
        );
        for _ in 0..region.len() {
            counters.work(limits, site)?;
            self.queued.push(None);
        }
        reserve_field!(
            self,
            changed,
            self.plan.predicate_count,
            Context::new(computation, limits, counters, site)
        );
        for _ in 0..self.plan.predicate_count {
            counters.work(limits, site)?;
            self.changed.push(Change::None);
        }
        let (held, cut) = region.decision_words();
        reserve_field!(
            self,
            held,
            held.len(),
            Context::new(computation, limits, counters, site)
        );
        reserve_field!(
            self,
            cut,
            cut.len(),
            Context::new(computation, limits, counters, site)
        );
        for _ in held {
            counters.work(limits, site)?;
            self.held.push(0);
            self.cut.push(0);
        }
        Ok(())
    }

    /// O(1) invalidation. Pending entries remain only as cleanup records; the
    /// next authenticated pass retires them before it can deliver a consequence.
    /// Retained capacities remain in this worker's sole lease.
    pub(in crate::formula_hybrid) fn reset(&mut self) {
        self.valid = false;
        self.completed_batch = None;
    }

    /// Only undelivered entries can still occupy a deduplication slot. Advancing
    /// after each charged write preserves cleanup progress if work is refused.
    fn retire_pending(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        self.completed_batch = None;
        while let Some(decision) = self.pending.get(self.next) {
            counters.work(limits, decision.site)?;
            self.queued[decision.atom] = None;
            self.next += 1;
        }
        self.pending.clear();
        self.next = 0;
        Ok(())
    }

    fn synchronize(
        &mut self,
        prepared: &PreparedConstraints<'_>,
        region: &Region,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let limits = &prepared.limits;
        let site = prepared.source.location;
        let (held, cut) = region.decision_words();
        let mut monotone = self.valid;
        for ((&old_held, &old_cut), (&held, &cut)) in
            self.held.iter().zip(&self.cut).zip(held.iter().zip(cut))
        {
            counters.work(limits, site)?;
            monotone &= old_held & !held == 0 && old_cut & !cut == 0;
        }
        if !monotone {
            self.reset();
            for flag in &mut self.scans {
                counters.work(limits, site)?;
                *flag = Scan::Full;
            }
            self.retire_pending(limits, counters)?;
        }
        for changed in &mut self.changed {
            counters.work(limits, site)?;
            *changed = Change::None;
        }
        for (word, (&held, &cut)) in held.iter().zip(cut).enumerate() {
            counters.work(limits, site)?;
            let mut changed = (held ^ self.held[word]) | (cut ^ self.cut[word]);
            if monotone {
                while changed != 0 {
                    counters.work(limits, site)?;
                    let atom = word * u64::BITS as usize + changed.trailing_zeros() as usize;
                    if let Some(group) = self.plan.atom_predicates[atom] {
                        self.changed[group].include(atom);
                    }
                    changed &= changed - 1;
                }
            }
            self.held[word] = held;
            self.cut[word] = cut;
        }
        if monotone {
            for (rule, scan) in self.scans.iter_mut().enumerate() {
                counters.work(limits, site)?;
                if *scan != Scan::Full || self.completed_batch == Some(rule) {
                    let change = wakeups::classify(
                        &prepared.source.rules[rule],
                        &self.plan.dependencies
                            [self.plan.offsets[rule]..self.plan.offsets[rule + 1]],
                        &self.changed,
                        prepared,
                        region,
                        counters,
                    )?;
                    // A productive receipt does not justify anchored scans:
                    // every old unit must first be discharged. Retain it only
                    // across changes that cannot introduce another witness.
                    if self.completed_batch == Some(rule) && change != Scan::Clean {
                        self.completed_batch = None;
                    }
                    scan.include(change);
                }
            }
        }
        self.valid = true;
        Ok(())
    }

    fn pop(
        &mut self,
        region: &Region,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<ConstraintConsequence>, FormulaFailure> {
        debug_assert!(
            self.valid,
            "pending consequences need an authenticated region"
        );
        while let Some(&decision) = self.pending.get(self.next) {
            counters.work(limits, decision.site)?;
            self.next += 1;
            self.queued[decision.atom] = None;
            match region.decision(decision.atom) {
                Some(held) if held == decision.held => {}
                Some(_) => {
                    return Ok(Some(ConstraintConsequence::Refuted {
                        site: decision.site,
                    }));
                }
                None => {
                    return Ok(Some(if decision.held {
                        ConstraintConsequence::Hold {
                            atom: decision.atom,
                            site: decision.site,
                        }
                    } else {
                        ConstraintConsequence::Cut {
                            atom: decision.atom,
                            site: decision.site,
                        }
                    }));
                }
            }
        }
        if let Some(rule) = self.completed_batch.take() {
            let mut applied = true;
            for decision in &self.pending {
                counters.work(limits, decision.site)?;
                applied &= region.decision(decision.atom) == Some(decision.held);
            }
            // Delivery alone proves nothing about the caller's next region.
            // Authenticate the delivered prefix as well as skipped units before
            // replacing a full scan with completed negative evidence. A failed
            // check leaves Full, so ignored or delayed units are rediscovered.
            if applied {
                self.scans[rule] = Scan::Clean;
            }
        }
        self.pending.clear();
        self.next = 0;
        Ok(None)
    }

    fn record(
        &mut self,
        consequence: ConstraintConsequence,
        context: ReadContext<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let (atom, held, site) = match consequence {
            ConstraintConsequence::Hold { atom, site } => (atom, true, site),
            ConstraintConsequence::Cut { atom, site } => (atom, false, site),
            ConstraintConsequence::Refuted { .. } => return Ok(true),
            ConstraintConsequence::NoConsequence => return Ok(false),
        };
        let Context { computation, work } = context;
        work.counters.work(work.limits, site)?;
        if let Some(prior) = self.queued[atom] {
            // Opposite units prove emptiness, but keeping both would require a
            // second slot. Finish this rule with an explicit refutation below.
            return Ok(prior != held);
        }
        let other = self.lease.bytes() - self.pending.capacity() * size_of::<Decision>();
        let required = if self.pending.len() == self.pending.capacity() {
            self.pending
                .capacity()
                .saturating_mul(2)
                .max(1)
                .min(self.queued.len())
        } else {
            self.pending.len() + 1
        };
        let additional = required - self.pending.len();
        reserve_exact(
            &mut self.pending,
            additional,
            &mut self.lease,
            other,
            Context::new(computation, work.limits, work.counters, site),
        )?;
        work.counters.work(work.limits, site)?;
        self.pending.push(Decision { atom, held, site });
        self.queued[atom] = Some(held);
        Ok(false)
    }

    pub(super) fn scan(
        &mut self,
        prepared: &mut PreparedConstraints<'source>,
        budget: &mut Budget,
        counters: &mut Counters,
        region: &Region,
    ) -> Result<ConstraintConsequence, FormulaFailure> {
        self.synchronize(prepared, region, counters)?;
        if let Some(consequence) = self.pop(region, &prepared.limits, counters)? {
            return Ok(consequence);
        }
        for rule in 0..prepared.source.rules.len() {
            counters.work(&prepared.limits, prepared.source.rules[rule].location)?;
            if self.scans[rule] == Scan::Clean {
                continue;
            }
            let atoms = match self.scans[rule] {
                Scan::PositiveDelta(atoms) => Some(atoms),
                Scan::Full => None,
                Scan::Clean => unreachable!("clean rules were skipped"),
            };
            // Completion below must establish new evidence. A refusal, panic
            // or refutation cannot retain the old premise; a productive scan
            // additionally needs its emitted units to be applied.
            counters.work(&prepared.limits, prepared.source.rules[rule].location)?;
            self.scans[rule] = Scan::Full;
            let consequence = scan_selected_rule(
                prepared,
                budget,
                counters,
                region,
                rule,
                atoms.as_ref().map(ChangedAtoms::as_slice),
                |consequence, context| self.record(consequence, context),
            )?;
            if consequence != ConstraintConsequence::NoConsequence {
                // A true body or incompatible proved units both refute this
                // immutable region. No partly scanned rule is marked clean.
                let site = match consequence {
                    ConstraintConsequence::Hold { site, .. }
                    | ConstraintConsequence::Cut { site, .. }
                    | ConstraintConsequence::Refuted { site } => site,
                    ConstraintConsequence::NoConsequence => unreachable!(),
                };
                return Ok(ConstraintConsequence::Refuted { site });
            }
            counters.work(&prepared.limits, prepared.source.rules[rule].location)?;
            // A completed batch identifies every unit of this immutable scan.
            // It is only a conditional receipt: keep Full until all its units
            // are applied and every intervening read change is disabling.
            if self.pending.is_empty() {
                self.scans[rule] = Scan::Clean;
            } else {
                self.completed_batch = Some(rule);
            }
            if let Some(consequence) = self.pop(region, &prepared.limits, counters)? {
                return Ok(consequence);
            }
        }
        Ok(ConstraintConsequence::NoConsequence)
    }
}

#[cfg(test)]
mod tests;
