use super::KeyWork;
use crate::{Resource, Stop};

#[test]
fn key_work_stops_at_its_ceiling_without_wrapping() {
    let mut work = KeyWork {
        steps: u64::MAX - 1,
        limit: u64::MAX,
    };
    assert_eq!(work.step(), Ok(()));
    assert_eq!(work.steps(), u64::MAX);
    for _ in 0..2 {
        assert_eq!(
            work.step(),
            Err(Stop {
                resource: Resource::Work,
                limit: u128::from(u64::MAX),
                observed: u128::from(u64::MAX) + 1,
            })
        );
        assert_eq!(work.steps(), u64::MAX);
    }
}
