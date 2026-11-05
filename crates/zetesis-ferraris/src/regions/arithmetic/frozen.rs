//! A candidate's inner subjects: parity rows and count bounds of its
//! countermodel class, recognized by probing the closed query root under the
//! candidate's frozen reduct.
//!
//! Proposals are syntactic (`proposals`); nothing rests on them. Every row's
//! provenance is a refuted probe: a frozen narrowing from a copy of the closed
//! root with exactly a guard and a pattern decided, which is the premise of
//! `Zetesis.Algebra.entailed_by_refutations`.

mod proposals;

#[cfg(test)]
mod tests;
