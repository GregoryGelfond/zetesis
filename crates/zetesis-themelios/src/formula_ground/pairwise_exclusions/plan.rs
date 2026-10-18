//! Complete-component certificates over an exact finite edge population.
//!
//! Sorting and weighted union-find take O(R log R) work and O(R) leased capacity
//! for R pair occurrences. Duplicate and reversed edges count once. Every union
//! joins disjoint sets, so component sizes and edge counts are bounded by their
//! respective admitted buffers. The edge-count equality uses u128 arithmetic.

use super::{Buffer, Builder, FormulaFailure, RuleIr};
use crate::formula_support::{GroundingWork, sort};

#[derive(Clone, Copy)]
struct Vertex {
    node: usize,
    parent: usize,
    size: usize,
    edges: usize,
}

pub(super) struct Plan {
    vertices: Buffer<Vertex>,
}

impl Plan {
    /// On return every pair endpoint is a private vertex coordinate. Until then
    /// no caller may use this partially sorted/remapped private population.
    pub(super) fn new(
        pairs: &mut Buffer<[usize; 2]>,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Self, FormulaFailure> {
        let mut vertices = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        for pair in pairs.slice() {
            for &node in pair {
                vertices.push(
                    Vertex {
                        node,
                        parent: 0,
                        size: 1,
                        edges: 0,
                    },
                    builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                )?;
            }
        }
        sort::by(
            vertices.slice_mut(),
            GroundingWork::new(builder.limits, builder.counters, rule.location),
            |left, right, _| Ok(left.node.cmp(&right.node)),
        )?;
        let mut distinct = 0;
        for read in 0..vertices.len() {
            builder.work(rule.location)?;
            let mut vertex = vertices.slice()[read];
            if distinct == 0 || vertices.slice()[distinct - 1].node != vertex.node {
                builder.work(rule.location)?;
                vertex.parent = distinct;
                vertices.slice_mut()[distinct] = vertex;
                distinct += 1;
            }
        }
        while vertices.len() > distinct {
            builder.work(rule.location)?;
            vertices.pop();
        }
        sort::by(
            pairs.slice_mut(),
            GroundingWork::new(builder.limits, builder.counters, rule.location),
            |left, right, _| Ok(key(*left).cmp(&key(*right))),
        )?;
        let mut plan = Self { vertices };
        for pair in pairs.slice_mut() {
            for endpoint in pair.iter_mut() {
                *endpoint = plan.position(*endpoint, builder, rule)?;
            }
            plan.join(pair[0], pair[1], builder, rule)?;
        }
        let mut previous = None;
        for &pair in pairs.slice() {
            builder.work(rule.location)?;
            let edge = key(pair);
            if pair[0] != pair[1] && previous != Some(edge) {
                let root = plan.component(pair[0], builder, rule)?;
                builder.work(rule.location)?;
                plan.vertices.slice_mut()[root].edges += 1;
                previous = Some(edge);
            }
        }
        Ok(plan)
    }

    fn position(
        &self,
        node: usize,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        let mut low = 0;
        let mut high = self.vertices.len();
        while low < high {
            builder.work(rule.location)?;
            let middle = low + (high - low) / 2;
            match self.vertices.slice()[middle].node.cmp(&node) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        unreachable!("every endpoint supplied the sorted vertex population")
    }

    pub(super) fn component(
        &self,
        mut vertex: usize,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        loop {
            builder.work(rule.location)?;
            let parent = self.vertices.slice()[vertex].parent;
            if parent == vertex {
                return Ok(vertex);
            }
            vertex = parent;
        }
    }

    fn join(
        &mut self,
        left: usize,
        right: usize,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        let mut left = self.component(left, builder, rule)?;
        let mut right = self.component(right, builder, rule)?;
        if left != right {
            builder.work(rule.location)?;
            if self.vertices.slice()[left].size < self.vertices.slice()[right].size {
                std::mem::swap(&mut left, &mut right);
            }
            builder.work(rule.location)?;
            let right_size = self.vertices.slice()[right].size;
            self.vertices.slice_mut()[right].parent = left;
            self.vertices.slice_mut()[left].size += right_size;
        }
        Ok(())
    }

    pub(super) fn node(&self, vertex: usize) -> usize {
        self.vertices.slice()[vertex].node
    }

    pub(super) fn complete(&self, component: usize) -> bool {
        let vertex = self.vertices.slice()[component];
        let size = vertex.size as u128;
        size >= 3 && vertex.edges as u128 == size * (size - 1) / 2
    }

    /// Group all vertices once, rather than scan every vertex per component.
    pub(super) fn members(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        rule: &RuleIr,
    ) -> Result<Buffer<[usize; 2]>, FormulaFailure> {
        let mut members = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )?;
        for vertex in 0..self.vertices.len() {
            let component = self.component(vertex, builder, rule)?;
            members.push(
                [component, self.node(vertex)],
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?;
        }
        sort::by(
            members.slice_mut(),
            GroundingWork::new(builder.limits, builder.counters, rule.location),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        Ok(members)
    }
}

fn key([left, right]: [usize; 2]) -> [usize; 2] {
    [left.min(right), left.max(right)]
}
