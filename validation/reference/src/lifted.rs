//! Bounded lifted reference: source templates compose binding, exact filtering,
//! frozen-seed gating, projection, union, and least closure. No carrier expansion
//! or ground-rule table occurs in `check`. Independent grounding is test-only.
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};

/// Closed-domain scalar identity; text and symbols remain distinct.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Value {
    /// Signed finite integer.
    Int(i32),
    /// Text value whose bytes determine identity.
    Text(String),
    /// Symbol value whose bytes determine identity.
    Symbol(String),
}
/// A template variable or a closed-domain constant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Term {
    /// Variable identifier local to the template.
    Var(usize),
    /// Fixed scalar value.
    Const(Value),
}
/// Predicate application whose arguments may contain template variables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomPattern {
    /// Index in the program's predicate catalog.
    pub predicate: usize,
    /// Ordered predicate arguments.
    pub terms: Vec<Term>,
}
/// Ground predicate application with exact logical scalar identity.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Atom {
    /// Index in the program's predicate catalog.
    pub predicate: usize,
    /// Ordered ground argument values.
    pub tuple: Vec<Value>,
}
/// Equality or disequality applied once both terms are bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Filter {
    /// Require identical logical values.
    Eq(Term, Term),
    /// Require different logical values.
    Neq(Term, Term),
}
/// Safe lifted rule with positive joins and frozen reduct gates.
#[derive(Clone, Debug)]
pub struct Template {
    /// Derived head, or absence for an integrity constraint.
    pub head: Option<AtomPattern>,
    /// Positive body applications supplying all variable bindings.
    pub positive: Vec<AtomPattern>,
    /// Applications required present in the frozen seed.
    pub gate_true: Vec<AtomPattern>,
    /// Applications required absent from the frozen seed.
    pub gate_false: Vec<AtomPattern>,
    /// Logical comparisons applied during binding.
    pub filters: Vec<Filter>,
}
/// Predicate identity within the finite source signature.
#[derive(Clone, Debug)]
pub struct Predicate {
    /// Predicate name; identity includes arity.
    pub name: String,
    /// Number of ordered arguments.
    pub arity: usize,
}
/// Validated templates over a finite closed domain and predicate signature.
#[derive(Clone, Debug)]
pub struct LiftedProgram {
    domain: BTreeSet<Value>,
    predicates: Vec<Predicate>,
    templates: Vec<Template>,
    gate_predicates: BTreeSet<usize>,
}
/// Independent execution ceilings; zero never means unlimited.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum synchronous closure passes, including the final quiescent pass.
    pub max_rounds: usize,
    /// Maximum charged binding entries and tuple probes.
    pub max_work: u64,
    /// Maximum distinct derived atoms retained across relations and the delta.
    pub max_atoms: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rounds: 1000,
            max_work: 1_000_000,
            max_atoms: 100_000,
        }
    }
}
/// Actual inference work, distinct from the symbolic carrier size.
#[derive(Clone, Debug, Default)]
pub struct LiftedStats {
    /// Synchronous closure passes executed.
    pub rounds: usize,
    /// Relation tuples inspected during positive joins.
    pub tuple_probes: u64,
    /// Complete bindings published after their filters and gates held.
    pub complete_bindings: u64,
    /// Partial or complete bindings rejected by a bound filter or gate.
    pub guard_prunes: u64,
    /// Charged binding entries plus tuple probes.
    pub work: u64,
    /// Materialized symbolic-carrier tuples; always zero for this source engine.
    pub carrier_tuples_enumerated: u64,
    /// Retained ground-rule rows; always zero for this source engine.
    pub ground_rows_materialized: u64,
}
/// Complete lifted reduct result with its inference accounting.
#[derive(Clone, Debug)]
pub struct LiftedCheck {
    /// Exact least closure of all enabled positive source instances.
    pub closure: BTreeSet<Atom>,
    /// Whether closure agrees with the seed and violates no active constraint.
    pub stable: bool,
    /// Whether a complete enabled constraint binding was found.
    pub constraint_violated: bool,
    /// Work performed by this complete check.
    pub stats: LiftedStats,
}
type Binding = BTreeMap<usize, Value>;
type Relations = Vec<BTreeSet<Vec<Value>>>;

