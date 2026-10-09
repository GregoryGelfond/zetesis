//! Sufficient original-constraint consequences over immutable candidate bounds.
//!
//! A complete scalar-passing body with one open occurrence forces that occurrence
//! false. Query modes select held positive inputs, with at most one designated
//! open positive occurrence. This is optional narrowing; final model acceptance
//! still exhausts the original constraints and reduct membership stays separate.

mod incremental;
#[cfg(test)]
mod pattern_rows_tests;
#[cfg(test)]
mod pivot_tests;
#[cfg(test)]
mod shared_tests;
pub(super) use incremental::{Incremental, Plan as IncrementalPlan};

use super::selection::{ConsequenceMode, ConsequenceSelection};
use super::{
    Candidate, ConstraintCheckCause, ConstraintCheckFailure, ConstraintChecker,
    PreparedConstraints, literal_atom,
};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::formula_support::{Context, Counters, GroundingWork, PositiveRows};
use crate::{FormulaFailure, ProgramSite};
use themelios_program::program::DefaultNegation;
use zetesis_core::AtomLookup;
use zetesis_cpu::{Cancellation, regions::Region};
use zetesis_ferraris::Theory;

/// A candidate closure gets one allowance across all its source passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintRegionPass {
    /// Start another candidate closure. The first includes checker preparation.
    First,
    /// Continue the active closure without renewing any resource ceiling.
    Continue,
}

/// One sufficient consequence of an admitted original constraint instance.
/// None of these results establishes answer-set membership or supports an atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintConsequence {
    /// No supported unit instance was found. This does not certify satisfaction.
    NoConsequence,
    /// An admitted body is true throughout the supplied region.
    Refuted {
        /// Original enclosing rule and any actual source coordinate.
        site: ProgramSite,
    },
    /// Every original model in the supplied region contains this open atom.
    Hold {
        /// Dense coordinate in the exact authenticated core theory.
        atom: usize,
        /// Original enclosing rule and any actual source coordinate.
        site: ProgramSite,
    },
    /// Every original model in the supplied region omits this open atom.
    Cut {
        /// Dense coordinate in the exact authenticated core theory.
        atom: usize,
        /// Original enclosing rule and any actual source coordinate.
        site: ProgramSite,
    },
}

impl ConstraintChecker<'_> {
    /// Find one original-constraint consequence without mutating the region.
    /// The exact core theory and its catalog dimension are authenticated on
    /// every call. Use only original candidate bounds, never a frozen reduct.
    /// Apply a returned decision only to the region supplied or a narrowing of
    /// it; it is not a permanent restriction on unrelated candidate regions.
    ///
    /// `First` starts one closure allowance, including preparation on the first
    /// use. All `Continue` passes share its work, substitution and scalar-byte
    /// ceilings. Each call settles accepted receipts on return, refusal or unwind;
    /// settlement does not renew those ceilings. Hold/Cut leaves the closure
    /// active. `NoConsequence`, `Refuted` or a returned error closes it. Another `First`
    /// or ordinary model/region check also retires an unfinished closure.
    /// A successful `NoConsequence` retains only completed rule-scan evidence:
    /// the next `First` authenticates monotone masks and rechecks rules whose
    /// signed predicates changed. This reuse never carries a pending decision
    /// across closures or renews an active closure's allowance.
    ///
    /// Total nongenerated rules use one join permitting at most one open
    /// positive occurrence. Other rules use at most m+1 joins for their m
    /// positive occurrences: all held, then each occurrence open with the others
    /// held. Prepared plans and complete source relations are reused.
    /// Repeated/aliased open occurrences may yield
    /// no consequence; full original model checking remains mandatory. No ground
    /// instances are retained. For total source partitions, one complete rule
    /// can produce a bounded batch, reused only under authenticated monotone
    /// decision masks. Fully scanned rules are revisited when their signed
    /// predicates change. Partial arithmetic keeps the single-result scan.
    /// Retained metadata and batch growth share the support-byte allowance;
    /// all passes still share one closure budget. Resource-limited prefixes may
    /// differ because a batch finishes its rule before publishing decisions.
    ///
    /// # Errors
    /// An inactive continuation, wrong theory/dimension, cancellation/deadline,
    /// or a checked index/source refusal. Partial work proves no completion.
    pub fn consequence_region(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: ConstraintRegionPass,
    ) -> Result<ConstraintConsequence, ConstraintCheckFailure> {
        match pass {
            ConstraintRegionPass::First => {
                if self.consequence_active {
                    self.finish_check();
                }
                self.consequence_active = true;
            }
            ConstraintRegionPass::Continue if !self.consequence_active => {
                self.invalidate_consequences();
                return Err(ConstraintCheckFailure {
                    cause: ConstraintCheckCause::NoActiveRegion,
                    statistics: self.statistics(),
                });
            }
            ConstraintRegionPass::Continue => {}
        }
        let start = self.prepare_pass(cancellation);
        let result = self
            .authenticate(Candidate::Region(theory, region))
            .and_then(|()| {
                cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
                let consequence = self
                    .accounting
                    .with_cancellation(cancellation, |counters| {
                        if let Some(prepared) = &mut self.prepared {
                            prepared.prepare_selection(self.owner, counters)?;
                        }
                        self.budget
                            .with_settled_receipt(|budget| {
                                scan(self.prepared.as_mut(), budget, counters, region)
                            })
                            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))
                    })?;
                cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
                Ok(consequence)
            });
        let statistics = self.statistics();
        match result {
            Ok(ConstraintConsequence::NoConsequence) => self.settle_check(),
            Ok(ConstraintConsequence::Hold { .. } | ConstraintConsequence::Cut { .. }) => {}
            Ok(ConstraintConsequence::Refuted { .. }) | Err(_) => self.finish_check(),
        }
        result.map_err(|cause| ConstraintCheckFailure {
            cause: cause.relative_to(start).retain_input(&self.owner.0.source),
            statistics,
        })
    }
}

