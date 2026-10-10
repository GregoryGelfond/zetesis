use super::*;

struct Boolean {
    mask: usize,
    gates: usize,
    visits: usize,
    limit: usize,
    reads: Vec<usize>,
    constants: Vec<bool>,
    gate_limit: usize,
}

impl Boolean {
    fn new(mask: usize) -> Self {
        Self {
            mask,
            gates: 0,
            visits: 0,
            limit: usize::MAX,
            reads: Vec::new(),
            constants: Vec::new(),
            gate_limit: usize::MAX,
        }
    }
}

impl Sink for Boolean {
    type Value = bool;
    type Error = Error;
    fn tick(&mut self) -> Result<(), Error> {
        if self.visits == self.limit {
            return Err(Error::Capacity);
        }
        self.visits += 1;
        Ok(())
    }
    fn constant(&mut self, value: bool) -> Result<bool, Error> {
        self.constants.push(value);
        Ok(value)
    }
    fn condition(&mut self, index: usize) -> Result<bool, Error> {
        self.reads.push(index);
        Ok(self.mask & (1 << index) != 0)
    }
    fn gate(&mut self, disjunction: bool, left: bool, right: bool) -> Result<bool, Error> {
        if self.gates == self.gate_limit {
            return Err(Error::Invalid);
        }
        self.gates += 1;
        Ok(if disjunction {
            left || right
        } else {
            left && right
        })
    }
}

#[test]
fn physical_recurrence_matches_independent_weighted_sums() {
    for weights in [[0, 0, 0, 0], [1, 2, 0, 3], [u64::MAX, 1, 2, 0]] {
        for bound in -1..=8 {
            let threshold = Threshold {
                conditions: &[0, 0, 1, 2],
                weights: &weights,
                bound,
            };
            let shape = Shape::new(threshold, || Ok::<_, Error>(())).unwrap();
            for mask in 0..8 {
                let mut sink = Boolean::new(mask);
                let actual = lower(threshold, &mut vec![false; shape.scratch], &mut sink).unwrap();
                let total: i128 = threshold
                    .conditions
                    .iter()
                    .zip(weights)
                    .filter(|(condition, _)| mask & (1 << **condition) != 0)
                    .map(|(_, weight)| i128::from(weight))
                    .sum();
                assert_eq!(actual, total >= bound);
                assert_eq!(sink.gates, shape.gates);
            }
        }
    }
}

#[test]
fn empty_thresholds_expand_only_their_constant() {
    for bound in [i128::MIN, -1, 0, 1, i128::MAX] {
        let threshold = Threshold {
            conditions: &[],
            weights: &[],
            bound,
        };
        let shape = Shape::new(threshold, || Ok::<_, Error>(())).unwrap();
        assert_eq!((shape.gates, shape.scratch), (0, 0));
        let mut sink = Boolean::new(0);
        assert_eq!(lower(threshold, &mut [], &mut sink).unwrap(), 0 >= bound);
    }
}

#[test]
fn physical_shape_validates_columns_before_constant_simplification() {
    let threshold = Threshold {
        conditions: &[0],
        weights: &[],
        bound: 0,
    };
    assert!(matches!(
        Shape::new(threshold, || Ok::<_, Error>(())),
        Err(Error::Invalid)
    ));
}

#[test]
fn physical_shape_refuses_unaddressable_expansion() {
    let threshold = Threshold {
        conditions: &[0, 1, 2],
        weights: &[u64::MAX; 3],
        // This is interior: unlike a boundary fold, it needs a host-sized row.
        bound: i128::from(u64::MAX) + 1,
    };
    assert!(matches!(
        Shape::new(threshold, || Ok::<_, Error>(())),
        Err(Error::Capacity)
    ));
}

#[test]
fn physical_lowering_preserves_control_failure() {
    let threshold = Threshold {
        conditions: &[0, 1],
        weights: &[1, 1],
        bound: 1,
    };
    let mut sink = Boolean::new(3);
    sink.limit = 3;
    assert_eq!(
        lower(threshold, &mut [false; 4], &mut sink),
        Err(Error::Capacity)
    );
}

#[test]
fn large_boundary_bounds_use_linear_gates_without_scratch() {
    let weights = [u64::MAX, 1, 2, 0];
    let total = i128::from(u64::MAX) + 3;
    for bound in [1, total] {
        let threshold = Threshold {
            conditions: &[0, 0, 1, 2],
            weights: &weights,
            bound,
        };
        let mut inspected = 0;
        let shape = Shape::new(threshold, || {
            inspected += 1;
            Ok::<_, Error>(())
        })
        .unwrap();
        assert_eq!(inspected, weights.len());
        assert_eq!((shape.gates, shape.scratch, shape.constant()), (3, 0, None));
        for mask in 0..8 {
            let mut sink = Boolean::new(mask);
            let actual = lower(threshold, &mut [], &mut sink).unwrap();
            let sum: i128 = threshold
                .conditions
                .iter()
                .zip(weights)
                .filter(|(condition, _)| mask & (1 << **condition) != 0)
                .map(|(_, weight)| i128::from(weight))
                .sum();
            assert_eq!(actual, sum >= bound);
            assert_eq!(sink.gates, shape.gates);
            assert_eq!(sink.visits, 1 + 2 * weights.len());
            assert_eq!(sink.constants, [true, false]);
            assert_eq!(sink.reads, [0, 0, 1]);
        }
    }
}

