use super::Masks;

#[test]
fn mask_slices_partition_the_owned_words() {
    for (nodes, atoms, chains) in [
        (0, 0, 0),
        (1, 0, 1),
        (0, 1, 63),
        (63, 64, 64),
        (64, 65, 65),
        (65, 130, 130),
    ] {
        let mut masks = Masks::empty(nodes, atoms, chains);
        let lengths = [
            nodes.div_ceil(64),
            nodes.div_ceil(64),
            atoms.div_ceil(64),
            atoms.div_ceil(64),
            atoms.div_ceil(64),
            chains.div_ceil(64),
        ];
        assert!(masks.words.iter().all(|&word| word == 0));
        for (index, mask) in masks.split().into_iter().enumerate() {
            assert_eq!(mask.len(), lengths[index]);
            mask.fill(1 << index);
        }
        let expected: Vec<_> = lengths
            .into_iter()
            .enumerate()
            .flat_map(|(index, length)| std::iter::repeat_n(1 << index, length))
            .collect();
        assert_eq!(&*masks.words, expected);
    }
}

#[test]
fn equal_length_copy_replaces_the_boundaries() {
    // Equal allocation length does not imply equal node, atom or witness spans.
    for (mut destination, mut source, lengths) in [
        (
            Masks::empty(1, 130, 65),
            Masks::empty(193, 64, 65),
            [4, 4, 1, 1, 1, 2],
        ),
        (
            Masks::empty(1, 1, 65),
            Masks::empty(65, 1, 0),
            [2, 2, 1, 1, 1, 0],
        ),
    ] {
        assert_eq!(destination.words.len(), source.words.len());
        for (index, mask) in source.split().into_iter().enumerate() {
            mask.fill(1 << index);
        }
        let address = destination.words.as_ptr();
        destination.clone_from(&source);
        assert_eq!(destination.words.as_ptr(), address);
        assert_eq!(destination.slices().map(<[u64]>::len), lengths);
        assert_eq!(destination.slices(), source.slices());
        for (index, mask) in destination.split().into_iter().enumerate() {
            mask.fill(0);
            assert!(
                source.slices()[index]
                    .iter()
                    .all(|&word| word == 1 << index)
            );
        }
    }
}

#[test]
fn cloned_masks_own_independent_words() {
    let source = Masks::empty(65, 130, 65);
    let mut cloned = source.clone();
    assert_ne!(source.words.as_ptr(), cloned.words.as_ptr());
    for mask in cloned.split() {
        mask.fill(u64::MAX);
    }
    assert!(source.words.iter().all(|&word| word == 0));
}
