use super::*;
use crate::{ExtremaMeasure, ExtremumRow};

struct Boolean<'a> {
    values: &'a [(bool, bool)],
    work: usize,
    limit: usize,
    gates: usize,
    gate_limit: usize,
    reads: Vec<usize>,
    constants: Vec<bool>,
}

impl<'a> Boolean<'a> {
    fn new(values: &'a [(bool, bool)]) -> Self {
        Self {
            values,
            work: 0,
            limit: usize::MAX,
            gates: 0,
            gate_limit: usize::MAX,
            reads: Vec::new(),
            constants: Vec::new(),
        }
    }
}

impl Sink for Boolean<'_> {
    type Value = (bool, bool);
    type Error = Error;
    fn tick(&mut self) -> Result<(), Error> {
        if self.work == self.limit {
            return Err(Error::Capacity);
        }
        self.work += 1;
        Ok(())
    }
    fn constant(&mut self, value: bool) -> Result<Self::Value, Error> {
        self.constants.push(value);
        Ok((value, value))
    }
    fn condition(&mut self, index: usize) -> Result<Self::Value, Error> {
        self.reads.push(index);
        self.values.get(index).copied().ok_or(Error::Invalid)
    }
    fn gate(
        &mut self,
        disjunction: bool,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Error> {
        assert!(disjunction);
        if self.gates == self.gate_limit {
            return Err(Error::Invalid);
        }
        self.gates += 1;
        let original = left.0 || right.0;
        Ok((original, original && (left.1 || right.1)))
    }
}

fn view<'a>(ranks: &'a [u64], rule: &'a ExtremumCutRule) -> ExtremumCut<'a> {
    ExtremumCut {
        row: ExtremumRow {
            conditions: &[0, 0, 1, 2, 3],
            ranks,
            rank_count: 5,
        },
        rule,
    }
}

#[test]
fn cut_transport_preserves_arbitrary_original_and_frozen_worlds() {
    for candidate in 0..4 {
        for tested in 0..4 {
            let a = (candidate & 1 != 0, candidate & tested & 1 != 0);
            let b = (candidate & 2 != 0, candidate & tested & 2 != 0);
            let implication = (!a.0 || b.0, (!a.0 || b.0) && (!a.1 || b.1));
            let negative = (!b.0, !b.0 && !b.1);
            let values = [a, b, implication, negative];
            for boundary in 0..=5 {
                for rule in [
                    ExtremumCutRule::Prefix(boundary),
                    ExtremumCutRule::Suffix(boundary),
                ] {
                    let cut = view(&[0, 4, 2, 3, 1], &rule);
                    let measure = |frozen: bool| {
                        cut.row
                            .conditions
                            .iter()
                            .zip(cut.row.ranks)
                            .filter(|(id, _)| {
                                if frozen {
                                    values[**id].1
                                } else {
                                    values[**id].0
                                }
                            })
                            .fold(
                                ExtremaMeasure {
                                    minimum: 4,
                                    maximum: 0,
                                },
                                |mut total, (_, &rank)| {
                                    total.minimum = total.minimum.min(rank);
                                    total.maximum = total.maximum.max(rank);
                                    total
                                },
                            )
                    };
                    let original = rule.holds(measure(false));
                    let expected = (original, original && rule.holds(measure(true)));
                    let shape = Shape::new(cut, || Ok::<_, Error>(())).unwrap();
                    let mut sink = Boolean::new(&values);
                    assert_eq!(lower(cut, &mut sink).unwrap(), expected);
                    assert_eq!(sink.gates, shape.gates);
                    assert_eq!(
                        sink.work,
                        1 + cut.row.ranks.len() * if shape.constant().is_some() { 1 } else { 2 }
                    );
                }
            }
        }
    }
}

#[test]
fn constant_cuts_validate_every_rank_before_emitting() {
    let rule = ExtremumCutRule::Suffix(0);
    let cut = view(&[0, 4, 2, 3, 5], &rule);
    let mut sink = Boolean::new(&[]);
    assert_eq!(lower(cut, &mut sink), Err(Error::Invalid));
    assert_eq!(sink.work, 6);
    assert!(sink.constants.is_empty());
}