/// Reference-only capability limit bounding the recursive join stack.
pub const MAX_POSITIVE_BODY: usize = 64;

fn term_value<'a>(t: &'a Term, b: &'a Binding) -> Option<&'a Value> {
    match t {
        Term::Const(v) => Some(v),
        Term::Var(i) => b.get(i),
    }
}
fn instantiate(p: &AtomPattern, b: &Binding) -> Option<Atom> {
    Some(Atom {
        predicate: p.predicate,
        tuple: p
            .terms
            .iter()
            .map(|t| term_value(t, b).cloned())
            .collect::<Option<_>>()?,
    })
}
fn filter_truth(f: &Filter, b: &Binding) -> Option<bool> {
    let (a, c, equal) = match f {
        Filter::Eq(a, c) => (a, c, true),
        Filter::Neq(a, c) => (a, c, false),
    };
    Some((term_value(a, b)? == term_value(c, b)?) == equal)
}
fn charge(stats: &mut LiftedStats, limits: Limits) -> Result<()> {
    stats.work = stats
        .work
        .checked_add(1)
        .ok_or("incomplete: work counter overflow")?;
    if stats.work > limits.max_work {
        return Err("incomplete: lifted work budget".into());
    }
    Ok(())
}

impl LiftedProgram {
    /// Validate and own source templates over the supplied closed domain.
    ///
    /// # Errors
    /// Rejects duplicate predicate identities, excessive positive-body length,
    /// unsafe variables, constants outside the domain, and invalid arities or
    /// predicate indices. Domain duplicates are coalesced by logical identity.
    pub fn new(
        domain: Vec<Value>,
        predicates: Vec<Predicate>,
        templates: Vec<Template>,
    ) -> Result<Self> {
        let domain: BTreeSet<_> = domain.into_iter().collect();
        let mut identities = BTreeSet::new();
        for p in &predicates {
            if !identities.insert((&p.name, p.arity)) {
                return Err("duplicate predicate identity".into());
            }
        }
        let mut gate_predicates = BTreeSet::new();
        for r in &templates {
            if r.positive.len() > MAX_POSITIVE_BODY {
                return Err("reference capability: positive body exceeds 64 atoms".into());
            }
            let bound: BTreeSet<_> = r
                .positive
                .iter()
                .flat_map(|p| &p.terms)
                .filter_map(|t| if let Term::Var(v) = t { Some(*v) } else { None })
                .collect();
            let validate_term = |t: &Term| -> Result<()> {
                match t {
                    Term::Var(v) if !bound.contains(v) => {
                        Err("unsafe variable: no positive body binding".into())
                    }
                    Term::Const(v) if !domain.contains(v) => {
                        Err("constant outside closed domain".into())
                    }
                    _ => Ok(()),
                }
            };
            for pattern in r
                .head
                .iter()
                .chain(&r.positive)
                .chain(&r.gate_true)
                .chain(&r.gate_false)
            {
                if predicates
                    .get(pattern.predicate)
                    .is_none_or(|p| p.arity != pattern.terms.len())
                {
                    return Err("invalid predicate or pattern arity".into());
                }
                for t in &pattern.terms {
                    validate_term(t)?;
                }
            }
            for f in &r.filters {
                let (a, b) = match f {
                    Filter::Eq(a, b) | Filter::Neq(a, b) => (a, b),
                };
                validate_term(a)?;
                validate_term(b)?;
            }
            for p in r.gate_true.iter().chain(&r.gate_false) {
                gate_predicates.insert(p.predicate);
            }
        }
        Ok(Self {
            domain,
            predicates,
            templates,
            gate_predicates,
        })
    }
    fn validate_seed(&self, seed: &BTreeSet<Atom>) -> Result<()> {
        for a in seed {
            if !self.gate_predicates.contains(&a.predicate)
                || self
                    .predicates
                    .get(a.predicate)
                    .is_none_or(|p| p.arity != a.tuple.len())
                || a.tuple.iter().any(|v| !self.domain.contains(v))
            {
                return Err("sparse seed atom outside symbolic gate carrier".into());
            }
        }
        Ok(())
    }
    /// A frozen finite true set is total on S: missing carrier tuples are false.
    /// Errors, including budget exhaustion, return no accepted/rejected value.
    ///
    /// # Errors
    /// Rejects an out-of-carrier seed, exhausted round/work/atom limits, counter
    /// overflow, or an internal head whose variables were not bound.
    pub fn check(&self, seed: &BTreeSet<Atom>, limits: Limits) -> Result<LiftedCheck> {
        self.validate_seed(seed)?;
        let mut relations: Relations = vec![BTreeSet::new(); self.predicates.len()];
        let mut stats = LiftedStats::default();
        let mut violated = false;
        let mut atom_count: usize = 0;
        loop {
            if stats.rounds == limits.max_rounds {
                return Err("incomplete: lifted round budget".into());
            }
            stats.rounds += 1;
            let mut delta = BTreeSet::new();
            for rule in &self.templates {
                let mut publish = |b: &Binding| -> Result<()> {
                    if let Some(h) = &rule.head {
                        let a = instantiate(h, b).ok_or("internal unbound head")?;
                        if !relations[a.predicate].contains(&a.tuple) && !delta.contains(&a) {
                            if atom_count
                                .checked_add(delta.len())
                                .and_then(|n| n.checked_add(1))
                                .is_none_or(|n| n > limits.max_atoms)
                            {
                                return Err("incomplete: lifted atom budget".into());
                            }
                            delta.insert(a);
                        }
                    } else {
                        violated = true;
                    }
                    Ok(())
                };
                Join {
                    relations: &relations,
                    seed,
                    limits,
                }
                .bind(rule, 0, &Binding::new(), &mut stats, &mut publish)?;
            }
            if delta.is_empty() {
                break;
            }
            atom_count += delta.len();
            for a in delta {
                relations[a.predicate].insert(a.tuple);
            }
        }
        let closure: BTreeSet<_> = relations
            .into_iter()
            .enumerate()
            .flat_map(|(predicate, tuples)| {
                tuples
                    .into_iter()
                    .map(move |tuple| Atom { predicate, tuple })
            })
            .collect();
        let projected: BTreeSet<_> = closure
            .iter()
            .filter(|a| self.gate_predicates.contains(&a.predicate))
            .cloned()
            .collect();
        Ok(LiftedCheck {
            stable: &projected == seed && !violated,
            closure,
            constraint_violated: violated,
            stats,
        })
    }
}