#[test]
fn one_positive_contribution_keeps_a_final_gate() {
    let threshold = Threshold {
        conditions: &[0, 1, 2],
        weights: &[0, u64::MAX, 0],
        bound: i128::from(u64::MAX),
    };
    let shape = Shape::new(threshold, || Ok::<_, Error>(())).unwrap();
    assert_eq!((shape.gates, shape.scratch), (1, 0));
    let mut sink = Boolean::new(2);
    assert!(lower(threshold, &mut [], &mut sink).unwrap());
    assert_eq!(sink.gates, 1);
    assert_eq!(sink.constants, [true, false]);
    assert_eq!(sink.reads, [1]);
    assert_eq!(sink.visits, 7);
}

#[test]
fn every_boundary_visit_can_refuse_before_further_work() {
    for bound in [1, 4] {
        let threshold = Threshold {
            conditions: &[0, 1, 2, 0],
            weights: &[1, 0, 2, 1],
            bound,
        };
        // One entry tick, a complete shape scan, and a complete condition scan.
        let work = 1 + 2 * threshold.conditions.len();
        for limit in 0..work {
            let mut sink = Boolean::new(7);
            sink.limit = limit;
            assert_eq!(lower(threshold, &mut [], &mut sink), Err(Error::Capacity));
            assert_eq!(sink.visits, limit);
            assert!(sink.gates < 3);
        }
        let mut sink = Boolean::new(7);
        sink.limit = work;
        assert!(lower(threshold, &mut [], &mut sink).unwrap());
        assert_eq!(sink.gates, 3);
    }
}

#[test]
fn boundary_folds_propagate_gate_refusals() {
    for bound in [1, 3] {
        let threshold = Threshold {
            conditions: &[0, 1, 2],
            weights: &[1, 1, 1],
            bound,
        };
        for limit in 0..3 {
            let mut sink = Boolean::new(7);
            sink.gate_limit = limit;
            assert_eq!(lower(threshold, &mut [], &mut sink), Err(Error::Invalid));
            assert_eq!(sink.gates, limit);
            assert_eq!(sink.reads.len(), limit + 1);
        }
    }
}

struct Frozen<'a>(&'a [(bool, bool)]);

impl Sink for Frozen<'_> {
    type Value = (bool, bool);
    type Error = Error;

    fn tick(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn constant(&mut self, value: bool) -> Result<Self::Value, Error> {
        Ok((value, value))
    }

    fn condition(&mut self, index: usize) -> Result<Self::Value, Error> {
        Ok(self.0[index])
    }

    fn gate(
        &mut self,
        disjunction: bool,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Error> {
        let (original, frozen) = if disjunction {
            (left.0 || right.0, left.1 || right.1)
        } else {
            (left.0 && right.0, left.1 && right.1)
        };
        // The physical evaluator masks each emitted gate by its original truth.
        Ok((original, original && frozen))
    }
}

#[test]
fn boundary_folds_preserve_arbitrary_frozen_condition_truth() {
    for candidate in 0..4 {
        for tested in 0..4 {
            let a = (candidate & 1 != 0, candidate & tested & 1 != 0);
            let b = (candidate & 2 != 0, candidate & tested & 2 != 0);
            let implication = (!a.0 || b.0, (!a.0 || b.0) && (!a.1 || b.1));
            let negative = (!b.0, !b.0 && !b.1);
            let conditions = [a, b, implication, negative];
            for bound in [2, 13] {
                let threshold = Threshold {
                    conditions: &[0, 0, 1, 2, 3],
                    weights: &[0, 2, 3, 4, 5],
                    bound,
                };
                let sum = |frozen: bool| {
                    threshold
                        .conditions
                        .iter()
                        .zip(threshold.weights)
                        .filter(|(index, _)| {
                            if frozen {
                                conditions[**index].1
                            } else {
                                conditions[**index].0
                            }
                        })
                        .map(|(_, &weight)| i128::from(weight))
                        .sum::<i128>()
                };
                let original = sum(false) >= bound;
                let expected = (original, original && sum(true) >= bound);
                assert_eq!(
                    lower(threshold, &mut [], &mut Frozen(&conditions)).unwrap(),
                    expected,
                    "bound={bound}, M={candidate}, J={tested}"
                );
            }
        }
    }
}
