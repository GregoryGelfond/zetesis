//! A flat positive join over true rows with one borrowed binding and undo trail.
//!
//! At depth d, slots contain exactly the consistent captures of the first d
//! selected rows. Every level's checkpoint separates its captures from that
//! prefix. The finite row cursors and depth delimit the remaining traversal.

use themelios_program::program::DefaultNegation;
use zetesis_core::{
    AtomRows, BindingView, Model, PatternRef, TemplateComponentsRef, UnificationFailure,
};
use zetesis_core::{atom_interner::AtomInterner, catalog::TermRef};

use super::{ReconstructionError, Work};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{StorageLease, components};

struct Level<'plan, 'model> {
    pattern: PatternRef<'plan>,
    rows: AtomRows<'model, 'model>,
    remaining: AtomRows<'model, 'model>,
    checkpoint: usize,
}

struct Frame<'plan, 'model> {
    slots: Vec<Option<TermRef<'model>>>,
    trail: Vec<usize>,
    levels: Vec<Level<'plan, 'model>>,
    lease: StorageLease,
}

impl<'plan, 'model> Frame<'plan, 'model> {
    fn new(
        rule: &RuleIr,
        components: TemplateComponentsRef<'plan>,
        model: &'model Model,
        writer: &AtomInterner,
        work: &mut Work<'_>,
    ) -> Result<Self, ReconstructionError> {
        let mut frame = Self {
            slots: Vec::new(),
            trail: Vec::new(),
            levels: Vec::new(),
            lease: work.counters.lease(),
        };
        frame.lease.observe(size_of::<Self>(), work.location)?;
        work.observe(writer.storage_bytes())?;
        work.reserve(
            &mut frame.slots,
            rule.variables,
            &mut frame.lease,
            size_of::<Self>(),
            writer,
        )?;
        let slots_bytes = frame.slots.capacity() * size_of::<Option<TermRef<'_>>>();
        work.reserve(
            &mut frame.trail,
            rule.variables,
            &mut frame.lease,
            size_of::<Self>() + slots_bytes,
            writer,
        )?;
        let slots_and_trail = slots_bytes + frame.trail.capacity() * size_of::<usize>();
        work.reserve(
            &mut frame.levels,
            rule.body.len(),
            &mut frame.lease,
            size_of::<Self>() + slots_and_trail,
            writer,
        )?;
        for _ in 0..rule.variables {
            work.permit()?;
            frame.slots.push(None);
        }
        for literal in &rule.body {
            work.permit()?;
            let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                return Err(components::missing(rule.location).into());
            };
            let pattern = pattern.get(components, work.limits, work.counters, rule.location)?;
            let rows = model
                .lookup()
                .predicate_with(pattern.predicate(), || work.permit())?;
            work.permit()?;
            frame.levels.push(Level {
                pattern,
                remaining: rows.clone(),
                rows,
                checkpoint: 0,
            });
        }
        Ok(frame)
    }

    fn undo(&mut self, checkpoint: usize, work: &mut Work<'_>) -> Result<(), ReconstructionError> {
        while self.trail.len() > checkpoint {
            work.permit()?;
            let variable = self.trail.pop().expect("trail is above checkpoint");
            self.slots[variable] = None;
        }
        Ok(())
    }
}

pub(super) fn derive(
    rule: &RuleIr,
    components: TemplateComponentsRef<'_>,
    model: &Model,
    writer: &mut AtomInterner,
    work: &mut Work<'_>,
) -> Result<(), ReconstructionError> {
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Err(components::missing(rule.location).into());
    };
    let head = head.get(components, work.limits, work.counters, rule.location)?;
    let mut frame = Frame::new(rule, components, model, writer, work)?;
    if frame.levels.is_empty() {
        return emit(head, &frame.slots, writer, work);
    }
    let mut depth = 0;
    loop {
        frame.undo(frame.levels[depth].checkpoint, work)?;
        work.permit()?;
        let Some(row) = frame.levels[depth].remaining.next() else {
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            continue;
        };
        let matched = frame.levels[depth]
            .pattern
            .terms()
            .unify_with(
                row.atom().values().iter(),
                &mut frame.slots,
                &mut frame.trail,
                || work.permit(),
            )
            .map_err(|error| match error {
                UnificationFailure::Input(error) => ReconstructionError::Unification(error),
                UnificationFailure::Stopped(error) => error.into(),
            })?;
        if !matched {
            continue;
        }
        if depth + 1 == frame.levels.len() {
            emit(head, &frame.slots, writer, work)?;
        } else {
            depth += 1;
            work.permit()?;
            frame.levels[depth].remaining = frame.levels[depth].rows.clone();
            frame.levels[depth].checkpoint = frame.trail.len();
        }
    }
}

fn emit(
    head: PatternRef<'_>,
    slots: &[Option<TermRef<'_>>],
    writer: &mut AtomInterner,
    work: &mut Work<'_>,
) -> Result<(), ReconstructionError> {
    work.permit()?;
    work.counters.substitution(work.limits, work.location)?;
    // key checks every argument for referenced variable availability. Charge
    // that bounded read before borrowing the ephemeral key; no tuple is copied.
    for _ in 0..head.terms().len() {
        work.permit()?;
    }
    let key = head
        .key(BindingView::from(slots))
        .map_err(ReconstructionError::Head)?;
    let limits = work.limits;
    let location = work.location;
    work.atoms(writer, |writer, bound, counters| {
        writer
            .entry_key_with(key, bound, || counters.work(limits, location))?
            .insert_with(bound, || counters.work(limits, location))
    })?;
    Ok(())
}