/// Immutable relation snapshot and frozen seed for one synchronous source join.
struct Join<'a> {
    relations: &'a Relations,
    seed: &'a BTreeSet<Atom>,
    limits: Limits,
}
impl Join<'_> {
    /// Filter and gate as soon as arguments are bound, then publish complete rows.
    /// Recursion is bounded by the admitted positive-body length.
    fn bind(
        &self,
        rule: &Template,
        depth: usize,
        binding: &Binding,
        stats: &mut LiftedStats,
        publish: &mut impl FnMut(&Binding) -> Result<()>,
    ) -> Result<()> {
        charge(stats, self.limits)?;
        let blocked = rule
            .filters
            .iter()
            .any(|f| filter_truth(f, binding) == Some(false))
            || rule
                .gate_true
                .iter()
                .any(|p| instantiate(p, binding).is_some_and(|a| !self.seed.contains(&a)))
            || rule
                .gate_false
                .iter()
                .any(|p| instantiate(p, binding).is_some_and(|a| self.seed.contains(&a)));
        if blocked {
            stats.guard_prunes += 1;
            return Ok(());
        }
        if depth == rule.positive.len() {
            stats.complete_bindings += 1;
            return publish(binding);
        }
        let pattern = &rule.positive[depth];
        for tuple in &self.relations[pattern.predicate] {
            charge(stats, self.limits)?;
            stats.tuple_probes += 1;
            let mut extended = binding.clone();
            let compatible = pattern
                .terms
                .iter()
                .zip(tuple)
                .all(|(term, value)| match term {
                    Term::Const(v) => v == value,
                    Term::Var(i) => {
                        if let Some(v) = extended.get(i) {
                            v == value
                        } else {
                            extended.insert(*i, value.clone());
                            true
                        }
                    }
                });
            if compatible {
                self.bind(rule, depth + 1, &extended, stats, publish)?;
            }
        }
        Ok(())
    }
}

