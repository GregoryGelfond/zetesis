//! Boolean reductions over already-evaluated child nodes.

/// A false conjunct or true disjunct decides the result independently of the
/// suffix. The callback owns checked reads, cancellation and work accounting;
/// an interrupted read publishes no value. Every other DAG node still needs its
/// own evaluation, even if this reduction did not read it.
#[inline]
pub(crate) fn reduce<E>(
    operands: &[usize],
    conjunction: bool,
    mut read: impl FnMut(usize) -> Result<bool, E>,
) -> Result<bool, E> {
    for &operand in operands {
        if read(operand)? != conjunction {
            return Ok(!conjunction);
        }
    }
    Ok(conjunction)
}

#[cfg(test)]
mod tests {
    use super::reduce;

    #[test]
    fn completed_prefixes_equal_the_full_boolean_reduction() {
        for length in 0..=8 {
            let operands: Vec<_> = (0..length).collect();
            for bits in 0_u16..1 << length {
                let truth = |index| bits & (1_u16 << index) != 0;
                for conjunction in [false, true] {
                    let expected = if conjunction {
                        operands.iter().copied().all(truth)
                    } else {
                        operands.iter().copied().any(truth)
                    };
                    let decisive = operands.iter().position(|&i| truth(i) != conjunction);
                    let needed = decisive.map_or(length, |position| position + 1);
                    // Every proper prefix stops without a Boolean result; an
                    // admitted decisive prefix must never touch the suffix.
                    for allowance in 0..=needed {
                        let mut reads = 0;
                        let result = reduce(&operands, conjunction, |index| {
                            if reads == allowance {
                                return Err(());
                            }
                            reads += 1;
                            Ok(truth(index))
                        });
                        assert_eq!(reads, allowance);
                        assert_eq!(
                            result,
                            if allowance == needed {
                                Ok(expected)
                            } else {
                                Err(())
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_operands_retain_occurrence_order() {
        let mut reads = Vec::new();
        assert_eq!(
            reduce(&[0, 0, 1, 2], true, |index| {
                reads.push(index);
                Ok::<_, ()>(index == 0)
            }),
            Ok(false)
        );
        assert_eq!(reads, [0, 0, 1]);
    }
}
