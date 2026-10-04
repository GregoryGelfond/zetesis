//! Bounded inspection before a logical formula program is copied or rewritten.
//!
//! Structural types have fixed grammar depth. Their borrowed methods below form
//! an acyclic descent; recursive Term/Symbol interiors use the shared iterative
//! suffix cursor. No child list is collected or cloned. Node/text accounting
//! includes provenance entries and annotations, because formula preparation
//! retains that evidence. Unsupported opaque families are refused before any
//! whole-program copy. This inspection does not establish binding safety or
//! arithmetic definedness; the shared formula compiler retains those checks.

use themelios_program::program::{
    Aggregate, Arguments, Atom, Body, BodyElement, Condition, Guard, HasGuards, Head, Literal,
    LiteralInner, Program, Project, SetElement, Show, Statement, Weight,
};
use themelios_program::provenance::{Origin, WithProvenance};
use themelios_program::symbol::Signature;
use themelios_program::term::Term;

use crate::program_limits::{Budget, Limits};
use crate::{
    CompilationFailure, ProfileFeature, ProgramAdmissionFailure, ProgramAdmissionOptions,
    ProgramFailureKind, ProgramSubject,
};

type Check = Result<(), ProgramFailureKind>;

/// Inspect every admitted carrier before the caller starts cloning, rewriting or
/// analysis. A refusal borrows the actual input part or enclosing statement.
pub(crate) fn check(
    program: &Program,
    options: ProgramAdmissionOptions,
) -> Result<(), ProgramAdmissionFailure<'_>> {
    let mut checker = Checker {
        budget: Budget::new(Limits {
            nodes: options.max_nodes,
            depth: options.max_depth,
            body_elements: options.max_body_elements,
            text_bytes: options.max_text_bytes,
        }),
    };
    for part in program.parts() {
        if part.key().name.as_str() != "base" || !part.key().formals.is_empty() {
            return Err(ProgramAdmissionFailure {
                subject: ProgramSubject::Part(part.key()),
                kind: unsupported(ProfileFeature::ProgramPart),
            });
        }
        for carrier in part.statements() {
            checker
                .provenance(carrier, 1)
                .and_then(|()| checker.statement(carrier.get(), 1))
                .map_err(|kind| ProgramAdmissionFailure {
                    subject: ProgramSubject::Statement(carrier),
                    kind,
                })?;
        }
    }
    Ok(())
}

