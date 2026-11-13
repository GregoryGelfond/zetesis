//! Producers that cannot support their own head.
//!
//! A body requires the atoms that are its conjuncts: an atom body requires its
//! atom, and a conjunction requires each operand that is an atom. An atom is
//! derived only through `a` when it has producers and every one of them has a
//! body requiring `a`. A producer whose one head is `a` is circular when its
//! body requires `a` itself or an atom derived only through `a`: its body can
//! hold only where its head already does.
//!
//! No answer set rests on a circular producer. Every atom of an answer set has
//! an enabled producer, with that atom as its only true head, whose body
//! requires neither the atom nor any atom derived only through it
//! (`ExternalSupport.answer_set_noncircular_support`). The support cut and the
//! positive-dependency test of an original theory therefore leave circular
//! producers out. Every root stays in the theory and propagates as the
//! implication it is; nothing here rewrites a rule or decides membership.
//!
//! Only producers with one head are marked. A producer with several heads may
//! be circular for one of them and a support of another, so it is never
//! marked: the restriction stays unused there, which is sound. Operands that
//! are themselves conjunctions are not opened either; a body then requires
//! fewer atoms than the law would grant it, and fewer producers are marked.
//!
//! The pass lists each distinct body's required atoms once and turns that list
//! around in one transient adjacency. For each head it stamps the bodies
//! requiring that head; a required atom is then derived only through the head
//! exactly when every producer of the atom has a stamped body. Each head and
//! atom pair is decided once, and a decision ends at the first producer whose
//! body is not stamped. Every node, head, operand, pair, stamp and producer
//! read is one unit of the caller's work; a body shared by several producers
//! is read once for each, as the ground rules it stands for would be. Placing
//! the pairs in their adjacency and zeroing the stamp, memo and mark vectors
//! are not charged, as the inventory's own adjacencies are not.
//!
//! The cost is not linear in the extraction's visits. The extraction visits
//! a conjunction once, whatever its width; the pass reads its operands once
//! for every producer of the body, `k * m` reads for `m` atoms under `k`
//! rules, and each head and required atom pair reads the atom's producers up
//! to the first unstamped one. The extraction therefore gives the pass an
//! allowance, a fixed multiple of its own visits within what its ceiling
//! leaves, and keeps an unmarked inventory when the pass exhausts it
//! ([`super::Producers::mark`]). No mark is ever needed for soundness.

use zetesis_algebra::words::Words;
use zetesis_algebra::{Failure, Quota, Stop};

use super::{Adjacency, Meter, NodeView, Producer, flag_words};
use crate::Theory;

#[cfg(test)]
mod tests;

/// No producer marked, for an inventory of `producers` rules.
///
/// # Errors
/// An allocation failure.
pub(super) fn none(producers: usize) -> Result<Words<u64>, Stop> {
    Ok(Words::<u64>::new(zeros(flag_words(producers))?))
}

