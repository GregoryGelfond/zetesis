//! Where a rule's innermost join may be taken a row at a time.
//!
//! At the innermost depth of a join every other positive occurrence has bound
//! its variables. Suppose the occurrence visited there is over a dense
//! relation, its last argument is a variable nothing else in the rule
//! mentions but the head, in its own last argument, and every other argument
//! of the two patterns is a constant or bound. Then the occurrence's rows
//! are one block of its relation, the heads they derive are one block of the
//! head's relation, and a tuple's place in the first block is its head's
//! place in the second when both relations list that argument's values
//! alike. The join may then mark the block of heads from the block of rows
//! a word at a time instead of binding each row: no gate or filter reads the
//! variable, so every guard was judged before the depth was reached, and each
//! row of the block is one binding of the rule.
//!
//! The plan is a property of the templates and the layouts, fixed at
//! preparation. Which occurrence a round visits innermost depends on the
//! occurrence it pivots on, so the plan answers for every occurrence.

use std::mem::size_of;

use zetesis_core::{AtomPattern, Program, Template, Term};

use super::Work;
use super::relations::Layouts;
use crate::Stop;

/// For each template and positive occurrence, whether the occurrence may be
/// taken a row at a time when the join visits it innermost.
///
/// Retains one flag for each positive occurrence of the program. Planning
/// reads every term of a template once for each of its occurrences, so its
/// work is at most the square of the largest template's terms for each
/// template; a round reads a flag in constant time.
#[derive(Default)]
pub(super) struct RowSteps(Vec<Vec<bool>>);

impl RowSteps {
    /// One unit for each term of each pattern inspected for each occurrence.
    pub(super) fn plan(
        program: &Program,
        layouts: &Layouts,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let mut steps = Vec::new();
        steps
            .try_reserve_exact(program.templates().len())
            .map_err(|_| Stop::Allocation)?;
        for template in program.templates() {
            let mut occurrences = Vec::new();
            occurrences
                .try_reserve_exact(template.positive().len())
                .map_err(|_| Stop::Allocation)?;
            for occurrence in 0..template.positive().len() {
                work.charge(1 + terms(template))?;
                occurrences.push(admits(template, occurrence, layouts));
            }
            steps.push(occurrences);
        }
        Ok(Self(steps))
    }

    pub(super) fn of(&self, template: usize) -> &[bool] {
        self.0.get(template).map_or(&[], Vec::as_slice)
    }

    /// Retained bytes; `None` when the sum does not fit.
    pub(super) fn bytes(&self) -> Option<u128> {
        let flags = self
            .0
            .iter()
            .try_fold(0u128, |sum, plan| sum.checked_add(plan.capacity() as u128))?;
        (self.0.capacity() as u128)
            .checked_mul(size_of::<Vec<bool>>() as u128)?
            .checked_add(flags)
    }
}

fn terms(template: &Template) -> usize {
    template
        .head()
        .into_iter()
        .chain(template.positive())
        .chain(template.gate_true())
        .chain(template.gate_false())
        .map(|pattern| pattern.terms().len())
        .sum::<usize>()
        + 2 * template.filters().len()
}

fn mentions(pattern: &AtomPattern, variable: usize) -> usize {
    pattern
        .terms()
        .iter()
        .filter(|term| **term == Term::Variable(variable))
        .count()
}

