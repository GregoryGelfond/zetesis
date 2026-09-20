//! Native outcome reconciliation and whole-record decoding.
use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_core::{Atom, Predicate, Value};

use super::{Limits, ModelRecord, NativeAnswers, values};
use crate::answers::{Error, Issue, Resource, check, invalid};

pub(super) fn parse(text: &str, limits: Limits) -> Result<NativeAnswers, Error> {
    let document: Json = serde_json::from_str(text).map_err(Error::Json)?;
    let schema = document["schema"].as_u64();
    if !super::is_native_document(&document) {
        return Err(invalid(
            Issue::MalformedField,
            "unsupported native JSON schema",
        ));
    }
    let outcome = &document["outcome"];
    if outcome["completion"] != "exhausted"
        || outcome["coverage"] != "exhausted"
        || !matches!(
            outcome["status"].as_str(),
            Some("satisfiable" | "unsatisfiable")
        )
        || outcome.get("error") != Some(&Json::Null)
        || outcome.get("interruption") != Some(&Json::Null)
    {
        return Err(invalid(
            Issue::Incomplete,
            "native outcome did not complete publication and enumeration",
        ));
    }
    let raw = values::array(&document["models"], "native model records")?;
    check(Resource::Witnesses, limits.report.max_witnesses, raw.len())?;
    let published = values::unsigned(&outcome["published_models"], "native published models")?;
    let verified_models = values::unsigned(&outcome["verified_models"], "native verified models")?;
    let checked = values::unsigned(&outcome["checked"], "native checked candidates")?;
    if usize::try_from(published).ok() != Some(raw.len())
        || checked < verified_models
        || verified_models < published
        || (outcome["status"] == "satisfiable") == raw.is_empty()
    {
        return Err(invalid(
            Issue::Contradiction,
            "native model/status/publication counts",
        ));
    }
    let costs = optimum(outcome, published, verified_models, limits)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(raw.len())
        .map_err(|_| Error::Allocation)?;
    let mut count = Counts::default();
    // The document's atom table: the atoms every record spelled, in order.
    let mut table = AtomTable::default();
    for (index, raw) in raw.iter().enumerate() {
        if values::index(&raw["number"], "native record number")? != index + 1 {
            return Err(invalid(
                Issue::Contradiction,
                "native publication numbering",
            ));
        }
        let record = if schema == Some(2) {
            record(&raw["model"], limits, &mut count, &mut table)?
        } else {
            spelled_record(&raw["model"], limits, &mut count)?
        };
        if record.costs != costs {
            return Err(invalid(
                Issue::Contradiction,
                "native record differs from its final optimum",
            ));
        }
        records.push(record);
    }
    Ok(NativeAnswers {
        records,
        verified_models,
        checked,
        costs,
    })
}

fn optimum(
    outcome: &Json,
    published: u64,
    verified: u64,
    limits: Limits,
) -> Result<Option<Vec<(i32, i64)>>, Error> {
    let optimization = outcome
        .get("optimization")
        .ok_or_else(|| invalid(Issue::MissingField, "native optimization field"))?;
    if optimization.is_null() {
        if published != verified {
            return Err(invalid(
                Issue::Contradiction,
                "unoptimized native publication is incomplete",
            ));
        }
        return Ok(None);
    }
    if optimization["optimal"] != true
        || published == 0
        || values::unsigned(&optimization["tied_models"], "native optimal ties")? != published
        || values::unsigned(&optimization["scored_models"], "native scored models")? != verified
    {
        return Err(invalid(
            Issue::Contradiction,
            "native optimum/score/tie counts",
        ));
    }
    values::unsigned(&optimization["work"], "native objective work")?;
    Ok(Some(values::costs(&optimization["costs"], limits)?))
}

#[derive(Default)]
struct Counts {
    atoms: usize,
    nodes: usize,
    shown: usize,
}

/// A spelled atom and the value-node count charged whenever a model uses it.
struct TableAtom {
    atom: Atom,
    value_nodes: usize,
}

/// Table decoding has its own cumulative ceiling, including unused entries.
/// Model occurrences are charged separately before their atoms are retained.
#[derive(Default)]
struct AtomTable {
    atoms: Vec<TableAtom>,
    value_nodes: usize,
}

