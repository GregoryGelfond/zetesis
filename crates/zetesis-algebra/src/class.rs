//! Sealed kinds of interpretation classes used by solver execution.
//!
//! These markers distinguish answer-set search from reduct countermodel search.
//! A concrete theory, restriction family and candidate remain in their semantic
//! owners; a marker does not authenticate those runtime identities.

mod sealed {
    pub trait Sealed {}
}

/// A built-in kind of interpretation class, not a membership extension point.
pub trait Class: sealed::Sealed {}

/// Answer sets of the original theory that meet the active restrictions.
#[derive(Clone, Copy, Debug)]
pub enum AnswerSetsSought {}

/// Models of a candidate's reduct strictly inside that candidate.
#[derive(Clone, Copy, Debug)]
pub enum Countermodels {}

impl sealed::Sealed for AnswerSetsSought {}
impl sealed::Sealed for Countermodels {}
impl Class for AnswerSetsSought {}
impl Class for Countermodels {}