/// Bound canonical objective templates before formula preparation copies them.
/// Source admission separately counts authored syntax before canonical merging.
pub(crate) fn check_objectives(
    program: &Program,
    limits: &crate::FormulaLimits,
) -> Result<(), crate::FormulaFailure> {
    let mut count = 0_u128;
    for (index, carrier) in program.statements().enumerate() {
        let site = crate::extended::origin(
            carrier,
            crate::ProgramSite::statement(crate::StatementId::new(index), None),
        );
        let mut element = || {
            let observed = count.saturating_add(1);
            crate::formula::ceiling(
                crate::FormulaResource::ObjectiveElements,
                observed,
                limits.objective.max_templates as u128,
                site,
            )?;
            count = observed;
            Ok::<(), crate::FormulaFailure>(())
        };
        match carrier.get() {
            Statement::WeakConstraint(_) => element()?,
            Statement::Optimize(optimize) => {
                for _ in optimize.elements() {
                    element()?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn unsupported(feature: ProfileFeature) -> ProgramFailureKind {
    ProgramFailureKind::Compilation(CompilationFailure::Profile(feature))
}

struct Checker {
    budget: Budget,
}

impl Checker {
    fn node(&mut self, depth: usize) -> Check {
        self.budget.node(depth).map_err(ProgramFailureKind::Limit)
    }

    fn text(&mut self, text: &str) -> Check {
        self.budget.text(text).map_err(ProgramFailureKind::Limit)
    }

    fn term(&mut self, term: &Term, depth: usize) -> Check {
        let mut empty_pool = false;
        self.budget
            .inspect_term(term, depth, |node| {
                empty_pool |= matches!(node, Term::Pool(alternatives) if alternatives.is_empty());
            })
            .map_err(ProgramFailureKind::Limit)?;
        if empty_pool {
            return Err(unsupported(ProfileFeature::Term));
        }
        Ok(())
    }

    fn terms<'a>(&mut self, terms: impl Iterator<Item = &'a Term>, depth: usize) -> Check {
        self.node(depth)?;
        for term in terms {
            self.term(term, depth + 1)?;
        }
        Ok(())
    }

    fn provenance<T>(&mut self, carrier: &WithProvenance<T>, depth: usize) -> Check {
        let provenance = carrier.provenance();
        for origin in provenance.origins() {
            self.node(depth + 1)?;
            if let Origin::Transformed(tag) = origin {
                self.text(tag.as_str())?;
            }
        }
        let annotations = provenance.annotations();
        for text in annotations
            .doc()
            .chain(annotations.label())
            .chain(annotations.reference())
            .chain(annotations.trace())
        {
            self.node(depth + 1)?;
            self.text(text)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Statement, depth: usize) -> Check {
        self.node(depth)?;
        match statement {
            Statement::Rule(rule) => {
                self.provenance(rule.head(), depth + 1)?;
                self.head(rule.head().get(), depth + 1)?;
                self.provenance(rule.body(), depth + 1)?;
                self.body(rule.body().get(), depth + 1)
            }
            Statement::WeakConstraint(weak) => {
                self.provenance(weak.body(), depth + 1)?;
                self.body(weak.body().get(), depth + 1)?;
                self.weight(weak.weight(), depth + 1)?;
                self.terms(weak.terms(), depth + 1)
            }
            Statement::Optimize(optimize) => {
                for element in optimize.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    self.weight(element.get().weight(), depth + 2)?;
                    self.terms(element.get().terms(), depth + 2)?;
                    self.condition(element.get().condition(), depth + 2)?;
                }
                Ok(())
            }
            Statement::Const(constant) => {
                self.text(constant.name.as_str())?;
                self.term(&constant.value, depth + 1)
            }
            Statement::Defined(defined) => self.signature(&defined.signature, depth + 1),
            Statement::Show(show) => match show {
                Show::All => Ok(()),
                Show::Signature(signature) => self.signature(signature, depth + 1),
                Show::Term(term) => self.term(term, depth + 1),
                Show::TermBody { term, body } => {
                    self.term(term, depth + 1)?;
                    self.provenance(body, depth + 1)?;
                    self.body(body.get(), depth + 1)
                }
            },
            Statement::Project(project) => match project {
                Project::Signature(signature) => self.signature(signature, depth + 1),
                Project::Atom { atom, body } => {
                    self.provenance(atom, depth + 1)?;
                    self.atom(atom.get(), depth + 1)?;
                    self.provenance(body, depth + 1)?;
                    self.body(body.get(), depth + 1)
                }
            },
            // Includes require a source bundle; script/theory/external/control
            // directives and queries are not in the formula admission profile.
            _ => Err(unsupported(ProfileFeature::Statement)),
        }
    }

    fn head(&mut self, head: &Head, depth: usize) -> Check {
        self.node(depth)?;
        match head {
            Head::Falsum | Head::Verum => Ok(()),
            Head::Literal(literal) => self.literal(literal, depth + 1),
            Head::Disjunction(disjunction) => {
                for element in disjunction.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    self.literal(element.get().literal(), depth + 2)?;
                    self.condition(element.get().condition(), depth + 2)?;
                }
                Ok(())
            }
            Head::Choice(choice) => {
                self.guards(choice.left_guard(), choice.right_guard(), depth + 1)?;
                for element in choice.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    self.literal(element.get().literal(), depth + 2)?;
                    self.condition(element.get().condition(), depth + 2)?;
                }
                Ok(())
            }
            Head::Aggregate(aggregate) => {
                self.guards(aggregate.left_guard(), aggregate.right_guard(), depth + 1)?;
                for element in aggregate.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    self.terms(element.get().terms(), depth + 2)?;
                    self.literal(element.get().literal(), depth + 2)?;
                    self.condition(element.get().condition(), depth + 2)?;
                }
                Ok(())
            }
            Head::TheoryAtom(_) => Err(unsupported(ProfileFeature::Head)),
        }
    }

    fn body(&mut self, body: &Body, depth: usize) -> Check {
        self.node(depth)?;
        for (index, element) in body.elements().enumerate() {
            self.budget
                .body_elements(index.saturating_add(1))
                .map_err(ProgramFailureKind::Limit)?;
            self.node(depth + 1)?;
            self.provenance(element, depth + 1)?;
            match element.get() {
                BodyElement::Literal(literal) => self.literal(literal, depth + 2)?,
                BodyElement::Conditional(conditional) => {
                    self.literal(&conditional.literal, depth + 2)?;
                    self.condition(&conditional.condition, depth + 2)?;
                }
                BodyElement::Aggregate { aggregate, .. } => {
                    self.aggregate(aggregate, depth + 2)?;
                }
                _ => return Err(unsupported(ProfileFeature::BodyElement)),
            }
        }
        Ok(())
    }

    fn condition(&mut self, condition: &Condition, depth: usize) -> Check {
        self.node(depth)?;
        for literal in condition.literals() {
            self.provenance(literal, depth + 1)?;
            self.literal(literal.get(), depth + 1)?;
        }
        Ok(())
    }

    fn literal(&mut self, literal: &Literal, depth: usize) -> Check {
        self.node(depth)?;
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                self.provenance(atom, depth + 1)?;
                self.atom(atom.get(), depth + 1)
            }
            LiteralInner::Comparison(comparison) => {
                self.node(depth + 1)?;
                self.provenance(comparison, depth + 1)?;
                self.term(comparison.get().first(), depth + 2)?;
                for (_, right) in comparison.get().steps() {
                    self.node(depth + 2)?;
                    self.term(right, depth + 3)?;
                }
                Ok(())
            }
            LiteralInner::True | LiteralInner::False => Ok(()),
        }
    }

    fn atom(&mut self, atom: &Atom, depth: usize) -> Check {
        self.node(depth)?;
        self.text(atom.name.as_str())?;
        if matches!(&atom.arguments, Arguments::Pooled(alternatives) if alternatives.is_empty()) {
            return Err(unsupported(ProfileFeature::PooledArguments));
        }
        self.node(depth + 1)?;
        for alternative in atom.alternatives() {
            self.terms(alternative.iter(), depth + 2)?;
        }
        Ok(())
    }

    fn aggregate(&mut self, aggregate: &Aggregate, depth: usize) -> Check {
        self.node(depth)?;
        match aggregate {
            Aggregate::Function(function) => {
                self.guards(function.left_guard(), function.right_guard(), depth + 1)?;
                for element in function.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    self.terms(element.get().terms(), depth + 2)?;
                    self.condition(element.get().condition(), depth + 2)?;
                }
            }
            Aggregate::Set(set) => {
                self.guards(set.left_guard(), set.right_guard(), depth + 1)?;
                for element in set.elements() {
                    self.node(depth + 1)?;
                    self.provenance(element, depth + 1)?;
                    match element.get() {
                        SetElement::Literal(literal) => self.literal(literal, depth + 2)?,
                        SetElement::ConditionalLiteral(conditional) => {
                            self.literal(&conditional.literal, depth + 2)?;
                            self.condition(&conditional.condition, depth + 2)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn guards(
        &mut self,
        left: Option<&WithProvenance<Guard>>,
        right: Option<&WithProvenance<Guard>>,
        depth: usize,
    ) -> Check {
        for guard in [left, right].into_iter().flatten() {
            self.node(depth)?;
            self.provenance(guard, depth)?;
            self.term(&guard.get().term, depth + 1)?;
        }
        Ok(())
    }

    fn weight(&mut self, weight: &Weight, depth: usize) -> Check {
        self.node(depth)?;
        self.term(weight.term(), depth + 1)?;
        if let Some(priority) = weight.priority() {
            self.term(priority, depth + 1)?;
        }
        Ok(())
    }

    fn signature(&mut self, signature: &Signature, depth: usize) -> Check {
        self.node(depth)?;
        self.text(signature.name.as_str())
    }
}

#[cfg(test)]
mod tests;