fn pattern(predicate: usize, terms: Vec<Term>) -> AtomPattern {
    AtomPattern { predicate, terms }
}
fn head_rule(head: AtomPattern, positive: Vec<AtomPattern>) -> Template {
    Template {
        head: Some(head),
        positive,
        gate_true: vec![],
        gate_false: vec![],
        filters: vec![],
    }
}
fn predicates(entries: &[(&str, usize)]) -> Vec<Predicate> {
    entries
        .iter()
        .map(|(name, arity)| Predicate {
            name: (*name).into(),
            arity: *arity,
        })
        .collect()
}
fn integer_domain(size: usize) -> Result<Vec<Value>> {
    (0..size)
        .map(|value| {
            i32::try_from(value)
                .map(Value::Int)
                .map_err(|_| "fixture domain exceeds integer carrier".into())
        })
        .collect()
}
fn diagonal_fixture(n: usize) -> Result<LiftedProgram> {
    let domain = integer_domain(n)?;
    let mut rules: Vec<_> = domain
        .iter()
        .map(|v| head_rule(pattern(0, vec![Term::Const(v.clone())]), vec![]))
        .collect();
    let x = Term::Var(0);
    let dom = pattern(0, vec![x.clone()]);
    let pick = pattern(1, vec![x.clone(), x.clone()]);
    let mut choice = head_rule(pick.clone(), vec![dom.clone()]);
    choice.gate_true.push(pick);
    let mut seen = head_rule(pattern(2, vec![x.clone(), x.clone()]), vec![dom]);
    seen.gate_false.push(pattern(3, vec![x.clone(), x]));
    rules.extend([choice, seen]);
    LiftedProgram::new(
        domain,
        predicates(&[("dom", 1), ("pick", 2), ("seen", 2), ("ban", 2)]),
        rules,
    )
}

fn pair_fixture(n: usize) -> Result<LiftedProgram> {
    let domain = integer_domain(n)?;
    let mut rules: Vec<_> = domain
        .iter()
        .map(|v| head_rule(pattern(0, vec![Term::Const(v.clone())]), vec![]))
        .collect();
    let x = Term::Var(0);
    let y = Term::Var(1);
    let dom_x = pattern(0, vec![x.clone()]);
    let dom_y = pattern(0, vec![y.clone()]);
    let pick_x = pattern(1, vec![x.clone()]);
    let pick_y = pattern(1, vec![y.clone()]);
    let mut choice = head_rule(pick_x.clone(), vec![dom_x.clone()]);
    choice.gate_true.push(pick_x.clone());
    let mut pair = head_rule(pattern(2, vec![x, y]), vec![dom_x, dom_y]);
    pair.gate_true = vec![pick_x, pick_y];
    rules.extend([choice, pair]);
    LiftedProgram::new(
        domain,
        predicates(&[("dom", 1), ("pick", 1), ("pair", 2)]),
        rules,
    )
}

