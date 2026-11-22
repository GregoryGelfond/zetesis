//! Least fixpoint of a monotone atom transform.

use super::relation::Atoms;
use super::union::union;

/// The least pre-fixpoint of `transform`, starting from the empty set.
///
/// Each round computes `Y = transform(X)`, then `X := union(X, Y)`, until a
/// round adds no atom. This is a permitted realization of `Least(T_M)`
/// (including a later semi-naive form). The transform sees the same account
/// as the unions that close each round. One unit is charged at the start of
/// every round, including the first and the stabilizing last round.
///
/// # Errors
/// Returns the first callback or transform refusal. No partial fixpoint is
/// returned.
///
/// # Panics
/// Panics if a transform result has a width other than `width`, or if
/// `width` is zero.
pub fn least<E, A, F>(width: usize, mut transform: F, account: &mut A) -> Result<Atoms, E>
where
    A: FnMut(u64) -> Result<(), E>,
    F: FnMut(&Atoms, &mut A) -> Result<Atoms, E>,
{
    let mut current = Atoms::empty(width);
    loop {
        account(1)?;
        let delta = transform(&current, account)?;
        assert_eq!(
            delta.width(),
            width,
            "transform results match the seed width"
        );
        let next = union(&current, &delta, account)?;
        if next.len() == current.len() {
            return Ok(next);
        }
        current = next;
    }
}
