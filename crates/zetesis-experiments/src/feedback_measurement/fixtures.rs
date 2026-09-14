use serde::Serialize;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use super::Error;

/// Fixed finite original-theory population; atom positions are semantic IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Case {
    /// No atoms, formulas or asserted roots.
    Empty,
    /// Asserted falsum over the empty universe.
    False,
    /// One atomic choice, with two comparable stable interpretations.
    Choice,
    /// Four independent choices, with all sixteen interpretations stable.
    Choices,
    /// Six positive self-loops, with only the empty interpretation stable.
    Loops,
    /// a or b, plus independent choices over c and d.
    Mixed,
    /// The nested implication (a implies b) implies c.
    Nested,
    /// Repeated shared a-or-b roots and an unused universe atom c.
    Shared,
}
impl Case {
    pub(super) const ALL: [Self; 8] = [Self::Empty, Self::False, Self::Choice,
        Self::Choices, Self::Loops, Self::Mixed, Self::Nested, Self::Shared];

    pub(super) fn theory(self) -> Result<Theory, Error> {
        let mut nodes = super::reserve(64)?;
        let mut roots = super::reserve(16)?;
        let atoms = match self {
            Self::Empty | Self::False => 0,
            Self::Choice => 1,
            Self::Choices | Self::Mixed => 4,
            Self::Loops => 6,
            Self::Nested | Self::Shared => 3,
        };
        for atom in 0..atoms {
            nodes.push(Node::Atom(atom));
        }
        match self {
            Self::Empty => {},
            Self::False => { nodes.push(Node::False); roots.push(0); },
            Self::Choice | Self::Choices => {
                let falsum = nodes.len();
                nodes.push(Node::False);
                for atom in 0..atoms {
                    choice(&mut nodes, &mut roots, atom, falsum);
                }
            }
            Self::Loops => {
                for atom in 0..atoms {
                    roots.push(nodes.len());
                    nodes.push(Node::Implies(atom, atom));
                }
            }
            Self::Mixed => {
                nodes.push(Node::Or(0, 1)); roots.push(4);
                nodes.push(Node::False);
                choice(&mut nodes, &mut roots, 2, 5);
                choice(&mut nodes, &mut roots, 3, 5);
            }
            Self::Nested => {
                nodes.push(Node::Implies(0, 1));
                nodes.push(Node::Implies(3, 2)); roots.push(4);
            }
            Self::Shared => { nodes.push(Node::Or(0, 1)); roots.extend([3, 3]); },
        }
        Theory::new(atoms, nodes, roots,
            AdmissionLimits { max_atoms: 6, max_nodes: 64, max_roots: 16 })
            .map_err(Error::Admission)
    }
}

fn choice(nodes: &mut Vec<Node>, roots: &mut Vec<usize>, atom: usize, falsum: usize) {
    let negated = nodes.len();
    nodes.push(Node::Implies(atom, falsum));
    roots.push(nodes.len());
    nodes.push(Node::Or(atom, negated));
}

pub(super) fn shape(theory: &Theory) -> Result<(), Error> {
    if theory.atom_count() > 6 || theory.nodes().len() > 64 || theory.roots().len() > 16 {
        return Err(Error::Configuration("feedback owners require at most 6 atoms, 64 nodes and 16 roots"));
    }
    Ok(())
}

pub(super) fn interpretation(theory: &Theory, bits: u64) -> Result<Interpretation, Error> {
    Interpretation::new(theory, (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0))
        .map_err(Error::Admission)
}

pub(super) fn bits(interpretation: &Interpretation) -> u64 {
    interpretation.atoms().fold(0, |bits, atom| bits | (1 << atom))
}