/// One executed sparse fixture with its symbolic size and exact closure record.
#[derive(Clone, Debug)]
pub struct SparseFixtureReport {
    /// Stable fixture name used in the published report.
    pub fixture: &'static str,
    /// Number of distinct scalar values in its authored domain.
    pub domain_size: usize,
    /// Number of possible ground atoms in the symbolic seed carrier.
    pub symbolic_seed_atoms: usize,
    /// Number of true atoms supplied in the sparse seed.
    pub true_seed_atoms: usize,
    /// Complete domain substitutions for the observed source rule before gates.
    pub target_rule_source_substitutions: usize,
    /// Completed reduct inference and measured work counts.
    pub check: LiftedCheck,
}
/// Executed by the CLI; these assert mechanism/work counts, not timing speedups.
///
/// # Errors
/// Returns an error if fixture construction, bounded inference, exact model
/// identity, or the declared sparse-work invariants fail.
pub fn run_sparse_fixtures() -> Result<Vec<SparseFixtureReport>> {
    let mut reports = Vec::new();
    for n in [32, 128] {
        let p = diagonal_fixture(n)?;
        let check = p.check(&BTreeSet::new(), Limits::default())?;
        let expected: BTreeSet<_> = integer_domain(n)?
            .into_iter()
            .flat_map(|v| {
                [
                    Atom {
                        predicate: 0,
                        tuple: vec![v.clone()],
                    },
                    Atom {
                        predicate: 2,
                        tuple: vec![v.clone(), v],
                    },
                ]
            })
            .collect();
        if !check.stable
            || check.closure != expected
            || check.stats.carrier_tuples_enumerated != 0
            || check.stats.ground_rows_materialized != 0
            || check.stats.tuple_probes > (8 * n) as u64
            || check.stats.complete_bindings > (8 * n) as u64
        {
            return Err(format!("sparse lifted fixture failed at n={n}"));
        }
        reports.push(SparseFixtureReport {
            fixture: "diagonal_carrier_laziness",
            domain_size: n,
            symbolic_seed_atoms: 2 * n * n,
            true_seed_atoms: 0,
            target_rule_source_substitutions: n,
            check,
        });
        let p = pair_fixture(n)?;
        let pick = Atom {
            predicate: 1,
            tuple: vec![Value::Int(0)],
        };
        let seed = [pick.clone()].into_iter().collect();
        let check = p.check(&seed, Limits::default())?;
        let mut expected: BTreeSet<_> = integer_domain(n)?
            .into_iter()
            .map(|value| Atom {
                predicate: 0,
                tuple: vec![value],
            })
            .collect();
        expected.insert(pick);
        expected.insert(Atom {
            predicate: 2,
            tuple: vec![Value::Int(0), Value::Int(0)],
        });
        if !check.stable
            || check.closure != expected
            || check.stats.carrier_tuples_enumerated != 0
            || check.stats.ground_rows_materialized != 0
            || check.stats.tuple_probes > (12 * n) as u64
            || check.stats.complete_bindings > (8 * n) as u64
        {
            return Err(format!("candidate-gated pair fixture failed at n={n}"));
        }
        reports.push(SparseFixtureReport {
            fixture: "candidate_gated_pair_join",
            domain_size: n,
            symbolic_seed_atoms: n,
            true_seed_atoms: 1,
            target_rule_source_substitutions: n * n,
            check,
        });
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Program, Rule, subsets};
    // This complete Cartesian expansion is deliberately outside the lazy engine.
    fn products(domain: &[Value], width: usize) -> Vec<Vec<Value>> {
        if width == 0 {
            return vec![vec![]];
        }
        products(domain, width - 1)
            .into_iter()
            .flat_map(|prefix| {
                domain.iter().map(move |v| {
                    let mut tuple = prefix.clone();
                    tuple.push(v.clone());
                    tuple
                })
            })
            .collect()
    }
    fn ground(p: &LiftedProgram) -> (Program, Vec<Atom>) {
        let domain: Vec<_> = p.domain.iter().cloned().collect();
        let atoms: Vec<_> = p
            .predicates
            .iter()
            .enumerate()
            .flat_map(|(predicate, pred)| {
                products(&domain, pred.arity)
                    .into_iter()
                    .map(move |tuple| Atom { predicate, tuple })
            })
            .collect();
        let index: BTreeMap<_, _> = atoms
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, a)| (a, i))
            .collect();
        let mut rules = vec![];
        for t in &p.templates {
            let vars: Vec<_> = t
                .positive
                .iter()
                .flat_map(|p| &p.terms)
                .filter_map(|v| if let Term::Var(i) = v { Some(*i) } else { None })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            for values in products(&domain, vars.len()) {
                let b: Binding = vars.iter().copied().zip(values).collect();
                // Independently substitute, then compare concrete typed values.
                let value = |term: &Term| match term {
                    Term::Const(v) => v.clone(),
                    Term::Var(i) => b[i].clone(),
                };
                if !t.filters.iter().all(|f| match f {
                    Filter::Eq(a, c) => value(a) == value(c),
                    Filter::Neq(a, c) => value(a) != value(c),
                }) {
                    continue;
                }
                let atom = |p: &AtomPattern| Atom {
                    predicate: p.predicate,
                    tuple: p.terms.iter().map(&value).collect(),
                };
                let mask = |patterns: &[AtomPattern]| {
                    patterns
                        .iter()
                        .fold(0, |m, pat| m | (1u64 << index[&atom(pat)]))
                };
                rules.push(Rule {
                    head: t.head.as_ref().map(|h| index[&atom(h)]),
                    choice: false,
                    positive: mask(&t.positive),
                    negative: mask(&t.gate_false),
                    double_negative: mask(&t.gate_true),
                });
            }
        }
        (Program::new(atoms.len(), rules).unwrap(), atoms)
    }
    fn compare(p: &LiftedProgram) {
        let (grounded, atoms) = ground(p);
        assert!(
            atoms.len() <= 8,
            "keep full-model reference genuinely small"
        );
        let symbolic_seed_carrier = atoms.iter().enumerate().fold(0, |m, (i, a)| {
            m | if p.gate_predicates.contains(&a.predicate) {
                1u64 << i
            } else {
                0
            }
        });
        let mut models = vec![];
        for seed in subsets(symbolic_seed_carrier) {
            let sparse = atoms
                .iter()
                .enumerate()
                .filter(|(i, _)| seed & (1u64 << i) != 0)
                .map(|(_, a)| a.clone())
                .collect();
            let checked = p.check(&sparse, Limits::default()).unwrap();
            let model = atoms.iter().enumerate().fold(0, |m, (i, a)| {
                m | if checked.closure.contains(a) {
                    1u64 << i
                } else {
                    0
                }
            });
            // S may conservatively include gates removed by false filters;
            // those extra seed bits do not influence the grounded reduct.
            let expected = grounded.check_seed(seed & grounded.seed_carrier()).unwrap();
            assert_eq!(model, expected.closure);
            assert_eq!(checked.constraint_violated, expected.constraint_violated);
            assert_eq!(
                checked.stable,
                model & symbolic_seed_carrier == seed && !expected.constraint_violated
            );
            if checked.stable {
                models.push(model);
            }
        }
        models.sort_unstable();
        assert_eq!(models, grounded.direct_full_candidate_models());
    }
    #[test]
    fn lifted_grounding_agrees_on_choices_default_negation_and_constraint_filters() {
        let domain = vec![Value::Int(0), Value::Int(1)];
        let x = Term::Var(0);
        let zero = Term::Const(Value::Int(0));
        let mut rules: Vec<_> = domain
            .iter()
            .map(|v| head_rule(pattern(0, vec![Term::Const(v.clone())]), vec![]))
            .collect();
        let dom = pattern(0, vec![x.clone()]);
        let pick = pattern(1, vec![x.clone()]);
        let seen = pattern(2, vec![x.clone()]);
        let mut choice = head_rule(pick.clone(), vec![dom.clone()]);
        choice.gate_true.push(pick.clone());
        let mut derive = head_rule(seen.clone(), vec![dom]);
        derive.gate_false.push(pick);
        rules.extend([
            choice,
            derive,
            Template {
                head: None,
                positive: vec![seen],
                gate_true: vec![],
                gate_false: vec![],
                filters: vec![Filter::Neq(x.clone(), x)],
            },
            Template {
                head: None,
                positive: vec![],
                gate_true: vec![],
                gate_false: vec![pattern(2, vec![zero.clone()])],
                filters: vec![Filter::Eq(zero.clone(), zero)],
            },
        ]);
        compare(
            &LiftedProgram::new(
                domain,
                predicates(&[("dom", 1), ("pick", 1), ("seen", 1)]),
                rules,
            )
            .unwrap(),
        );
    }
    #[test]
    fn lifted_multibody_joins_and_repeated_variables_agree_with_grounding() {
        let c = |i| Term::Const(Value::Int(i));
        let x = Term::Var(0);
        let y = Term::Var(1);
        let rules = vec![
            head_rule(pattern(0, vec![c(0), c(1)]), vec![]),
            head_rule(pattern(0, vec![c(1), c(1)]), vec![]),
            head_rule(
                pattern(1, vec![x.clone()]),
                vec![
                    pattern(0, vec![x.clone(), y.clone()]),
                    pattern(0, vec![y, x.clone()]),
                ],
            ),
            head_rule(
                pattern(1, vec![x.clone()]),
                vec![pattern(0, vec![x.clone(), x])],
            ),
        ];
        compare(
            &LiftedProgram::new(
                vec![Value::Int(0), Value::Int(1)],
                predicates(&[("edge", 2), ("p", 1)]),
                rules,
            )
            .unwrap(),
        );
    }
    #[test]
    fn empty_bindings_and_typed_filter_identity() {
        let one = Term::Const(Value::Int(1));
        let text = Term::Const(Value::Text("1".into()));
        let base = Template {
            head: None,
            positive: vec![],
            gate_true: vec![],
            gate_false: vec![],
            filters: vec![Filter::Eq(one.clone(), text)],
        };
        let p = LiftedProgram::new(
            vec![Value::Int(1), Value::Text("1".into())],
            vec![],
            vec![base.clone()],
        )
        .unwrap();
        assert!(p.check(&BTreeSet::new(), Limits::default()).unwrap().stable);
        compare(&p);
        let mut reject = base;
        reject.filters = vec![Filter::Eq(one.clone(), one)];
        let p = LiftedProgram::new(vec![Value::Int(1)], vec![], vec![reject]).unwrap();
        assert!(!p.check(&BTreeSet::new(), Limits::default()).unwrap().stable);
        compare(&p);
    }
    #[test]
    fn sparse_domain_fixtures_never_enumerate_gate_carrier() {
        run_sparse_fixtures().unwrap();
    }
    #[test]
    fn candidate_gated_pair_join_agrees_with_independent_grounding() {
        compare(&pair_fixture(2).unwrap());
    }
    #[test]
    fn unsafe_templates_bad_seeds_and_incomplete_execution_are_not_models() {
        let unsafe_rule = head_rule(pattern(0, vec![Term::Var(7)]), vec![]);
        assert!(
            LiftedProgram::new(
                vec![Value::Int(0)],
                predicates(&[("p", 1)]),
                vec![unsafe_rule]
            )
            .is_err()
        );
        let p = diagonal_fixture(2).unwrap();
        let bad = [Atom {
            predicate: 1,
            tuple: vec![Value::Int(99), Value::Int(99)],
        }]
        .into_iter()
        .collect();
        assert!(p.check(&bad, Limits::default()).is_err());
        assert!(
            p.check(
                &BTreeSet::new(),
                Limits {
                    max_rounds: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        assert!(
            p.check(
                &BTreeSet::new(),
                Limits {
                    max_work: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        assert!(
            p.check(
                &BTreeSet::new(),
                Limits {
                    max_atoms: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn oversized_positive_bodies_are_refused_before_recursive_execution() {
        let nullary = pattern(0, vec![]);
        let too_wide = head_rule(
            nullary.clone(),
            vec![nullary.clone(); MAX_POSITIVE_BODY + 1],
        );
        assert!(LiftedProgram::new(vec![], predicates(&[("p", 0)]), vec![too_wide]).is_err());
        let boundary = head_rule(nullary.clone(), vec![nullary.clone(); MAX_POSITIVE_BODY]);
        let fact = head_rule(nullary, vec![]);
        let program =
            LiftedProgram::new(vec![], predicates(&[("p", 0)]), vec![fact, boundary]).unwrap();
        let checked = program.check(&BTreeSet::new(), Limits::default()).unwrap();
        assert!(checked.stable);
        assert_eq!(checked.closure.len(), 1);
    }
}
