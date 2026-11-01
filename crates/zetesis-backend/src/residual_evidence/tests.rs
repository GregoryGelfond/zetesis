use super::*;

#[test]
fn header_seals_sizes_without_using_native_layout() {
    let h = Header {
        nonce: [19; 16],
        seed: u64::MAX,
        worlds: Some(65_536),
        batches: Some(4_096),
        retained_worlds: 65_536,
        retained_batches: 4096,
        complete: false,
    };
    assert_eq!(h.byte_len(), Some(5_177_472));
    let bytes = h.encode();
    assert_eq!(Header::decode(&bytes).unwrap(), h);
    for index in [0, 8, 12, 17, 72, 127] {
        let mut corrupt = bytes;
        corrupt[index] ^= 1;
        assert!(Header::decode(&corrupt).is_err());
    }
}

#[test]
fn absent_world_fields_have_one_encoding() {
    let world = World {
        reason: None,
        open: Some(0),
        query: Query::Unknown,
    };
    let bytes = world.encode();
    assert_eq!(World::decode(&bytes).unwrap(), world);
    let mut other = bytes;
    other[2] = 1;
    assert!(World::decode(&other).is_err());
    other = bytes;
    other[16] = 1;
    assert!(World::decode(&other).is_err());
    assert_ne!(
        World {
            open: None,
            ..world
        }
        .encode(),
        bytes
    );
    assert_ne!(
        World {
            query: Query::Certified,
            ..world
        }
        .encode(),
        bytes
    );
}

#[test]
fn entered_world_preserves_wide_duration_and_attempted_splits() {
    let world = World {
        reason: Some(Reason::WorkLimit),
        open: None,
        query: Query::Entered {
            visited: 5,
            attempted_splits: 4,
            published_splits: 2,
            depth: Some(3),
            work: 0,
            host_ns: Some(u128::MAX),
        },
    };
    let mut bytes = world.encode();
    assert_eq!(World::decode(&bytes).unwrap(), world);
    bytes[40] = 5;
    assert!(World::decode(&bytes).is_err());
}

#[test]
fn batch_checks_partitions_and_keeps_zero_distinct_from_missing() {
    let batch = Batch {
        residuals: [Some(7), Some(0), Some(2), Some(5)],
        width: None,
        host_ns: [Some(7), Some(0), Some(7), None],
    };
    let mut bytes = batch.encode();
    assert_eq!(Batch::decode(&bytes).unwrap(), batch);
    bytes[8] = 6;
    assert!(Batch::decode(&bytes).is_err());
    bytes = batch.encode();
    bytes[64] = 1;
    assert!(Batch::decode(&bytes).is_err());
    bytes = batch.encode();
    bytes[96] = 1;
    assert!(Batch::decode(&bytes).is_err());
}
