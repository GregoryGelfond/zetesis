use std::{collections::BTreeSet, mem::size_of};

use serde::Serialize;
use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Cancellation;

use super::{Case, Configuration, Error, Output, config::DOMAIN_VALUES as VALUES};

const QUERY_PERIOD: usize = 7;
const PAYLOAD_ENVELOPE: usize = 256;
const MAX_FIXTURE_BYTES: usize = 32 * 1024 * 1024;

pub(super) struct Fixture {
    pub predicate: Predicate,
    pub atoms: Vec<Atom>,
    pub indices: Vec<usize>,
    pub scope: Vec<usize>,
    pub values: Vec<Value>,
    pub domains: Vec<Vec<Vec<Value>>>,
    pub storage_bound: usize,
}

/// Exact typed source/query identity. Value IDs are positions in `values`, not
/// integers from the model or an implementation's relation dictionary.
#[derive(Serialize)]
pub struct Subject {
    /// Version of the deterministic row/domain generator.
    pub generator: u32,
    /// Original predicate name.
    pub predicate: &'static str,
    /// Original predicate sign, independent of value identity.
    pub sign: &'static str,
    /// Canonical typed values; all row/domain IDs index this exact list.
    pub values: Vec<TypedValue>,
    /// Authoritative original source tuples in source index order.
    pub rows: Vec<Vec<usize>>,
    /// Original source index per relation row occurrence; duplicates remain.
    pub original_indices: Vec<usize>,
    /// Variable label per argument column, including repeated variables.
    pub scope: Vec<usize>,
    /// Complete ordered queries, variables and input-domain value IDs.
    pub domains: Vec<Vec<Vec<usize>>>,
}

/// Exact logical values used by this fixed experiment population.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum TypedValue {
    /// Integer, distinct from text with the same spelling.
    Number(i32),
    /// String literal, including its exact content.
    String(String),
    /// Symbolic constant, including its exact name.
    Symbol(String),
    /// One-element tuple whose sole child is the supplied integer.
    UnaryTuple(i32),
}

impl Fixture {
    pub fn new(config: Configuration, cancellation: &Cancellation) -> Result<Self, Error> {
        cancellation.poll().map_err(Error::Stopped)?;
        let scope = match config.case {
            Case::Correlated => vec![0, 1],
            Case::Independent => vec![0, 1, 2, 3],
            Case::Aliased => vec![0, 1, 0],
        };
        let variables = 1 + scope.iter().max().copied().unwrap_or(0);
        // The fixed generator has bounded short payloads. This envelope counts
        // every clone independently, including Arc-backed tuples and source
        // predicate text; it also reserves twice the input vectors for subject
        // views and reference scratch. It is deliberately not reported as RSS.
        let source_cells = config.rows * scope.len();
        let domain_cells = config.queries * variables * (VALUES + 1);
        let storage_bound = 2
            * ((source_cells + domain_cells + VALUES + 1)
                * (size_of::<Value>() + PAYLOAD_ENVELOPE)
                + config.rows * (size_of::<Atom>() + size_of::<usize>() + PAYLOAD_ENVELOPE)
                + config.queries * variables * size_of::<Vec<Value>>());
        if storage_bound > MAX_FIXTURE_BYTES {
            return Err(Error::Configuration("fixture envelope exceeds 32 MiB"));
        }
        let predicate = Predicate::with_sign("finite_table", scope.len(), Sign::Negative)
            .map_err(Error::Atom)?;
        let mut values = Vec::with_capacity(VALUES + 1);
        for id in 0..=VALUES {
            values.push(value(config.case, id)?);
        }
        values.sort_unstable();
        values.dedup();
        let mut atoms = Vec::with_capacity(config.rows);
        for row in 0..config.rows {
            cancellation.poll().map_err(Error::Stopped)?;
            let cells = (0..scope.len())
                .map(|column| {
                    let id = match config.case {
                        Case::Correlated => row % VALUES,
                        Case::Independent => {
                            (row / VALUES.pow(u32::try_from(column).unwrap())) % VALUES
                        }
                        Case::Aliased => match column {
                            0 => row % VALUES,
                            1 => (row / VALUES) % VALUES,
                            _ => (row + usize::from(row % 3 == 0)) % VALUES,
                        },
                    };
                    value(config.case, id)
                })
                .collect::<Result<Vec<_>, _>>()?;
            atoms.push(Atom::new(predicate.clone(), cells).map_err(Error::Atom)?);
        }
        let indices: Vec<_> = (0..config.rows)
            .map(|row| config.rows - 1 - row / 2)
            .collect();
        let full = (0..VALUES)
            .map(|id| value(config.case, id))
            .collect::<Result<Vec<_>, _>>()?;
        let absent = value(config.case, VALUES)?;
        let mut domains = Vec::with_capacity(config.queries);
        for query in 0..config.queries {
            let mut family = Vec::with_capacity(variables);
            for variable in 0..variables {
                let first = scope.iter().position(|&label| label == variable).unwrap();
                let singleton = atoms[indices[0]].values()[first].clone();
                family.push(match query % QUERY_PERIOD {
                    1 => vec![singleton],
                    2 => vec![absent.clone()],
                    3 => full.iter().step_by(2).cloned().collect(),
                    4 if variable == 0 => Vec::new(),
                    5 => full.iter().skip(1).step_by(2).cloned().collect(),
                    _ => full.clone(),
                });
            }
            domains.push(family);
        }
        Ok(Self {
            predicate,
            atoms,
            indices,
            scope,
            values,
            domains,
            storage_bound,
        })
    }

