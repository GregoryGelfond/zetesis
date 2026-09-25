//! Authoritative family evidence over completed support, before formula emission.
//!
//! Comparison/domain pruning and support delta shortcuts are deliberately absent
//! here. A complete false row still witnesses defined arithmetic. Each local
//! source occurrence is finalized for its fixed outer binding, after combining
//! its normalized alternatives. Top-level fragments with the same original
//! source span likewise merge their evidence.
//! Work stacks borrow the finite compiled scope tree. Their live frontier is
//! bounded by its nesting and alternatives, not by the total substitutions
//! visited. Each push spends grounding work and reserves fallibly; transient
//! frame cells do not consume the cumulative scalar-payload allowance.

use crate::formula_support::{Context, GroundingWork};

use themelios_base::span::Location;

use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_conditional_ir::{Consequent, ConsequentOperand};
use crate::formula_ir::{HeadIr, LiteralIr, Prepared, Projection, RuleIr};
use crate::formula_support::family::{Evidence, Warnings};
use crate::formula_support::{Computation, Counters, Join, Support};
use crate::{FormulaFailure, FormulaLimits};

#[derive(Clone, Copy)]
enum Continue<'a> {
    None,
    Head(&'a HeadIr),
    Consequent(&'a Consequent),
    Projection(&'a Projection),
}

struct Frame<'a, 'source> {
    join: Join<'a, 'source>,
    body: &'a [LiteralIr],
    body_variables: usize,
    continuation: Continue<'a>,
    location: Location,
    root: bool,
}

/// A group marker precedes its contiguous source alternatives on the work
/// stack. It owns their evidence until every fragment and nested scope finishes.
struct Task<'a, 'source> {
    frame: Option<Frame<'a, 'source>>,
    evidence: Evidence,
    destination: Option<usize>,
    location: Location,
}

struct Scan<'a, 'source, 'context, 'compute> {
    support: &'a Support<'source>,
    computation: &'context mut Computation<'compute, 'source>,
    limits: &'context FormulaLimits,
    budget: &'context mut Budget,
    counters: &'context mut Counters,
    warnings: &'context mut Warnings,
    stack: Vec<Task<'a, 'source>>,
}

pub(super) fn prepare<'a, 'source>(
    prepared: &'a Prepared,
    support: &'a Support<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Warnings, FormulaFailure> {
    let mut warnings = Warnings::default();
    let mut affected = Vec::new();
    for rule in prepared.rules.iter().chain(&prepared.projection) {
        if contains_partial(rule, limits, counters)? {
            affected
                .try_reserve(1)
                .map_err(|_| allocation(rule.location))?;
            affected.push(rule.location);
        }
    }
    let mut families: Vec<(Location, Evidence)> = Vec::new();
    for rule in prepared.rules.iter().chain(&prepared.projection) {
        counters.charge_work(affected.len() as u128, limits, rule.location)?;
        if !affected.contains(&rule.location) {
            continue;
        }
        let mut scan = Scan {
            support,
            computation,
            limits,
            budget,
            counters,
            warnings: &mut warnings,
            stack: Vec::new(),
        };
        let join = Join::rule(
            rule,
            support,
            scan.computation,
            limits,
            scan.budget,
            scan.counters,
        )?;
        scan.push(Frame {
            join,
            body: &rule.body,
            body_variables: rule.body_variables,
            continuation: Continue::Head(&rule.head),
            location: rule.location,
            root: true,
        })?;
        let evidence = scan.run()?;
        let mut existing = None;
        for (index, (location, _)) in families.iter().enumerate() {
            counters.work(limits, rule.location)?;
            if *location == rule.location {
                existing = Some(index);
                break;
            }
        }
        if let Some(index) = existing {
            families[index].1.merge(evidence);
        } else {
            counters.work(limits, rule.location)?;
            families
                .try_reserve(1)
                .map_err(|_| allocation(rule.location))?;
            families.push((rule.location, evidence));
        }
    }
    for (location, evidence) in families {
        warnings.family(evidence, limits, counters, location)?;
    }
    Ok(warnings)
}

/// Objective rows use the same independent nested-scope evidence before their
/// priority, weight and tuple expressions complete the enclosing family.
pub(super) fn body<'a, 'source>(
    literals: &'a [LiteralIr],
    binding: &Binding,
    support: &'a Support<'source>,
    budget: &mut Budget,
    warnings: &mut Warnings,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<(), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut scan = Scan {
        support,
        computation,
        limits,
        budget,
        counters,
        warnings,
        stack: Vec::new(),
    };
    scan.children(literals, binding, location)?;
    scan.run()?;
    Ok(())
}

impl<'a, 'source> Scan<'a, 'source, '_, '_> {
    fn task(&mut self, task: Task<'a, 'source>) -> Result<usize, FormulaFailure> {
        self.counters.work(self.limits, task.location)?;
        // An outer row's descendants drain before that row advances. Live
        // tasks are its ancestors and pending source siblings, bounded by
        // compiled scope structure; each Join retains its existing frame and
        // query bounds. Repeated rows reuse this transient capacity.
        self.stack
            .try_reserve(1)
            .map_err(|_| allocation(task.location))?;
        let index = self.stack.len();
        self.stack.push(task);
        Ok(index)
    }

    fn group(&mut self, location: Location) -> Result<usize, FormulaFailure> {
        self.task(Task {
            frame: None,
            evidence: Evidence::default(),
            destination: None,
            location,
        })
    }

    fn push(&mut self, frame: Frame<'a, 'source>) -> Result<(), FormulaFailure> {
        self.fragment(frame, None)
    }

    fn fragment(
        &mut self,
        mut frame: Frame<'a, 'source>,
        destination: Option<usize>,
    ) -> Result<(), FormulaFailure> {
        frame.join.evidence();
        let location = frame.location;
        self.task(Task {
            frame: Some(frame),
            evidence: Evidence::default(),
            destination,
            location,
        })?;
        Ok(())
    }

    fn local(
        &mut self,
        body: &'a [LiteralIr],
        binding: &Binding,
        variables: usize,
        continuation: Continue<'a>,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.local_fragment(body, binding, variables, continuation, location, None)
    }

    fn local_fragment(
        &mut self,
        body: &'a [LiteralIr],
        binding: &Binding,
        variables: usize,
        continuation: Continue<'a>,
        location: Location,
        destination: Option<usize>,
    ) -> Result<(), FormulaFailure> {
        let mut join = Join::new(
            body,
            binding,
            variables,
            self.support,
            self.budget,
            Context::new(&*self.computation, self.limits, self.counters, location),
        )?;
        if let Continue::Consequent(Consequent::Guard(guard)) = continuation {
            join.check_guard(guard);
        }
        self.fragment(
            Frame {
                join,
                body,
                body_variables: variables,
                continuation,
                location,
                root: false,
            },
            destination,
        )
    }

    fn run(&mut self) -> Result<Evidence, FormulaFailure> {
        let mut result = Evidence::default();
        while let Some(task) = self.stack.last_mut() {
            let location = task.location;
            let Some(frame) = &mut task.frame else {
                let task = self.stack.pop().expect("completed source family group");
                self.warnings
                    .family(task.evidence, self.limits, self.counters, location)?;
                continue;
            };
            if let Some(row) = frame.join.next_owned_row(
                self.computation,
                self.limits,
                self.budget,
                self.counters,
                location,
            )? {
                let (body, body_variables, continuation) =
                    (frame.body, frame.body_variables, frame.continuation);
                if matches!(
                    continuation,
                    Continue::Consequent(Consequent::Atoms(..) | Consequent::Guards(_))
                ) {
                    // A selected condition is only a prefix of this local
                    // substitution. Its consequent alternatives must supply
                    // the jointly defined witness; a false condition already
                    // excludes the consequent and remains a defined witness.
                    let mut evidence = frame.join.take_family();
                    evidence.defined &= !row.passes;
                    task.evidence.merge(evidence);
                }
                if row.passes {
                    let destination = self.stack.len() - 1;
                    self.children(body, &row.values.prefix(body_variables), location)?;
                    self.continue_with(continuation, &row.values, location, destination)?;
                }
            } else {
                let task = self.stack.pop().expect("current family frame");
                let mut frame = task.frame.expect("completed fragment");
                let mut evidence = task.evidence;
                evidence.merge(frame.join.take_family());
                if frame.root {
                    result.merge(evidence);
                } else if let Some(destination) = task.destination {
                    self.stack[destination].evidence.merge(evidence);
                } else {
                    self.warnings
                        .family(evidence, self.limits, self.counters, location)?;
                }
            }
        }
        Ok(result)
    }

    fn children(
        &mut self,
        body: &'a [LiteralIr],
        binding: &Binding,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        for literals in body.chunk_by(|left, right| {
            matches!(
                (left, right),
                (LiteralIr::Conditional(left), LiteralIr::Conditional(right))
                    if left.family == right.family
            )
        }) {
            self.counters.work(self.limits, location)?;
            match &literals[0] {
                LiteralIr::Aggregate(aggregate) => {
                    for elements in aggregate
                        .elements
                        .chunk_by(|left, right| left.family == right.family)
                    {
                        let group = self.group(location)?;
                        for element in elements {
                            self.local_fragment(
                                &element.condition,
                                binding,
                                element.variables,
                                Continue::None,
                                location,
                                Some(group),
                            )?;
                        }
                    }
                }
                LiteralIr::Conditional(_) => {
                    let group = self.group(location)?;
                    for literal in literals {
                        let LiteralIr::Conditional(conditional) = literal else {
                            unreachable!("contiguous alternatives of one conditional")
                        };
                        self.local_fragment(
                            &conditional.condition,
                            binding,
                            conditional.variables,
                            Continue::Consequent(&conditional.consequent),
                            location,
                            Some(group),
                        )?;
                    }
                }
                LiteralIr::ProjectedAtom(_, projection) => {
                    self.projection(projection, binding, location)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn projection(
        &mut self,
        projection: &'a Projection,
        binding: &Binding,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if let Projection::Witnesses {
            bindings,
            variables,
            inputs,
            ..
        } = projection
        {
            self.local(
                bindings,
                &binding.prefix(*inputs),
                *variables,
                Continue::None,
                location,
            )?;
        }
        Ok(())
    }

    fn continue_with(
        &mut self,
        continuation: Continue<'a>,
        binding: &Binding,
        location: Location,
        destination: usize,
    ) -> Result<(), FormulaFailure> {
        match continuation {
            Continue::None | Continue::Consequent(Consequent::Guard(_)) => {}
            Continue::Projection(projection) => self.projection(projection, binding, location)?,
            Continue::Head(head) => self.head(head, binding, location)?,
            Continue::Consequent(Consequent::Atoms(_, alternatives)) => {
                for alternative in alternatives {
                    let continuation = match &alternative.operand {
                        ConsequentOperand::Projection(projection) => {
                            Continue::Projection(projection)
                        }
                        ConsequentOperand::Atom(_) => Continue::None,
                    };
                    self.local_fragment(
                        &alternative.bindings,
                        binding,
                        alternative.variables,
                        continuation,
                        location,
                        Some(destination),
                    )?;
                }
            }
            Continue::Consequent(Consequent::Guards(alternatives)) => {
                for alternative in alternatives {
                    let mut join = Join::new(
                        &alternative.bindings,
                        binding,
                        alternative.variables,
                        self.support,
                        self.budget,
                        Context::new(&*self.computation, self.limits, self.counters, location),
                    )?;
                    join.check_guard(&alternative.guard);
                    self.fragment(
                        Frame {
                            join,
                            body: &alternative.bindings,
                            body_variables: alternative.variables,
                            continuation: Continue::None,
                            location,
                            root: false,
                        },
                        Some(destination),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn head(
        &mut self,
        head: &'a HeadIr,
        binding: &Binding,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        match head {
            HeadIr::Choice(choice) => {
                for elements in choice
                    .elements
                    .chunk_by(|left, right| left.family == right.family)
                {
                    let group = self.group(location)?;
                    for element in elements {
                        let join = Join::element(
                            element,
                            binding,
                            self.support,
                            self.budget,
                            Context::new(&*self.computation, self.limits, self.counters, location),
                        )?;
                        self.fragment(
                            Frame {
                                join,
                                body: &element.condition,
                                body_variables: element.body_variables,
                                continuation: Continue::None,
                                location,
                                root: false,
                            },
                            Some(group),
                        )?;
                    }
                }
            }
            HeadIr::ConditionalDisjunction { elements, .. } => {
                for elements in elements.chunk_by(|left, right| left.family == right.family) {
                    let group = self.group(location)?;
                    for element in elements {
                        let join = Join::local_head(
                            &element.condition,
                            &binding.prefix(element.outer_variables),
                            element.body_variables..element.variables,
                            self.support,
                            self.budget,
                            Context::new(&*self.computation, self.limits, self.counters, location),
                        )?;
                        self.fragment(
                            Frame {
                                join,
                                body: &element.condition,
                                body_variables: element.body_variables,
                                continuation: Continue::None,
                                location,
                                root: false,
                            },
                            Some(group),
                        )?;
                    }
                }
            }
            HeadIr::Normal(_) | HeadIr::Disjunction(_) => {}
        }
        Ok(())
    }
}

fn allocation(location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Allocation,
        location,
    }
}

#[derive(Clone, Copy)]
enum Syntax<'a> {
    Body(&'a [LiteralIr]),
    Continued(Continue<'a>),
}

fn contains_partial(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let mut syntax = SyntaxScan {
        pending: Vec::new(),
        limits,
        counters,
        location: rule.location,
    };
    syntax.push(Syntax::Body(&rule.body))?;
    syntax.push(Syntax::Continued(Continue::Head(&rule.head)))?;
    while let Some(next) = syntax.pending.pop() {
        if syntax.visit(next)? {
            return Ok(true);
        }
    }
    Ok(false)
}

struct SyntaxScan<'a, 'context> {
    pending: Vec<Syntax<'a>>,
    limits: &'context FormulaLimits,
    counters: &'context mut Counters,
    location: Location,
}

impl<'a> SyntaxScan<'a, '_> {
    fn push(&mut self, syntax: Syntax<'a>) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        // This borrowed traversal frontier holds at most the pending scopes
        // of the acyclic, already-admitted IR tree. It copies no scalar payload
        // and is released at the end of this source-rule inspection.
        self.pending
            .try_reserve(1)
            .map_err(|_| allocation(self.location))?;
        self.pending.push(syntax);
        Ok(())
    }

    fn expression(
        &mut self,
        expression: &crate::formula_ir::Expression,
    ) -> Result<bool, FormulaFailure> {
        self.counters
            .charge_work(expression.nodes.len() as u128, self.limits, self.location)?;
        Ok(crate::formula_support::family::expression(expression))
    }

    fn guard(&mut self, guard: &crate::formula_guard::Guard) -> Result<bool, FormulaFailure> {
        for expression in guard.expressions() {
            if self.expression(expression)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn body(&mut self, body: &'a [LiteralIr]) -> Result<bool, FormulaFailure> {
        for literal in body {
            self.counters.work(self.limits, self.location)?;
            match literal {
                LiteralIr::Compare(left, _, right)
                | LiteralIr::ArgumentCheck {
                    captured: left,
                    value: right,
                }
                | LiteralIr::Range {
                    lower: left,
                    upper: right,
                    ..
                } => {
                    if self.expression(left)? || self.expression(right)? {
                        return Ok(true);
                    }
                }
                LiteralIr::TupleCompare(left, _, right) => {
                    for expression in left.iter().chain(right) {
                        if self.expression(expression)? {
                            return Ok(true);
                        }
                    }
                }
                LiteralIr::Bind { value, .. } => {
                    if self.expression(value)? {
                        return Ok(true);
                    }
                }
                LiteralIr::Guard(guard) => {
                    if self.guard(guard)? {
                        return Ok(true);
                    }
                }
                LiteralIr::Aggregate(aggregate) => {
                    for guard in &aggregate.guards {
                        if self.expression(&guard.bound)? {
                            return Ok(true);
                        }
                    }
                    for element in &aggregate.elements {
                        self.push(Syntax::Body(&element.condition))?;
                    }
                }
                LiteralIr::Conditional(conditional) => {
                    self.push(Syntax::Body(&conditional.condition))?;
                    self.push(Syntax::Continued(Continue::Consequent(
                        &conditional.consequent,
                    )))?;
                }
                LiteralIr::ProjectedAtom(_, projection) => {
                    self.push(Syntax::Continued(Continue::Projection(projection)))?;
                }
                _ => {}
            }
        }
        Ok(false)
    }

    fn visit(&mut self, syntax: Syntax<'a>) -> Result<bool, FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        match syntax {
            Syntax::Body(body) => return self.body(body),
            Syntax::Continued(Continue::Head(HeadIr::Choice(choice))) => {
                for guard in &choice.guards {
                    if self.expression(&guard.bound)? {
                        return Ok(true);
                    }
                }
                for element in &choice.elements {
                    self.push(Syntax::Body(&element.condition))?;
                }
            }
            Syntax::Continued(Continue::Head(HeadIr::ConditionalDisjunction {
                elements, ..
            })) => {
                for element in elements {
                    self.push(Syntax::Body(&element.condition))?;
                }
            }
            Syntax::Continued(Continue::Consequent(Consequent::Guard(guard))) => {
                return self.guard(guard);
            }
            Syntax::Continued(Continue::Consequent(Consequent::Atoms(_, alternatives))) => {
                for alternative in alternatives {
                    self.push(Syntax::Body(&alternative.bindings))?;
                    if let ConsequentOperand::Projection(projection) = &alternative.operand {
                        self.push(Syntax::Continued(Continue::Projection(projection)))?;
                    }
                }
            }
            Syntax::Continued(Continue::Consequent(Consequent::Guards(alternatives))) => {
                for alternative in alternatives {
                    if self.guard(&alternative.guard)? {
                        return Ok(true);
                    }
                    self.push(Syntax::Body(&alternative.bindings))?;
                }
            }
            Syntax::Continued(Continue::Projection(Projection::Witnesses { bindings, .. })) => {
                self.push(Syntax::Body(bindings))?;
            }
            Syntax::Continued(_) => {}
        }
        Ok(false)
    }
}
