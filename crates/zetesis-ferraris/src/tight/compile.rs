use std::mem::size_of;

use super::{
    TightAttempt, TightError, TightPlan, TightPlanLimits, TightPlanStatistics, TightProducer,
    TightProducerKind, TightResource, Work, bytes, filled, reserve,
};
use crate::{Node, Theory};
use zetesis_cpu::Cancellation;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Body {
    Opaque,
    Frozen,
    Positive,
}

#[derive(Clone, Copy)]
struct Edge {
    target: usize,
    next: usize,
    strict: bool,
}

const END: usize = usize::MAX;

impl TightPlan {
    /// Extract all original roots and derive a positive rank certificate.
    ///
    /// Accepted roots are atomic facts, ordinary atomic-head implications,
    /// atomic choices `h ∨ not h` (optionally with a body), falsum and default
    /// negations. Bodies admit atoms, falsum, conjunction, disjunction and
    /// default negation; a negation freezes its whole interior. General
    /// disjunctions, unnegated body implications and positive cycles fall back.
    /// Every root is inspected, including constraints and candidate guards.
    ///
    /// # Errors
    /// Returns explicit shape/cycle refusal, resource limits or shared control
    /// and allocation failures. A failure never creates a partial certificate.
    pub fn compile(
        theory: &Theory,
        limits: TightPlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, TightError> {
        Self::compile_accounted(theory, limits, cancellation).result
    }

    /// Derive the same certificate as [`Self::compile`], retaining charged work
    /// on every result, including shape refusal, cancellation and work limits.
    #[must_use]
    pub fn compile_accounted(
        theory: &Theory,
        limits: TightPlanLimits,
        cancellation: &Cancellation,
    ) -> TightAttempt<Self> {
        let mut work = Work {
            used: 0,
            max: limits.max_work,
            cancellation,
        };
        let result = build(theory, None, limits, &mut work);
        TightAttempt {
            result,
            work: work.used,
        }
    }

    /// Check an externally proposed atom rank against the same complete root
    /// extraction. Ranks must be less than the atom count; gaps are permitted.
    /// Source-analysis ranks are only proposals until this check succeeds.
    ///
    /// # Errors
    /// Has the errors of [`Self::compile`], plus malformed or invalid ranks.
    pub fn certify(
        theory: &Theory,
        ranks: &[usize],
        limits: TightPlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, TightError> {
        let mut work = Work {
            used: 0,
            max: limits.max_work,
            cancellation,
        };
        build(theory, Some(ranks), limits, &mut work)
    }
}

fn classify(theory: &Theory, work: &mut Work<'_>) -> Result<Vec<Body>, TightError> {
    let mut classes = reserve(theory.nodes().len())?;
    for node in theory.nodes() {
        work.tick()?;
        classes.push(match *node {
            Node::Atom(_) => Body::Positive,
            Node::False => Body::Frozen,
            Node::Implies(_, b) if theory.nodes()[b] == Node::False => Body::Frozen,
            Node::Implies(_, _) => Body::Opaque,
            Node::And(a, b) | Node::Or(a, b) => match (classes[a], classes[b]) {
                (Body::Opaque, _) | (_, Body::Opaque) => Body::Opaque,
                (Body::Frozen, Body::Frozen) => Body::Frozen,
                _ => Body::Positive,
            },
        });
    }
    Ok(classes)
}

fn producer(
    theory: &Theory,
    root: usize,
    classes: &[Body],
) -> Result<Option<TightProducer>, TightError> {
    let node = theory.nodes()[root];
    let (body, head) = match node {
        Node::False => return Ok(None),
        Node::Implies(_, b) if theory.nodes()[b] == Node::False => return Ok(None),
        Node::Implies(a, b) => (Some(a), b),
        _ => (None, root),
    };
    let node = theory.nodes()[head];
    let (head, kind) = match node {
        Node::Atom(atom) => (atom, TightProducerKind::Normal),
        _ => (
            crate::atomic_choice::atom(theory, head).ok_or(TightError::UnsupportedRoot { root })?,
            TightProducerKind::Choice,
        ),
    };
    if let Some(body) = body
        && classes[body] == Body::Opaque
    {
        return Err(TightError::UnsupportedBody { root, body });
    }
    Ok(Some(TightProducer {
        head,
        body,
        root,
        kind,
    }))
}

fn extract(
    theory: &Theory,
    classes: &[Body],
    limits: TightPlanLimits,
    work: &mut Work<'_>,
) -> Result<Vec<TightProducer>, TightError> {
    let mut count = 0usize;
    for &root in theory.roots() {
        work.tick()?;
        if producer(theory, root, classes)?.is_some() {
            if count == limits.max_producers {
                return Err(TightError::Limit(TightResource::Producers));
            }
            count += 1;
        }
    }
    bytes(
        theory.nodes().len() as u128 * size_of::<Body>() as u128
            + count as u128 * size_of::<TightProducer>() as u128,
        limits.max_bytes,
    )?;
    let mut producers = reserve(count)?;
    for &root in theory.roots() {
        work.tick()?;
        if let Some(producer) = producer(theory, root, classes)? {
            producers.push(producer);
        }
    }
    Ok(producers)
}

fn each_dependency(
    theory: &Theory,
    classes: &[Body],
    producers: &[TightProducer],
    work: &mut Work<'_>,
    mut visit: impl FnMut(usize, usize, bool) -> Result<(), TightError>,
) -> Result<(), TightError> {
    let offset = theory.atom_count();
    for (index, node) in theory.nodes().iter().enumerate() {
        work.tick()?;
        match *node {
            Node::Atom(atom) => visit(atom, offset + index, false)?,
            Node::And(a, b) | Node::Or(a, b) if classes[index] == Body::Positive => {
                // Duplicate operands contribute duplicate edges and matching
                // indegrees. Neither is silently deduplicated on its own.
                for child in [a, b] {
                    if classes[child] == Body::Positive {
                        visit(offset + child, offset + index, false)?;
                    }
                }
            }
            _ => {}
        }
    }
    for producer in producers {
        work.tick()?;
        if let Some(body) = producer.body
            && classes[body] == Body::Positive
        {
            visit(offset + body, producer.head, true)?;
        }
    }
    Ok(())
}

fn build(
    theory: &Theory,
    proposed: Option<&[usize]>,
    limits: TightPlanLimits,
    work: &mut Work<'_>,
) -> Result<TightPlan, TightError> {
    work.cancellation.poll()?;
    bytes(
        theory.nodes().len() as u128 * size_of::<Body>() as u128,
        limits.max_bytes,
    )?;
    let classes = classify(theory, work)?;
    let producers = extract(theory, &classes, limits, work)?;
    let mut dependencies = 0usize;
    each_dependency(theory, &classes, &producers, work, |_, _, _| {
        if dependencies == limits.max_dependencies {
            return Err(TightError::Limit(TightResource::Dependencies));
        }
        dependencies += 1;
        Ok(())
    })?;
    let vertices = theory
        .atom_count()
        .checked_add(theory.nodes().len())
        .ok_or(TightError::Limit(TightResource::Bytes))?;
    let resident_bytes = bytes(
        producers.len() as u128 * size_of::<TightProducer>() as u128
            + theory.atom_count() as u128 * size_of::<usize>() as u128,
        limits.max_bytes,
    )?;
    let graph_bytes = if proposed.is_some() {
        0
    } else {
        dependencies as u128 * size_of::<Edge>() as u128
            + vertices as u128 * size_of::<usize>() as u128 * 4
    };
    let construction_bytes = bytes(
        u128::from(resident_bytes)
            + theory.nodes().len() as u128 * (size_of::<Body>() + size_of::<usize>()) as u128
            + graph_bytes,
        limits.max_bytes,
    )?;
    let ranks = if let Some(ranks) = proposed {
        validate_ranks(theory, &classes, &producers, ranks, work)?;
        let mut owned = reserve(ranks.len())?;
        owned.extend_from_slice(ranks);
        owned
    } else {
        derive_ranks(theory, &classes, &producers, vertices, dependencies, work)?
    };
    Ok(TightPlan {
        theory: theory.clone(),
        producers,
        ranks,
        statistics: TightPlanStatistics {
            work: work.used,
            dependencies,
            construction_bytes,
            resident_bytes,
        },
    })
}

fn derive_ranks(
    theory: &Theory,
    classes: &[Body],
    producers: &[TightProducer],
    vertices: usize,
    dependency_count: usize,
    work: &mut Work<'_>,
) -> Result<Vec<usize>, TightError> {
    let mut links = filled(vertices, END)?;
    let mut indegrees = filled(vertices, 0usize)?;
    let mut rank = filled(vertices, 0usize)?;
    let mut queue = reserve(vertices)?;
    let mut edges = reserve(dependency_count)?;
    each_dependency(
        theory,
        classes,
        producers,
        work,
        |source, target, strict| {
            edges.push(Edge {
                target,
                next: links[source],
                strict,
            });
            links[source] = edges.len() - 1;
            indegrees[target] += 1;
            Ok(())
        },
    )?;
    for (index, &degree) in indegrees.iter().enumerate() {
        work.tick()?;
        if degree == 0 {
            queue.push(index);
        }
    }
    let mut cursor = 0;
    while cursor < queue.len() {
        work.tick()?;
        let source = queue[cursor];
        cursor += 1;
        let mut edge_index = links[source];
        while edge_index != END {
            work.tick()?;
            let edge = edges[edge_index];
            rank[edge.target] = rank[edge.target].max(rank[source] + usize::from(edge.strict));
            indegrees[edge.target] -= 1;
            if indegrees[edge.target] == 0 {
                queue.push(edge.target);
            }
            edge_index = edge.next;
        }
    }
    if queue.len() != vertices {
        for (atom, degree) in indegrees[..theory.atom_count()].iter().enumerate() {
            work.tick()?;
            if *degree != 0 {
                return Err(TightError::PositiveCycle { atom });
            }
        }
        // Formula edges are topological, so a remaining cycle necessarily
        // passes through an atom. Keep a typed failure if that invariant fails.
        return Err(zetesis_cpu::Stop::InvalidProgram.into());
    }
    let mut atoms = reserve(theory.atom_count())?;
    atoms.extend_from_slice(&rank[..theory.atom_count()]);
    validate_ranks(theory, classes, producers, &atoms, work)?;
    Ok(atoms)
}

fn validate_ranks(
    theory: &Theory,
    classes: &[Body],
    producers: &[TightProducer],
    ranks: &[usize],
    work: &mut Work<'_>,
) -> Result<(), TightError> {
    if ranks.len() != theory.atom_count() {
        return Err(TightError::RankShape);
    }
    for &rank in ranks {
        work.tick()?;
        if rank >= ranks.len() {
            return Err(TightError::RankShape);
        }
    }
    let mut maximum = reserve::<usize>(theory.nodes().len())?;
    for (index, node) in theory.nodes().iter().enumerate() {
        work.tick()?;
        maximum.push(match *node {
            Node::Atom(atom) => ranks[atom],
            Node::And(a, b) | Node::Or(a, b) if classes[index] == Body::Positive => {
                maximum[a].max(maximum[b])
            }
            _ => 0,
        });
    }
    for producer in producers {
        work.tick()?;
        if let Some(body) = producer.body
            && classes[body] == Body::Positive
            && maximum[body] >= ranks[producer.head]
        {
            return Err(TightError::RankOrder {
                head: producer.head,
            });
        }
    }
    Ok(())
}
