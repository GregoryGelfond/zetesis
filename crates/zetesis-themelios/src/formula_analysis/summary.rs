//! Summary analysis and an exact normalized program reconstructed only on demand.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use themelios_analysis::Analysis;
use themelios_program::program::{Atom, Program, Statement};
use themelios_program::provenance::WithProvenance;

use super::facts::{self, Allocation, Checked, Families, Inspection};
use crate::expansion::Budget;
use crate::formula_keys::owners::Owners;
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, ProgramSite, StatementId};

/// The internal analyzed carrier: every non-fact statement and the first fact
/// of each signed signature. A private layout separates representatives from
/// actual non-fact producers, including a carrier equal to a representative.
pub(crate) struct Summary {
    program: Program,
    plan: Option<Plan>,
    full: OnceLock<FullProgram>,
    prepared_full: OnceLock<()>,
}

struct Plan {
    source: Arc<Program>,
    families: Families,
    facts: Vec<facts::Fact>,
    nonfacts: Vec<Option<StatementId>>,
}

pub(crate) struct FullProgram {
    pub(crate) program: Program,
    pub(crate) analysis: Analysis,
}

impl std::fmt::Debug for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Summary")
            .field("statements", &self.program.statements().count())
            .field(
                "fact_occurrences",
                &self.plan.as_ref().map_or(0, |plan| plan.families.rows()),
            )
            .field(
                "canonical_facts",
                &self.plan.as_ref().map_or(0, |plan| plan.facts.len()),
            )
            .finish_non_exhaustive()
    }
}

