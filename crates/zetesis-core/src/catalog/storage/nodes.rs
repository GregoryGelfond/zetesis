//! Typed term lanes shared by canonical authorities. Scalar payloads stay u32;
//! only compound names and child coordinates depend on the owning authority.
//! Child prefix ends count expanded occurrences, never distinct DAG nodes.

use std::ops::Range;

use super::{Failure, Fault, budget, control::Work};
use crate::catalog::Limits;
use crate::{Sign, ValueError, ValueNodeRef, ValueResource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Kind {
    Infimum,
    Number,
    String,
    Symbol,
    Function,
    Tuple,
    Supremum,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Measures {
    pub(super) nodes: usize,
    pub(super) depth: usize,
    pub(super) canonical: usize,
    pub(super) rendered: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Compound<Name> {
    pub(super) name: Option<Name>,
    pub(super) sign: Sign,
    pub(super) children: Range<usize>,
    pub(super) measures: Measures,
}

/// Five existing vector headers; enclosing owners account this inline header.
/// A scalar text payload is an authority-local u32 text coordinate. Compound
/// names use Name directly, allowing a different coordinate without widening
/// every scalar. Child and Name are identity coordinates, never owned payload.
#[derive(Debug)]
pub(super) struct Nodes<Child, Name> {
    pub(super) kinds: Vec<Kind>,
    pub(super) payloads: Vec<u32>,
    pub(super) compounds: Vec<Compound<Name>>,
    pub(super) children: Vec<Child>,
    pub(super) ends: Vec<usize>,
}

impl<Child, Name> Nodes<Child, Name> {
    pub(super) fn new() -> Self {
        Self {
            kinds: Vec::new(),
            payloads: Vec::new(),
            compounds: Vec::new(),
            children: Vec::new(),
            ends: Vec::new(),
        }
    }

    /// Allocation capacities only; the surrounding owner already holds Self.
    pub(super) fn buffer_bytes(&self) -> u128 {
        budget::capacity(&self.kinds)
            + budget::capacity(&self.payloads)
            + budget::capacity(&self.compounds)
            + budget::capacity(&self.children)
            + budget::capacity(&self.ends)
    }

    pub(super) fn compound(&self, local: usize) -> Option<&Compound<Name>> {
        matches!(self.kinds[local], Kind::Function | Kind::Tuple)
            .then(|| &self.compounds[self.payloads[local] as usize])
    }

    pub(super) fn child_end(&self, local: usize, index: usize) -> Option<usize> {
        let compound = self.compound(local)?;
        (index < compound.children.len()).then(|| self.ends[compound.children.start + index])
    }

    pub(super) fn measures(&self, local: usize, text: impl Fn(u32) -> (usize, usize)) -> Measures {
        let payload = self.payloads[local];
        let (canonical, rendered) = match self.kinds[local] {
            Kind::Function | Kind::Tuple => return self.compounds[payload as usize].measures,
            Kind::Infimum | Kind::Supremum => (1, 4),
            Kind::Number => {
                let number = i32::from_ne_bytes(payload.to_ne_bytes());
                let digits = number.unsigned_abs().checked_ilog10().unwrap_or(0) as usize + 1;
                (5, digits + usize::from(number < 0))
            }
            Kind::String | Kind::Symbol => {
                let (bytes, quoted) = text(payload);
                // Admission checked this scalar sum before publication.
                (
                    9 + bytes,
                    if self.kinds[local] == Kind::String {
                        quoted
                    } else {
                        bytes
                    },
                )
            }
        };
        Measures {
            nodes: 1,
            depth: 1,
            canonical,
            rendered,
        }
    }
}

impl<Child: Copy, Name: Copy> Nodes<Child, Name> {
    pub(super) fn descriptor<'a>(
        &self,
        local: usize,
        text: impl Fn(u32) -> &'a str,
        name: impl Fn(Name) -> &'a str,
    ) -> ValueNodeRef<'a> {
        let payload = self.payloads[local];
        match self.kinds[local] {
            Kind::Infimum => ValueNodeRef::Infimum,
            Kind::Supremum => ValueNodeRef::Supremum,
            Kind::Number => ValueNodeRef::Number(i32::from_ne_bytes(payload.to_ne_bytes())),
            Kind::String => ValueNodeRef::String(text(payload)),
            Kind::Symbol => ValueNodeRef::Symbol(text(payload)),
            Kind::Function | Kind::Tuple => {
                let compound = &self.compounds[payload as usize];
                let arity = compound.children.len();
                compound
                    .name
                    .map_or(ValueNodeRef::Tuple { arity }, |coordinate| {
                        ValueNodeRef::Function {
                            name: name(coordinate),
                            sign: compound.sign,
                            arity,
                        }
                    })
            }
        }
    }

    pub(super) fn child(&self, local: usize, index: usize) -> Option<Child> {
        let compound = self.compound(local)?;
        (index < compound.children.len()).then(|| self.children[compound.children.start + index])
    }
}

pub(super) fn normalized(descriptor: ValueNodeRef<'_>) -> Result<ValueNodeRef<'_>, Fault> {
    match descriptor {
        ValueNodeRef::Function { name: "", .. } => Err(Fault::Shape),
        ValueNodeRef::Function {
            name,
            sign: Sign::Positive,
            arity: 0,
        } => Ok(ValueNodeRef::Symbol(name)),
        descriptor => Ok(descriptor),
    }
}

/// Read each child measure only after its permit; repeated coordinates are
/// expanded repeatedly. The resolver owns scope/prefix validation, and this
/// shared arithmetic owns overflow and logical-size accumulation.
pub(super) fn node_measures<Child: Copy, E>(
    descriptor: ValueNodeRef<'_>,
    children: &[Child],
    mut measure: impl FnMut(Child) -> Measures,
    work: &mut Work<'_, E>,
) -> Result<Measures, Failure<E>> {
    let mut result = descriptor_measures(descriptor, work)?;
    for child in children {
        work.step()?;
        let child = measure(*child);
        work.step()?;
        result.nodes = result
            .nodes
            .checked_add(child.nodes)
            .ok_or(Fault::Overflow)?;
        result.depth = result
            .depth
            .max(child.depth.checked_add(1).ok_or(Fault::Overflow)?);
        result.canonical = result
            .canonical
            .checked_add(child.canonical)
            .ok_or(Fault::Overflow)?;
        result.rendered = result
            .rendered
            .checked_add(child.rendered)
            .ok_or(Fault::Overflow)?;
    }
    Ok(result)
}

impl Measures {
    pub(super) fn check(self, limits: Limits) -> Result<(), Fault> {
        ceiling(ValueResource::Nodes, self.nodes as u128, limits.max_nodes)?;
        ceiling(ValueResource::Depth, self.depth as u128, limits.max_depth)?;
        ceiling(
            ValueResource::Bytes,
            self.canonical as u128 + self.rendered as u128,
            limits.max_bytes,
        )
    }
}

pub(super) fn descriptor_measures<E>(
    descriptor: ValueNodeRef<'_>,
    work: &mut Work<'_, E>,
) -> Result<Measures, Failure<E>> {
    work.step()?;
    let canonical = usize::try_from(descriptor.canonical_bytes()).map_err(|_| Fault::Overflow)?;
    let rendered = if let ValueNodeRef::String(text) = descriptor {
        work.quoted_bytes(text)?
    } else {
        usize::try_from(descriptor.rendered_bytes()).map_err(|_| Fault::Overflow)?
    };
    Ok(Measures {
        nodes: 1,
        depth: 1,
        canonical,
        rendered,
    })
}

pub(super) const fn arity(descriptor: ValueNodeRef<'_>) -> usize {
    match descriptor {
        ValueNodeRef::Function { arity, .. } | ValueNodeRef::Tuple { arity } => arity,
        _ => 0,
    }
}

pub(super) fn ceiling(resource: ValueResource, observed: u128, limit: usize) -> Result<(), Fault> {
    if observed > limit as u128 {
        return Err(ValueError::Limit {
            resource,
            observed,
            limit,
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn repeated_children_contribute_repeated_expanded_measures() {
        let child = Measures {
            nodes: 3,
            depth: 2,
            canonical: 17,
            rendered: 8,
        };
        let mut before = || Ok::<_, ()>(());
        let result = node_measures(
            ValueNodeRef::Tuple { arity: 2 },
            &[0, 0],
            |_| child,
            &mut Work::new(&mut before),
        )
        .unwrap();
        assert_eq!(
            (
                result.nodes,
                result.depth,
                result.canonical,
                result.rendered
            ),
            (7, 3, 43, 19)
        );
    }

    #[test]
    fn child_resolution_follows_its_work_permit() {
        for cutoff in 0..5 {
            let reads = Cell::new(0);
            let mut permitted = 0;
            let mut before = || {
                if permitted == cutoff {
                    return Err(cutoff);
                }
                permitted += 1;
                Ok(())
            };
            let result = node_measures(
                ValueNodeRef::Tuple { arity: 2 },
                &[0, 0],
                |_| {
                    reads.set(reads.get() + 1);
                    Measures {
                        nodes: 1,
                        depth: 1,
                        canonical: 5,
                        rendered: 1,
                    }
                },
                &mut Work::new(&mut before),
            );
            assert!(matches!(result, Err(Failure::Stopped(stopped)) if stopped == cutoff));
            assert_eq!(reads.get(), cutoff / 2);
        }
    }

    #[test]
    fn expanded_node_overflow_is_a_typed_refusal() {
        let mut before = || Ok::<_, ()>(());
        let result = node_measures(
            ValueNodeRef::Tuple { arity: 1 },
            &[0],
            |_| Measures {
                nodes: usize::MAX,
                depth: 1,
                canonical: 1,
                rendered: 1,
            },
            &mut Work::new(&mut before),
        );
        assert!(matches!(result, Err(Failure::Storage(Fault::Overflow))));
    }

    #[test]
    fn positive_nullary_constructor_normalizes_to_symbol() {
        assert_eq!(
            normalized(ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 0,
            })
            .unwrap(),
            ValueNodeRef::Symbol("f")
        );
    }
}