/// The condition of the module documentation, clause by clause.
fn admits(template: &Template, occurrence: usize, layouts: &Layouts) -> bool {
    let (Some(head), Some(body)) = (template.head(), template.positive().get(occurrence)) else {
        return false;
    };
    let (Some(&Term::Variable(free)), Some(&Term::Variable(last))) =
        (body.terms().last(), head.terms().last())
    else {
        return false;
    };
    if free != last || mentions(body, free) != 1 || mentions(head, free) != 1 {
        return false;
    }
    let others = || {
        template
            .positive()
            .iter()
            .enumerate()
            .filter(move |(index, _)| *index != occurrence)
            .map(|(_, pattern)| pattern)
    };
    let read_elsewhere = others()
        .chain(template.gate_true())
        .chain(template.gate_false())
        .any(|pattern| mentions(pattern, free) != 0)
        || template.filters().iter().any(|filter| {
            let (left, right) = filter.terms();
            [left, right].contains(&&Term::Variable(free))
        });
    // Every other variable of the occurrence is bound by an earlier depth.
    let bound = body.terms().iter().all(|term| match term {
        Term::Constant(_) => true,
        Term::Variable(variable) => {
            *variable == free || others().any(|pattern| mentions(pattern, *variable) != 0)
        }
    });
    if read_elsewhere || !bound {
        return false;
    }
    match (layouts.get(body.predicate()), layouts.get(head.predicate())) {
        (Some(body), Some(head)) => body.last_values() == head.last_values(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use zetesis_core::{Filter, Predicate, Value};

    use super::*;
    use crate::oracle::argument_bounds::Bound;
    use crate::oracle::relations::Layout;

    fn pattern(name: &str, terms: &[Term]) -> AtomPattern {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms.to_vec()).unwrap()
    }

    fn var(variable: usize) -> Term {
        Term::Variable(variable)
    }

    fn numbers(values: std::ops::RangeInclusive<i32>) -> Bound {
        Bound::Finite(values.map(Value::Number).collect())
    }

    /// `e` and `reach` over 1..=4 by 2..=5, `r` over 0..=5, `q` over 2..=5.
    fn layouts() -> Layouts {
        let mut layouts = Layouts::default();
        for (name, bounds) in [
            ("e", vec![numbers(1..=4), numbers(2..=5)]),
            ("q", vec![numbers(2..=5)]),
            ("r", vec![numbers(0..=5)]),
            ("reach", vec![numbers(1..=4), numbers(2..=5)]),
        ] {
            let predicate = Predicate::new(name, bounds.len()).unwrap();
            layouts.push(Layout::new(&predicate, &bounds, 1 << 10).unwrap());
        }
        layouts
    }

    /// `reach(X,Z) :- reach(X,Y), e(Y,Z).` with X, Y, Z as 0, 1, 2.
    fn transitive() -> Template {
        Template::new(
            Some(pattern("reach", &[var(0), var(2)])),
            vec![
                pattern("reach", &[var(0), var(1)]),
                pattern("e", &[var(1), var(2)]),
            ],
            vec![],
            vec![],
            vec![],
        )
    }

    #[test]
    fn the_transitive_rule_steps_by_rows_of_its_edge_occurrence() {
        assert!(admits(&transitive(), 1, &layouts()));
    }

    #[test]
    fn an_occurrence_whose_last_variable_another_occurrence_binds_does_not_step() {
        // Y, the last argument of `reach(X,Y)`, is bound by `e(Y,Z)`.
        assert!(!admits(&transitive(), 0, &layouts()));
    }

    #[test]
    fn an_occurrence_with_a_free_leading_variable_does_not_step() {
        // `reach(X,Y) :- e(X,Y).`: X is free where the occurrence is visited.
        let copy = Template::new(
            Some(pattern("reach", &[var(0), var(1)])),
            vec![pattern("e", &[var(0), var(1)])],
            vec![],
            vec![],
            vec![],
        );
        assert!(!admits(&copy, 0, &layouts()));
    }

    /// The transitive rule with a false gate and filters added.
    fn guarded(gate_false: Vec<AtomPattern>, filters: Vec<Filter>) -> Template {
        Template::new(
            Some(pattern("reach", &[var(0), var(2)])),
            vec![
                pattern("reach", &[var(0), var(1)]),
                pattern("e", &[var(1), var(2)]),
            ],
            vec![],
            gate_false,
            filters,
        )
    }

    #[test]
    fn a_gate_reading_the_variable_prevents_the_step() {
        let gated = guarded(vec![pattern("q", &[var(2)])], vec![]);
        assert!(!admits(&gated, 1, &layouts()));
    }

    #[test]
    fn a_filter_reading_the_variable_prevents_the_step() {
        let filtered = guarded(vec![], vec![Filter::Neq(var(2), var(0))]);
        assert!(!admits(&filtered, 1, &layouts()));
    }

    #[test]
    fn guards_over_the_bound_variables_leave_the_step() {
        let guarded = guarded(
            vec![pattern("q", &[var(1)])],
            vec![Filter::Neq(var(0), var(1))],
        );
        assert!(admits(&guarded, 1, &layouts()));
    }

    #[test]
    fn a_head_that_does_not_end_in_the_variable_alone_does_not_step() {
        let headed = |head| {
            Template::new(
                Some(head),
                vec![
                    pattern("reach", &[var(0), var(1)]),
                    pattern("e", &[var(1), var(2)]),
                ],
                vec![],
                vec![],
                vec![],
            )
        };
        let layouts = layouts();
        assert!(!admits(
            &headed(pattern("reach", &[var(2), var(0)])),
            1,
            &layouts
        ));
        assert!(!admits(
            &headed(pattern("reach", &[var(2), var(2)])),
            1,
            &layouts
        ));
    }

    /// `<head>(Y) :- r(X), e(X,Y).`, the chain rule when the head is `r`.
    fn chain(head: &str) -> Template {
        Template::new(
            Some(pattern(head, &[var(1)])),
            vec![pattern("r", &[var(0)]), pattern("e", &[var(0), var(1)])],
            vec![],
            vec![],
            vec![],
        )
    }

    #[test]
    fn relations_listing_the_argument_differently_do_not_step() {
        // `r` lists 0..=5 and `e` lists its second argument as 2..=5.
        assert!(!admits(&chain("r"), 1, &layouts()));
    }

    #[test]
    fn a_head_of_another_arity_steps_when_the_lists_agree() {
        // `q` lists 2..=5, as `e` lists its second argument.
        assert!(admits(&chain("q"), 1, &layouts()));
    }

    /// The transitive body under another head, or none.
    fn headed(head: Option<AtomPattern>) -> Template {
        Template::new(
            head,
            vec![
                pattern("reach", &[var(0), var(1)]),
                pattern("e", &[var(1), var(2)]),
            ],
            vec![],
            vec![],
            vec![],
        )
    }

    #[test]
    fn a_head_without_a_layout_does_not_step() {
        let unlaid = headed(Some(pattern("tree", &[var(0), var(2)])));
        assert!(!admits(&unlaid, 1, &layouts()));
    }

    #[test]
    fn a_constraint_does_not_step() {
        assert!(!admits(&headed(None), 1, &layouts()));
    }
}