/// Decode one record against the document's table: its spelled atoms extend
/// the table, its full model and shown positions are indices into it. The
/// returned record holds its atoms in full and its shown atoms as positions
/// within them, as a spelled record would.
fn record(
    raw: &Json,
    limits: Limits,
    count: &mut Counts,
    table: &mut AtomTable,
) -> Result<ModelRecord, Error> {
    let spelled = values::array(&raw["atoms"], "native spelled atoms")?;
    table
        .atoms
        .try_reserve(spelled.len())
        .map_err(|_| Error::Allocation)?;
    for atom in spelled {
        let before = table.value_nodes;
        let atom = decode_atom(atom, limits, &mut table.value_nodes)?;
        table.atoms.push(TableAtom {
            atom,
            value_nodes: table.value_nodes - before,
        });
    }
    let raw_atoms = values::array(&raw["full_model"], "native full model")?;
    count.atoms = count
        .atoms
        .checked_add(raw_atoms.len())
        .ok_or_else(|| invalid(Issue::CountOverflow, "native atom count"))?;
    check(Resource::Atoms, limits.max_atoms, count.atoms)?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(raw_atoms.len())
        .map_err(|_| Error::Allocation)?;
    let mut positions = std::collections::BTreeMap::new();
    for (position, index) in raw_atoms.iter().enumerate() {
        let index = values::index(index, "native atom index")?;
        let Some(entry) = table.atoms.get(index) else {
            return Err(invalid(
                Issue::Contradiction,
                "native atom index is outside the document's table",
            ));
        };
        count.nodes = count
            .nodes
            .checked_add(entry.value_nodes)
            .ok_or_else(|| invalid(Issue::CountOverflow, "native value node count"))?;
        check(Resource::ValueNodes, limits.max_value_nodes, count.nodes)?;
        positions.insert(index, position);
        atoms.push(entry.atom.clone());
    }
    if atoms.iter().collect::<BTreeSet<_>>().len() != atoms.len() {
        return Err(invalid(
            Issue::Contradiction,
            "duplicate atom in native full model",
        ));
    }
    let shown = &raw["shown"];
    let indices = values::array(&shown["atom_indices"], "native shown indices")?;
    let terms = values::array(&shown["terms"], "native shown terms")?;
    count.shown = count
        .shown
        .checked_add(indices.len())
        .and_then(|value| value.checked_add(terms.len()))
        .ok_or_else(|| invalid(Issue::CountOverflow, "native shown occurrence count"))?;
    check(Resource::Symbols, limits.report.max_symbols, count.shown)?;
    let mut shown_indices = Vec::new();
    shown_indices
        .try_reserve_exact(indices.len())
        .map_err(|_| Error::Allocation)?;
    for index in indices {
        let index = values::index(index, "native shown index")?;
        let Some(&position) = positions.get(&index) else {
            return Err(invalid(
                Issue::Contradiction,
                "native shown index is outside its full model",
            ));
        };
        if shown_indices
            .last()
            .is_some_and(|previous| *previous >= position)
        {
            return Err(invalid(
                Issue::Contradiction,
                "native shown position is out of order or repeated",
            ));
        }
        shown_indices.push(position);
    }
    let tail = terms_and_costs(raw, terms, limits, count)?;
    Ok(ModelRecord {
        atoms,
        shown_indices,
        shown_terms: tail.terms,
        costs: tail.costs,
    })
}

/// Decode a schema-1 record, which spells every atom of its model and gives
/// its shown atoms as positions within them, so the documents of earlier
/// executables can still be compared.
fn spelled_record(raw: &Json, limits: Limits, count: &mut Counts) -> Result<ModelRecord, Error> {
    let raw_atoms = values::array(&raw["full_model"], "native full model")?;
    count.atoms = count
        .atoms
        .checked_add(raw_atoms.len())
        .ok_or_else(|| invalid(Issue::CountOverflow, "native atom count"))?;
    check(Resource::Atoms, limits.max_atoms, count.atoms)?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(raw_atoms.len())
        .map_err(|_| Error::Allocation)?;
    for atom in raw_atoms {
        atoms.push(decode_atom(atom, limits, &mut count.nodes)?);
    }
    if atoms.iter().collect::<BTreeSet<_>>().len() != atoms.len() {
        return Err(invalid(
            Issue::Contradiction,
            "duplicate atom in native full model",
        ));
    }
    let shown = &raw["shown"];
    let indices = values::array(&shown["atom_indices"], "native shown indices")?;
    let terms = values::array(&shown["terms"], "native shown terms")?;
    count.shown = count
        .shown
        .checked_add(indices.len())
        .and_then(|value| value.checked_add(terms.len()))
        .ok_or_else(|| invalid(Issue::CountOverflow, "native shown occurrence count"))?;
    check(Resource::Symbols, limits.report.max_symbols, count.shown)?;
    let mut shown_indices = Vec::new();
    shown_indices
        .try_reserve_exact(indices.len())
        .map_err(|_| Error::Allocation)?;
    for index in indices {
        let index = values::index(index, "native shown position")?;
        if index >= atoms.len()
            || shown_indices
                .last()
                .is_some_and(|previous| *previous >= index)
        {
            return Err(invalid(
                Issue::Contradiction,
                "native shown position is outside its full model or repeated",
            ));
        }
        shown_indices.push(index);
    }
    let tail = terms_and_costs(raw, terms, limits, count)?;
    Ok(ModelRecord {
        atoms,
        shown_indices,
        shown_terms: tail.terms,
        costs: tail.costs,
    })
}

/// The shown terms and the costs a record of either form ends with.
struct Tail {
    terms: Vec<Value>,
    costs: Option<Vec<(i32, i64)>>,
}

/// The shown terms and the costs of a record of either form.
fn terms_and_costs(
    raw: &Json,
    terms: &[Json],
    limits: Limits,
    count: &mut Counts,
) -> Result<Tail, Error> {
    let mut shown_terms = Vec::new();
    shown_terms
        .try_reserve_exact(terms.len())
        .map_err(|_| Error::Allocation)?;
    for term in terms {
        shown_terms.push(values::value(term, limits, &mut count.nodes)?);
    }
    let raw_costs = raw
        .get("costs")
        .ok_or_else(|| invalid(Issue::MissingField, "native model costs"))?;
    let costs = if raw_costs.is_null() {
        None
    } else {
        Some(values::costs(raw_costs, limits)?)
    };
    Ok(Tail {
        terms: shown_terms,
        costs,
    })
}

/// One spelled atom: predicate, sign and typed arguments.
fn decode_atom(atom: &Json, limits: Limits, nodes: &mut usize) -> Result<Atom, Error> {
    let raw_arguments = values::array(&atom["arguments"], "native atom arguments")?;
    let predicate = Predicate::with_sign(
        values::string(&atom["predicate"], "native predicate")?,
        raw_arguments.len(),
        values::sign(&atom["sign"])?,
    )
    .map_err(Error::Atom)?;
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(raw_arguments.len())
        .map_err(|_| Error::Allocation)?;
    for argument in raw_arguments {
        arguments.push(values::value(argument, limits, nodes)?);
    }
    Atom::new(predicate, arguments).map_err(Error::Atom)
}