    pub fn id<'value>(
        &self,
        value: impl Into<zetesis_core::catalog::TermRef<'value>>,
    ) -> Result<usize, Error> {
        let value = value.into();
        self.values
            .binary_search_by(|entry| value.compare(entry).reverse())
            .map_err(|_| Error::Parity)
    }

    pub fn subject(&self, case: Case) -> Result<Subject, Error> {
        let mut values: Vec<_> = (0..=VALUES)
            .map(|id| Ok((value(case, id)?, description(case, id))))
            .collect::<Result<_, Error>>()?;
        values.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        Ok(Subject {
            generator: 1,
            predicate: "finite_table",
            sign: "negative",
            values: values.into_iter().map(|(_, value)| value).collect(),
            rows: self
                .atoms
                .iter()
                .map(|atom| atom.values().iter().map(|v| self.id(v)).collect())
                .collect::<Result<_, _>>()?,
            original_indices: self.indices.clone(),
            scope: self.scope.clone(),
            domains: self
                .domains
                .iter()
                .map(|family| {
                    family
                        .iter()
                        .map(|domain| domain.iter().map(|v| self.id(v)).collect())
                        .collect()
                })
                .collect::<Result<_, _>>()?,
        })
    }

    /// Independent original-row reference: linear domain membership and pairwise
    /// alias equality. No prepared Table, relation IDs or binary domain lookup.
    pub fn reference(&self, query: usize, cancellation: &Cancellation) -> Result<Output, Error> {
        let domains = &self.domains[query];
        let mut rows = Vec::new();
        let mut projected = vec![BTreeSet::new(); domains.len()];
        for (position, &original) in self.indices.iter().enumerate() {
            cancellation.poll().map_err(Error::Stopped)?;
            let tuple = self.atoms[original].values();
            if !tuple
                .iter()
                .zip(&self.scope)
                .all(|(value, &variable)| domains[variable].contains(value))
            {
                continue;
            }
            if tuple
                .iter()
                .zip(&self.scope)
                .enumerate()
                .any(|(column, (value, variable))| {
                    tuple
                        .iter()
                        .zip(&self.scope)
                        .take(column)
                        .any(|(other, label)| variable == label && value != other)
                })
            {
                continue;
            }
            rows.push(position);
            for (value, &variable) in tuple.iter().zip(&self.scope) {
                projected[variable].insert(self.id(value)?);
            }
        }
        Ok(Output {
            rows,
            domains: projected
                .into_iter()
                .map(|domain| domain.into_iter().collect())
                .collect(),
        })
    }
}

fn description(case: Case, id: usize) -> TypedValue {
    let number = i32::try_from(id / 4).unwrap();
    if case == Case::Correlated {
        return TypedValue::Number(i32::try_from(id).unwrap());
    }
    match id % 4 {
        0 => TypedValue::Number(number),
        1 => TypedValue::String(number.to_string()),
        2 => TypedValue::Symbol(format!("v{number}")),
        _ => TypedValue::UnaryTuple(number),
    }
}
fn value(case: Case, id: usize) -> Result<Value, Error> {
    Ok(match description(case, id) {
        TypedValue::Number(number) => Value::Number(number),
        TypedValue::String(text) => Value::String(text),
        TypedValue::Symbol(text) => Value::Symbol(text),
        TypedValue::UnaryTuple(number) => Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(number)],
            ValueLimits::default(),
        )
        .map_err(Error::Value)?,
    })
}