/// One mark per circular producer, in the inventory's producer order, and
/// whether any producer is marked.
///
/// # Errors
/// The caller's stop when its work is refused, or an allocation failure. No
/// partial marking is returned.
pub(super) fn mark<Q: Quota + ?Sized>(
    theory: &Theory,
    rules: &[Producer],
    by_head: &Adjacency,
    by_body: &Adjacency,
    work: &mut Meter<'_, Q>,
) -> Result<(Words<u64>, bool), Failure<Q::Error>> {
    let mut marks = none(rules.len())?;
    let mut any = false;
    let requirements = Requirements::read(theory, by_body, work)?;
    if requirements.pairs == 0 {
        return Ok((marks, any));
    }
    // The head whose requiring bodies are stamped, as `head + 1`.
    let mut stamped = zeros(theory.nodes().len())?;
    // Per atom, the head it was last decided for and whether it is derived
    // only through that head.
    let mut decided = zeros(theory.atom_count())?;
    let mut derived = Words::<u64>::new(zeros(flag_words(theory.atom_count()))?);
    for head in 0..by_head.len() {
        work.tick()?;
        let mut candidates = false;
        for &row in &by_head[head] {
            work.tick()?;
            if rules[row].heads.len() == 1 && rules[row].body.is_some() {
                candidates = true;
                break;
            }
        }
        if !candidates {
            continue;
        }
        let stamp = head + 1;
        for &body in &requirements.bodies[head] {
            work.tick()?;
            stamped[body] = stamp;
        }
        for &row in &by_head[head] {
            work.tick()?;
            let producer = &rules[row];
            let (Some(body), [_]) = (producer.body, producer.heads.as_slice()) else {
                continue;
            };
            let mut circular = false;
            for atom in required(theory, body) {
                work.tick()?;
                let atom = atom?;
                if atom == head {
                    circular = true;
                    break;
                }
                if decided[atom] != stamp {
                    decided[atom] = stamp;
                    let only = only_through(&by_head[atom], rules, &stamped, stamp, work)?;
                    if only {
                        derived.insert(atom);
                    } else {
                        derived.remove(atom);
                    }
                }
                if derived.contains(atom) {
                    circular = true;
                    break;
                }
            }
            if circular {
                marks.insert(row);
                any = true;
            }
        }
    }
    Ok((marks, any))
}

/// Whether an atom has producers and the body of every one of them is stamped
/// for the head under examination.
fn only_through<Q: Quota + ?Sized>(
    producers: &[usize],
    rules: &[Producer],
    stamped: &[usize],
    stamp: usize,
    work: &mut Meter<'_, Q>,
) -> Result<bool, Failure<Q::Error>> {
    for &row in producers {
        work.tick()?;
        if rules[row].body.is_none_or(|body| stamped[body] != stamp) {
            return Ok(false);
        }
    }
    Ok(!producers.is_empty())
}

/// The atoms a body requires: itself when it is an atom, or its operands that
/// are atoms when it is a conjunction. Repeated operands repeat.
fn required(theory: &Theory, body: usize) -> impl Iterator<Item = Result<usize, Stop>> + '_ {
    let nodes = theory.view();
    let (own, operands) = match nodes.node(body) {
        Ok(NodeView::Atom(atom)) => (Some(Ok(atom)), &[][..]),
        Ok(NodeView::And(operands)) => (None, operands),
        Ok(_) => (None, &[][..]),
        Err(_) => (Some(Err(Stop::InvalidProgram)), &[][..]),
    };
    own.into_iter().chain(
        operands
            .iter()
            .filter_map(move |&operand| match nodes.node(operand) {
                Ok(NodeView::Atom(atom)) => Some(Ok(atom)),
                Ok(_) => None,
                Err(_) => Some(Err(Stop::InvalidProgram)),
            }),
    )
}

/// Per atom, the distinct producer bodies that require it.
struct Requirements {
    bodies: Adjacency,
    pairs: usize,
}

impl Requirements {
    fn read<Q: Quota + ?Sized>(
        theory: &Theory,
        by_body: &Adjacency,
        work: &mut Meter<'_, Q>,
    ) -> Result<Self, Failure<Q::Error>> {
        let mut pairs = Vec::new();
        for body in 0..by_body.len() {
            work.tick()?;
            if by_body[body].is_empty() {
                continue;
            }
            for atom in required(theory, body) {
                work.tick()?;
                pairs.try_reserve(1).map_err(|_| Stop::Allocation)?;
                pairs.push((atom?, body));
            }
        }
        let bodies = Adjacency::build(theory.atom_count(), pairs.iter().copied())?;
        Ok(Self {
            bodies,
            pairs: pairs.len(),
        })
    }
}

fn zeros<T: Clone + Default>(length: usize) -> Result<Vec<T>, Stop> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    values.resize(length, T::default());
    Ok(values)
}