fn scan(
    prepared: Option<&mut PreparedConstraints<'_>>,
    budget: &mut Budget,
    counters: &mut Counters,
    region: &Region,
) -> Result<ConstraintConsequence, FormulaFailure> {
    let Some(prepared) = prepared else {
        return Ok(ConstraintConsequence::NoConsequence);
    };
    let eligible = if let Some(eligible) = prepared.incremental_eligible {
        eligible
    } else {
        let eligible = incremental::eligible(prepared, counters)?;
        prepared.incremental_eligible = Some(eligible);
        eligible
    };
    if eligible {
        // Taking the owner makes unwinding discard every partial scan receipt.
        // Accounting guards still settle work and scalar charges independently.
        let mut state = match prepared.incremental.take() {
            Some(state) => state,
            None => Incremental::new(prepared, counters, region)?,
        };
        let result = state.scan(prepared, budget, counters, region);
        if result.is_err() {
            state.reset();
        }
        prepared.incremental = Some(state);
        return result;
    }
    for rule_index in 0..prepared.source.rules.len() {
        let consequence = scan_rule(prepared, budget, counters, region, rule_index, |_, _| {
            Ok(true)
        })?;
        if consequence != ConstraintConsequence::NoConsequence {
            return Ok(consequence);
        }
    }
    Ok(ConstraintConsequence::NoConsequence)
}