impl Summary {
    pub(crate) fn collect(
        source: Arc<Program>,
        statements: Vec<WithProvenance<Statement>>,
        owners: Vec<Option<StatementId>>,
        sources: Vec<StatementId>,
        families: Families,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<(Self, Owners), FormulaFailure> {
        let mut allocation = Checked { budget, site };
        let reader = families.read(&source, &mut allocation)?;
        let mut representatives = Vec::new();
        let mut signatures = BTreeSet::new();
        for position in families.first_rows() {
            allocation.budget.poll(site)?;
            let (sign, name, arity) = reader.signature(position);
            if !signatures.contains(&(sign, name, arity)) {
                allocation.budget.charge(
                    ExpansionResource::ScalarBytes,
                    size_of::<(
                        themelios_program::symbol::Sign,
                        &themelios_program::symbol::Name,
                        usize,
                    )>() as u128,
                    site,
                )?;
                signatures.insert((sign, name, arity));
                allocation.reserve(&mut representatives, 1)?;
                representatives.push(reader.representative(position));
            }
        }
        let facts = reader.canonical(&mut allocation)?;
        drop(signatures);
        drop(reader);
        let (program, owners, nonfacts) = Owners::collect_summary(
            statements,
            owners,
            representatives,
            sources,
            allocation.budget,
            site,
        )?;
        Ok((
            Self {
                program,
                plan: Some(Plan {
                    source,
                    families,
                    facts,
                    nonfacts,
                }),
                full: OnceLock::new(),
                prepared_full: OnceLock::new(),
            },
            owners,
        ))
    }

    /// A terminal partition already owns a full normalized program. Its analysis
    /// is carried alongside it, and neither accessor needs a reconstruction.
    pub(crate) fn complete(program: Program) -> Self {
        Self {
            program,
            plan: None,
            full: OnceLock::new(),
            prepared_full: OnceLock::new(),
        }
    }

    /// Summary carriers for dependency/classes and the bounded rewrite passes.
    pub(crate) fn program(&self) -> &Program {
        &self.program
    }

    /// Every non-fact analysis producer. Representatives never masquerade as
    /// the complete fact population; genuine generated fact carriers join the
    /// same canonical atom sequence as arena families in `atoms`.
    pub(crate) fn producers(&self) -> impl Iterator<Item = &Statement> {
        self.program
            .statements()
            .enumerate()
            .filter_map(|(index, carrier)| {
                self.plan
                    .as_ref()
                    .is_none_or(|plan| plan.nonfacts[index].is_some())
                    .then_some(carrier.get())
                    .filter(|statement| crate::fact_expansion::fact_atom(statement).is_none())
            })
    }

    /// Preserve non-fact identity through the one replacement compilation. The
    /// supplied carriers are old summary positions followed by new constraints;
    /// the parallel flags describe which are real non-fact carriers.
    pub(crate) fn replace(
        &mut self,
        carriers: Vec<WithProvenance<Statement>>,
        nonfacts: Vec<Option<StatementId>>,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        assert_eq!(carriers.len(), nonfacts.len());
        let mut content = std::collections::BTreeMap::new();
        for (carrier, nonfact) in carriers.iter().zip(nonfacts) {
            budget.charge(ExpansionResource::TermWork, 1, site)?;
            budget.charge(
                ExpansionResource::ScalarBytes,
                size_of::<(&Statement, Option<StatementId>)>() as u128,
                site,
            )?;
            content
                .entry(carrier.get())
                .and_modify(|existing| {
                    if nonfact.is_some() {
                        *existing = nonfact;
                    }
                })
                .or_insert(nonfact);
        }
        let mut flags = Vec::new();
        crate::formula_pattern::reserve(&mut flags, content.len(), budget, site)?;
        flags.extend(content.into_values());
        self.program = Program::of_nodes(carriers);
        if let Some(plan) = &mut self.plan {
            plan.nonfacts = flags;
        }
        assert!(
            self.full.get().is_none(),
            "replacement precedes full program construction"
        );
        Ok(())
    }

    pub(crate) fn source_at(&self, index: usize) -> Option<StatementId> {
        self.plan
            .as_ref()
            .expect("only compact preparation performs replacement")
            .nonfacts[index]
    }

    /// Analyze the summary after the exact, deduplicated full-program preflight.
    /// A fact contributes its predicate node, no edge, and no non-Horn or unsafe
    /// structure. Pin-differential tests establish equal graph and class verdicts.
    pub(crate) fn analyze(
        &self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        fallback: ProgramSite,
    ) -> Result<Analysis, FormulaFailure> {
        let Some(plan) = &self.plan else {
            return super::analyze(&self.program, limits, budget, fallback);
        };
        let reader = plan.families.read(
            &plan.source,
            &mut Checked {
                budget,
                site: fallback,
            },
        )?;
        let mut scan = super::Preflight::default();
        for entry in self.entries(&reader) {
            match entry {
                Entry::Statement(statement) => {
                    scan.statement(statement, limits, budget, fallback)?;
                }
                Entry::Fact(fact) => scan.fact(
                    reader.arity(fact),
                    reader.name_bytes(fact),
                    &reader.provenance(fact),
                    limits,
                    budget,
                    fallback,
                )?,
                Entry::Both {
                    statement, fact, ..
                } => {
                    let evidence = reader
                        .provenance(fact)
                        .into_owned()
                        .merge(statement.provenance().clone());
                    scan.fact(
                        reader.arity(fact),
                        reader.name_bytes(fact),
                        &evidence,
                        limits,
                        budget,
                        fallback,
                    )?;
                }
            }
        }
        budget.poll(fallback)?;
        let analysis = Analysis::of(&self.program);
        budget.poll(fallback)?;
        Ok(analysis)
    }

    /// Exact public program, built once from prevalidated source values. Ordinary
    /// allocation is the only remaining failure boundary; no fallible bridge or
    /// admission is deferred behind this infallible accessor.
    pub(crate) fn full_program(&self) -> &Program {
        if self.plan.is_none() {
            &self.program
        } else {
            &self.full().program
        }
    }

    pub(crate) fn full_analysis<'a>(&'a self, summary: &'a Analysis) -> &'a Analysis {
        if self.plan.is_none() {
            summary
        } else {
            &self.full().analysis
        }
    }

    fn full(&self) -> &FullProgram {
        self.full.get_or_init(|| self.rebuild())
    }

    fn rebuild(&self) -> FullProgram {
        let plan = self.plan.as_ref().expect("only compact summaries rebuild");
        let reader = plan
            .families
            .read(&plan.source, &mut Inspection)
            .unwrap_or_else(|never| match never {});
        self.rebuild_with(&reader, &mut Inspection)
            .unwrap_or_else(|never| match never {})
    }

    fn rebuild_with<A: Allocation>(
        &self,
        reader: &facts::Reader<'_>,
        allocation: &mut A,
    ) -> Result<FullProgram, A::Failure> {
        let mut statements = Vec::new();
        let mut count = 0_usize;
        for _ in self.entries(reader) {
            allocation.poll()?;
            count += 1;
        }
        allocation.reserve_prepaid(&mut statements, count)?;
        for entry in self.entries(reader) {
            allocation.poll()?;
            let statement = match entry {
                Entry::Statement(statement) => statement.clone(),
                Entry::Fact(fact) => reader.statement(fact),
                Entry::Both {
                    statement,
                    owner,
                    fact,
                } => {
                    let evidence = reader
                        .provenance(fact)
                        .into_owned()
                        .merge(statement.provenance().clone());
                    let value = if owner >= reader.last_owner(fact) {
                        statement.get().clone()
                    } else {
                        reader.statement(fact).into_value()
                    };
                    WithProvenance::new(value, evidence)
                }
            };
            statements.push(statement);
        }
        allocation.poll()?;
        let program = Program::of_nodes(statements);
        allocation.poll()?;
        let analysis = Analysis::of(&program);
        allocation.poll()?;
        Ok(FullProgram { program, analysis })
    }

    fn entries<'a>(&'a self, reader: &'a facts::Reader<'a>) -> impl Iterator<Item = Entry<'a>> {
        let plan = self.plan.as_ref().expect("compact summary layout");
        let mut statements = self
            .program
            .statements()
            .enumerate()
            .filter_map(|(index, statement)| plan.nonfacts[index].map(|owner| (statement, owner)))
            .peekable();
        let mut facts = plan.facts.iter().peekable();
        std::iter::from_fn(move || match (statements.peek(), facts.peek()) {
            (Some((statement, _)), Some(fact)) => {
                match reader.compare_statement(fact, statement.get()) {
                    Ordering::Less => Some(Entry::Fact(facts.next().expect("peeked fact"))),
                    Ordering::Equal => {
                        let (statement, owner) = statements.next().expect("peeked statement");
                        Some(Entry::Both {
                            statement,
                            owner,
                            fact: facts.next().expect("peeked fact"),
                        })
                    }
                    Ordering::Greater => Some(Entry::Statement(
                        statements.next().expect("peeked statement").0,
                    )),
                }
            }
            (Some(_), None) => Some(Entry::Statement(
                statements.next().expect("peeked statement").0,
            )),
            (None, Some(_)) => Some(Entry::Fact(facts.next().expect("peeked fact"))),
            (None, None) => None,
        })
    }

    /// Explicit preparation trigger for terminal analysis. Atom construction is
    /// charged before a fresh full view is published. Refusal publishes no new
    /// view or completed-trigger marker; prior public inspection is retained.
    /// The exact analysis ceilings already passed in `analyze`.
    pub(crate) fn materialize(
        &self,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<&Program, FormulaFailure> {
        if self.plan.is_none() || self.prepared_full.get().is_some() {
            return Ok(self.full_program());
        }
        // Public inspection cannot waive a mandatory preparation charge. The
        // separate marker records this budgeted trigger even if inspection
        // already populated the infallible cache.
        self.charge_atoms(budget, site)?;
        let plan = self.plan.as_ref().expect("compact preparation");
        let mut allocation = Checked { budget, site };
        let reader = plan.families.read(&plan.source, &mut allocation)?;
        let mut count = 0_usize;
        for _ in self.entries(&reader) {
            allocation.budget.poll(site)?;
            count += 1;
        }
        // This logical full-carrier reservation is required at the terminal
        // trigger even when introspection already allocated those carriers.
        // It is one deterministic charge, independent of allocator growth or
        // inspection history; the fresh builder reserves exactly this room.
        allocation.budget.charge(
            ExpansionResource::ScalarBytes,
            count as u128 * size_of::<WithProvenance<Statement>>() as u128,
            site,
        )?;
        let full = if self.full.get().is_none() {
            Some(self.rebuild_with(&reader, &mut allocation)?)
        } else {
            None
        };
        allocation.budget.poll(site)?;
        if let Some(full) = full {
            self.full
                .set(full)
                .unwrap_or_else(|_| unreachable!("private preparation initializes once"));
        }
        self.prepared_full
            .set(())
            .expect("one budgeted full-program trigger");
        Ok(&self.full.get().expect("materialized program").program)
    }

    pub(crate) fn into_full(self, analysis: Analysis) -> FullProgram {
        if self.plan.is_none() {
            FullProgram {
                program: self.program,
                analysis,
            }
        } else if let Some(full) = self.full.into_inner() {
            full
        } else {
            unreachable!("terminal partition materialized the original before taking it")
        }
    }

    pub(crate) fn atoms(
        &self,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<Vec<Atom>, FormulaFailure> {
        self.charge_atoms(budget, site)?;
        let mut atoms = Vec::new();
        if let Some(plan) = &self.plan {
            let mut allocation = Checked { budget, site };
            let reader = plan.families.read(&plan.source, &mut allocation)?;
            for entry in self.entries(&reader) {
                allocation.budget.poll(site)?;
                match entry {
                    Entry::Statement(statement) => {
                        if let Some(atom) = crate::fact_expansion::fact_atom(statement.get()) {
                            allocation.reserve(&mut atoms, 1)?;
                            atoms.push(atom.clone());
                        }
                    }
                    Entry::Fact(fact) | Entry::Both { fact, .. } => {
                        allocation.reserve(&mut atoms, 1)?;
                        atoms.push(reader.atom(fact));
                    }
                }
            }
        } else {
            let mut allocation = Checked { budget, site };
            for statement in self.program.statements() {
                allocation.poll()?;
                if let Some(atom) = crate::fact_expansion::fact_atom(statement.get()) {
                    allocation.reserve(&mut atoms, 1)?;
                    atoms.push(atom.clone());
                }
            }
        }
        Ok(atoms)
    }

    fn charge_atoms(&self, budget: &mut Budget, site: ProgramSite) -> Result<(), FormulaFailure> {
        if let Some(plan) = &self.plan {
            let reader = plan
                .families
                .read(&plan.source, &mut Checked { budget, site })?;
            for entry in self.entries(&reader) {
                budget.poll(site)?;
                let (arity, name_bytes) = match entry {
                    Entry::Fact(fact) | Entry::Both { fact, .. } => {
                        (reader.arity(fact), reader.name_bytes(fact))
                    }
                    Entry::Statement(statement) => {
                        let Some(atom) = crate::fact_expansion::fact_atom(statement.get()) else {
                            continue;
                        };
                        // The existing pool-free preflight validates this producer;
                        // a generated source fact need not belong to an arena family.
                        atom_size(atom)
                    }
                };
                budget.charge(ExpansionResource::TermWork, arity as u128 + 2, site)?;
                budget.charge(ExpansionResource::ScalarBytes, name_bytes as u128 * 2, site)?;
            }
        } else {
            for statement in self.program.statements() {
                budget.poll(site)?;
                if let Some(atom) = crate::fact_expansion::fact_atom(statement.get()) {
                    let (arity, name_bytes) = atom_size(atom);
                    budget.charge(ExpansionResource::TermWork, arity as u128 + 2, site)?;
                    budget.charge(ExpansionResource::ScalarBytes, name_bytes as u128 * 2, site)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn is_materialized(&self) -> bool {
        self.plan.is_none() || self.full.get().is_some()
    }
}

fn atom_size(atom: &Atom) -> (usize, usize) {
    let themelios_program::program::Arguments::Single(terms) = &atom.arguments else {
        unreachable!("normalized analysis facts have one argument tuple")
    };
    (terms.len(), atom.name.as_str().len())
}

enum Entry<'a> {
    Statement(&'a WithProvenance<Statement>),
    Fact(&'a facts::Fact),
    Both {
        statement: &'a WithProvenance<Statement>,
        owner: StatementId,
        fact: &'a facts::Fact,
    },
}

/// The preparation cache owns lazy atoms; a separate reader borrows them and
/// holds `FactIndex`. This separation prevents a self-referential owned index.
pub(crate) struct PreparationFacts<'a> {
    pub(crate) summary: &'a Summary,
    atoms: std::cell::OnceCell<Vec<Atom>>,
}

impl<'a> PreparationFacts<'a> {
    pub(crate) const fn new(summary: &'a Summary) -> Self {
        Self {
            summary,
            atoms: std::cell::OnceCell::new(),
        }
    }

    pub(crate) fn atoms(
        &self,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<&[Atom], FormulaFailure> {
        if self.atoms.get().is_none() {
            let atoms = self.summary.atoms(budget, site)?;
            self.atoms
                .set(atoms)
                .unwrap_or_else(|_| unreachable!("one private preparation reader"));
        }
        Ok(self.atoms.get().expect("prepared fact atoms"))
    }
}
