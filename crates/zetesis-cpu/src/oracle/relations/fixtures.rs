//! Owned descriptions are test ingress only; production rows retain canonical IDs.

use super::{Catalogs, Work, charge, storage};
use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Program, Template, Term};

pub(in crate::oracle) fn program(atoms: &[Atom]) -> Program {
    Program::new(
        atoms
            .iter()
            .map(|atom| {
                let head = AtomPattern::new(
                    atom.predicate().clone(),
                    atom.values().iter().cloned().map(Term::Constant).collect(),
                )
                .unwrap();
                Template::new(Some(head), vec![], vec![], vec![], vec![])
            })
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

pub(in crate::oracle) fn catalogs(atoms: &[Atom], work: &mut Work<'_>) -> Catalogs {
    let mut catalogs = Catalogs::default();
    catalogs.bind_program(&program(atoms), work).unwrap();
    catalogs
}

pub(in crate::oracle) fn intern(
    catalogs: &mut Catalogs,
    atom: &Atom,
    work: &mut Work<'_>,
) -> usize {
    let other = catalogs.metadata_bytes() + catalogs.overhead;
    let allowance = storage::atom_limits(work, other).unwrap();
    let authority = catalogs.authority.as_mut().unwrap();
    authority.restart_storage_peak();
    let id = authority
        .entry_atom_with(atom, allowance, || charge(work, 1))
        .unwrap()
        .insert_with(allowance, || charge(work, 1))
        .unwrap();
    storage::record(work, other + authority.storage_peak_bytes()).unwrap();
    catalogs.commit(0, work).unwrap();
    id
}

pub(in crate::oracle) fn insert(catalogs: &mut Catalogs, atom: &Atom, work: &mut Work<'_>) {
    let id = intern(catalogs, atom, work);
    catalogs.insert(id, 0, work).unwrap();
}