fn scan_rule(
    prepared: &mut PreparedConstraints<'_>,
    budget: &mut Budget,
    counters: &mut Counters,
    region: &Region,
    rule_index: usize,
    mut visit: impl FnMut(
        ConstraintConsequence,
        Context<'_, &crate::formula_support::Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure>,
) -> Result<ConstraintConsequence, FormulaFailure> {
    prepared.prepare_predicates(rule_index, counters)?;
    let rule = &prepared.source.rules[rule_index];
    // One-open traversal changes instance order, so keep the established
    // multi-mode route for partial source arithmetic and generated frames.
    let mut shared = prepared.incremental_eligible == Some(true);
    if shared {
        for literal in &rule.body {
            counters.work(&prepared.limits, rule.location)?;
            if crate::formula_binding_cursor::target(literal).is_some() {
                shared = false;
                break;
            }
        }
    }
    if shared {
        let mode = ConsequenceSelection::shared(
            prepared.selection(rule_index, region),
            rule,
            &prepared.completed,
            &prepared.limits,
            counters,
        )?
        .map(|selection| selection.mode);
        return match mode {
            Some(mode) => scan_mode(
                prepared, budget, counters, region, rule_index, mode, &mut visit,
            ),
            None => Ok(ConstraintConsequence::NoConsequence),
        };
    }
    for pivot in std::iter::once(None).chain((0..rule.body.len()).map(Some)) {
        counters.work(&prepared.limits, rule.location)?;
        if let Some(occurrence) = pivot
            && !matches!(
                literal_atom(&rule.body[occurrence]),
                Some((DefaultNegation::None, _))
            )
        {
            continue;
        }
        let mode = pivot.map_or(ConsequenceMode::Held, ConsequenceMode::Pivot);
        if !(ConsequenceSelection {
            selection: prepared.selection(rule_index, region),
            mode,
        })
        .possible(rule, &prepared.completed, &prepared.limits, counters)?
        {
            continue;
        }
        let consequence = scan_mode(
            prepared, budget, counters, region, rule_index, mode, &mut visit,
        )?;
        if consequence != ConstraintConsequence::NoConsequence {
            return Ok(consequence);
        }
    }
    Ok(ConstraintConsequence::NoConsequence)
}

fn scan_mode(
    prepared: &mut PreparedConstraints<'_>,
    budget: &mut Budget,
    counters: &mut Counters,
    region: &Region,
    rule_index: usize,
    mode: ConsequenceMode,
    visit: &mut impl FnMut(
        ConstraintConsequence,
        Context<'_, &crate::formula_support::Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure>,
) -> Result<ConstraintConsequence, FormulaFailure> {
    let rule = &prepared.source.rules[rule_index];
    prepared.prepare_rule(rule_index, budget, counters)?;
    let selection = ConsequenceSelection {
        selection: prepared.selection(rule_index, region),
        mode,
    };
    let queries = prepared.completed.queries(
        crate::JoinStrategy::Indexed,
        &prepared.limits,
        counters,
        rule.location,
    )?;
    let mut computation = queries.computation(rule.location)?;
    let mut join = prepared.plans[rule_index]
        .as_ref()
        .expect("prepared above")
        .rows(
            &queries,
            Some(&selection),
            &computation,
            &prepared.limits,
            budget,
            counters,
        )?;
    while let Some(row) = join.next_row(
        &mut computation,
        &prepared.limits,
        budget,
        counters,
        rule.location,
    )? {
        if row.passes {
            let consequence = body(
                &rule.body,
                &row.values,
                row.positives.as_ref(),
                region,
                prepared
                    .index
                    .expect("prepared before region scan")
                    .lookup(),
                prepared.predicates.get(rule_index).and_then(Option::as_ref),
                Context::new(&computation, &prepared.limits, counters, rule.location),
            )?;
            if consequence != ConstraintConsequence::NoConsequence
                && visit(
                    consequence,
                    Context::new(&computation, &prepared.limits, counters, rule.location),
                )?
            {
                return Ok(consequence);
            }
        }
    }
    Ok(ConstraintConsequence::NoConsequence)
}

/// Exact body truth is read only after scalar completion. Positive-row evidence
/// either discharges all held positives or lends one mapped open identity with
/// all other positives held. Negative occurrences keep their ordinary lookups.
fn body(
    literals: &[LiteralIr],
    binding: &Binding<'_>,
    positives: Option<&PositiveRows<'_>>,
    region: &Region,
    index: AtomLookup<'_, '_>,
    predicates: Option<&super::RulePredicates<'_>>,
    context: Context<'_, &crate::formula_support::Computation<'_, '_>>,
) -> Result<ConstraintConsequence, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let pivot = positives.and_then(|proof| proof.open_in(literals));
    let positives = positives.is_some_and(|proof| proof.covers(literals));
    let mut view = None;
    let mut open = None;
    for (occurrence, literal) in literals.iter().enumerate() {
        let Some((negation, pattern)) = literal_atom(literal) else {
            continue;
        };
        counters.work(limits, location)?;
        if positives && negation == DefaultNegation::None {
            continue;
        }
        let known = if let Some((at, atom)) = pivot
            && negation == DefaultNegation::None
        {
            if occurrence != at {
                continue;
            }
            Some(atom)
        } else {
            None
        };
        let atom = if let Some(atom) = known {
            atom
        } else {
            let binding_view = if let Some(view) = view {
                view
            } else {
                let checked = binding.view(computation.read(), limits, counters, location)?;
                view = Some(checked);
                checked
            };
            let key = computation
                .static_pattern(*pattern, limits, counters, location)?
                .key(binding_view)
                .map_err(|error| FormulaFailure::UnsafeVariable {
                    variable: error.variable,
                    location,
                })?;
            let row = if let Some(prepared) =
                predicates.and_then(|prepared| prepared.at(literals, occurrence))
            {
                prepared.get_key_with(&key, || counters.work(limits, location))?
            } else {
                index.get_key_with(&key, || counters.work(limits, location))?
            };
            row.ok_or(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            })?
            .position()
        };
        let required = negation != DefaultNegation::Not;
        match region.decision(atom) {
            Some(held) if held != required => return Ok(ConstraintConsequence::NoConsequence),
            Some(_) => {}
            None if open.is_some() => return Ok(ConstraintConsequence::NoConsequence),
            None => open = Some((atom, required)),
        }
    }
    Ok(match open {
        None => ConstraintConsequence::Refuted { site: location },
        Some((atom, true)) => ConstraintConsequence::Cut {
            atom,
            site: location,
        },
        Some((atom, false)) => ConstraintConsequence::Hold {
            atom,
            site: location,
        },
    })
}