#[test]
fn empty_rows_and_endpoint_cuts_emit_exact_constants() {
    for boundary in 0..=2 {
        for rule in [
            ExtremumCutRule::Prefix(boundary),
            ExtremumCutRule::Suffix(boundary),
        ] {
            let cut = ExtremumCut {
                row: ExtremumRow {
                    conditions: &[],
                    ranks: &[],
                    rank_count: 2,
                },
                rule: &rule,
            };
            let mut sink = Boolean::new(&[]);
            let expected = rule.holds(ExtremaMeasure {
                minimum: 1,
                maximum: 0,
            });
            assert_eq!(lower(cut, &mut sink).unwrap(), (expected, expected));
            assert_eq!(sink.constants, [expected]);
            assert_eq!(sink.gates, 0);
        }
    }
}

#[test]
fn physical_cut_shape_rejects_invalid_carriers_and_boundaries() {
    for (row, rule) in [
        (
            ExtremumRow {
                conditions: &[0],
                ranks: &[],
                rank_count: 2,
            },
            ExtremumCutRule::Prefix(0),
        ),
        (
            ExtremumRow {
                conditions: &[],
                ranks: &[],
                rank_count: 1,
            },
            ExtremumCutRule::Prefix(0),
        ),
        (
            ExtremumRow {
                conditions: &[],
                ranks: &[],
                rank_count: 2,
            },
            ExtremumCutRule::Suffix(3),
        ),
    ] {
        assert!(matches!(
            Shape::new(ExtremumCut { row, rule: &rule }, || Ok::<_, Error>(())),
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn every_linear_visit_can_refuse_without_unmetered_tail_work() {
    let rule = ExtremumCutRule::Prefix(4);
    let cut = view(&[1, 4, 2, 3, 0], &rule);
    for limit in 0..11 {
        let mut sink = Boolean::new(&[(true, true); 4]);
        sink.limit = limit;
        assert_eq!(lower(cut, &mut sink), Err(Error::Capacity));
        assert_eq!(sink.work, limit);
        assert!(sink.gates < 4);
    }
    let mut sink = Boolean::new(&[(true, true); 4]);
    sink.limit = 11;
    assert_eq!(lower(cut, &mut sink).unwrap(), (true, true));
    assert_eq!(sink.reads, [0, 1, 2, 3]);
}

#[test]
fn repeated_qualifiers_and_one_witness_keep_identity_gates() {
    for ranks in [[1, 1, 4, 4, 4], [4, 4, 1, 4, 4]] {
        let rule = ExtremumCutRule::Prefix(2);
        let cut = view(&ranks, &rule);
        let mut sink = Boolean::new(&[(true, true); 4]);
        lower(cut, &mut sink).unwrap();
        assert_eq!(sink.gates, ranks.iter().filter(|&&rank| rank < 2).count());
        assert_eq!(sink.constants, [false]);
    }
}

#[test]
fn map_and_gate_failures_are_preserved() {
    let rule = ExtremumCutRule::Prefix(4);
    let cut = view(&[1, 1, 2, 3, 0], &rule);
    assert_eq!(lower(cut, &mut Boolean::new(&[])), Err(Error::Invalid));
    for ceiling in 0..5 {
        let mut sink = Boolean::new(&[(true, true); 4]);
        sink.gate_limit = ceiling;
        assert_eq!(lower(cut, &mut sink), Err(Error::Invalid));
        assert_eq!(sink.gates, ceiling);
    }
}

#[test]
fn huge_ordinal_carriers_need_no_host_sized_scratch() {
    let rule = ExtremumCutRule::Prefix(u64::MAX);
    let cut = ExtremumCut {
        row: ExtremumRow {
            conditions: &[0],
            ranks: &[u64::MAX - 1],
            rank_count: u64::MAX,
        },
        rule: &rule,
    };
    let shape = Shape::new(cut, || Ok::<_, Error>(())).unwrap();
    assert_eq!((shape.gates, shape.constant()), (0, Some(true)));
    assert_eq!(lower(cut, &mut Boolean::new(&[])).unwrap(), (true, true));
}
