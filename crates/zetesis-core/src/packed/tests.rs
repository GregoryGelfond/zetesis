use super::{union_shifted, union_shifted_until_hit};

fn reference(words: &[u64], source_words: usize, shift: usize) -> Vec<u64> {
    let mut expected = words.to_vec();
    for source in 0..source_words * 64 {
        if words[source / 64] & (1 << (source % 64)) != 0
            && let Some(target) = source.checked_add(shift).filter(|&s| s / 64 < words.len())
        {
            expected[target / 64] |= 1 << (target % 64);
        }
    }
    expected
}

#[test]
fn translation_matches_independent_position_sets() {
    for seed in [0, 1, 3, u64::MAX, 0x8100_0041_0000_0001] {
        for source_words in 0..=3 {
            for shift in [0, 1, 2, 31, 63, 64, 65, 127, 128, 191, 192, usize::MAX] {
                let original = [seed, seed.rotate_left(17), seed.rotate_right(9)];
                let expected = reference(&original, source_words, shift);
                let mut actual = original;
                let mut reads = 0;
                union_shifted(&mut actual, source_words, shift, &mut || {
                    reads += 1;
                    Ok::<_, ()>(())
                })
                .unwrap();
                assert_eq!(actual.as_slice(), expected);
                assert_eq!(reads, source_words.min(3usize.saturating_sub(shift / 64)));
            }
        }
    }
}

#[test]
fn a_new_bit_is_not_reused_by_the_same_occurrence() {
    for shift in [1, 63, 64, 65] {
        let mut words = [1, 0, 0];
        union_shifted(&mut words, 1, shift, &mut || Ok::<_, ()>(())).unwrap();
        assert_eq!(words.iter().map(|word| word.count_ones()).sum::<u32>(), 2);
        assert_ne!(words[shift / 64] & (1 << (shift % 64)), 0);
    }
}

#[test]
fn refusal_precedes_the_next_descending_source_read() {
    let original = [0x8100_0000_0000_0001, 3, 7, 0];
    for allowed in 0..3 {
        let mut actual = original;
        let mut visited = 0;
        assert_eq!(
            union_shifted(&mut actual, 3, 1, &mut || {
                if visited == allowed {
                    return Err("stopped");
                }
                visited += 1;
                Ok(())
            }),
            Err("stopped")
        );
        assert_eq!(visited, allowed);
        let mut expected = original;
        for source in (3 - allowed)..3 {
            let bits = original[source];
            expected[source] |= bits << 1;
            expected[source + 1] |= bits >> 63;
        }
        assert_eq!(actual, expected);
        let mut retried = original;
        union_shifted(&mut retried, 3, 1, &mut || Ok::<_, ()>(())).unwrap();
        assert_eq!(retried.as_slice(), reference(&original, 3, 1));
    }
}

#[test]
fn empty_or_outside_translations_read_nothing() {
    for (mut words, count, shift) in [(vec![], 0, 0), (vec![7], 1, usize::MAX)] {
        let before = words.clone();
        union_shifted(&mut words, count, shift, &mut || Err("unexpected read")).unwrap();
        assert_eq!(words, before);
    }
}

#[test]
fn translated_hits_match_independent_position_sets() {
    for seed in [0, 1, 3, u64::MAX, 0x8100_0041_0000_0001] {
        for source_words in 0..=3 {
            for shift in [0, 1, 2, 31, 63, 64, 65, 127, 128, 191, 192, usize::MAX] {
                let original = [seed, seed.rotate_left(17), seed.rotate_right(9)];
                let complete = reference(&original, source_words, shift);
                for (lower, upper) in [
                    (0, 0),
                    (1, 1),
                    (2, 2),
                    (1, 63),
                    (63, 63),
                    (63, 64),
                    (64, 65),
                    (127, 128),
                    (129, 191),
                    (191, 191),
                    (192, usize::MAX),
                    (2, 1),
                ] {
                    let interval = lower..=upper;
                    let expected = (0..source_words * 64).any(|source| {
                        original[source / 64] & (1 << (source % 64)) != 0
                            && source.checked_add(shift).is_some_and(|target| {
                                target / 64 < original.len() && interval.contains(&target)
                            })
                    });
                    let mut actual = original;
                    let mut reads = 0;
                    let hit = union_shifted_until_hit(
                        &mut actual,
                        source_words,
                        shift,
                        interval,
                        &mut || {
                            reads += 1;
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap();
                    assert_eq!(
                        hit, expected,
                        "source={source_words}, shift={shift}, bounds=({lower},{upper})"
                    );
                    let full_reads = source_words.min(3usize.saturating_sub(shift / 64));
                    if hit {
                        assert!(reads > 0 && reads <= full_reads);
                        for ((&old, &partial), &full) in original.iter().zip(&actual).zip(&complete)
                        {
                            assert_eq!(old & partial, old);
                            assert_eq!(partial & full, partial);
                        }
                    } else {
                        assert_eq!(actual.as_slice(), complete);
                        assert_eq!(reads, full_reads);
                    }
                }
            }
        }
    }
}

#[test]
fn existing_output_bits_are_not_translated_witnesses() {
    let mut words = [1, 1];
    assert!(!union_shifted_until_hit(&mut words, 1, 1, 64..=64, &mut || Ok::<_, ()>(())).unwrap());
    assert_eq!(words, [3, 1], "the full union still completes");
}

#[test]
fn carry_witness_stops_before_lower_source_words() {
    let original = [1, 1, 1_u64 << 63, 0];
    let mut words = original;
    let mut reads = 0;
    assert!(
        union_shifted_until_hit(&mut words, 3, 1, 192..=192, &mut || {
            assert_eq!(reads, 0, "no lower source is read after the witness");
            reads += 1;
            Ok::<_, ()>(())
        })
        .unwrap()
    );
    assert_eq!(reads, 1);
    assert_eq!(words, [1, 1, 1_u64 << 63, 1]);
    assert_ne!(words.as_slice(), reference(&original, 3, 1));
}

#[test]
fn refusal_before_a_witness_is_not_a_hit() {
    let original = [1, 1, 1, 0];
    for allowed in 0..2 {
        let mut words = original;
        let mut reads = 0;
        assert_eq!(
            union_shifted_until_hit(&mut words, 3, 1, 65..=65, &mut || {
                if reads == allowed {
                    return Err("refused source");
                }
                reads += 1;
                Ok(())
            }),
            Err("refused source")
        );
        assert_eq!(reads, allowed);
        let expected = if allowed == 0 { original } else { [1, 1, 3, 0] };
        assert_eq!(
            words, expected,
            "only admitted source words may mutate output"
        );
        assert_eq!(words[1] & 2, 0, "the witness source was not inspected");
        let mut retried = original;
        let mut remaining = 2;
        assert!(
            union_shifted_until_hit(&mut retried, 3, 1, 65..=65, &mut || {
                if remaining == 0 {
                    return Err("unread suffix");
                }
                remaining -= 1;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(remaining, 0);
        assert_eq!(retried, [1, 3, 3, 0]);
    }
}

#[test]
fn empty_or_outside_translations_have_no_witness() {
    for (mut words, source_words, shift) in [(vec![], 0, 0), (vec![7], 1, usize::MAX)] {
        let original = words.clone();
        assert!(
            !union_shifted_until_hit(
                &mut words,
                source_words,
                shift,
                0..=usize::MAX,
                &mut || Err("unexpected read"),
            )
            .unwrap()
        );
        assert_eq!(words, original);
    }
}
