//! Checked human units at the argument boundary; library settings retain bytes and seconds.

use std::num::NonZeroUsize;

pub(crate) fn workers(value: &str) -> Result<NonZeroUsize, String> {
    if value == "auto" {
        Ok(super::host_workers())
    } else {
        value
            .parse()
            .map_err(|_| "expected auto or a positive worker count".to_owned())
    }
}

pub(super) fn seconds(value: &str) -> Result<u64, String> {
    quantity(value, &[("", 1), ("s", 1), ("m", 60), ("h", 3_600)]).map_err(|()| {
        "expected whole seconds or a whole number with s, m or h; value must fit u64 seconds"
            .to_owned()
    })
}

pub(super) fn bytes(value: &str) -> Result<u64, String> {
    quantity(
        value,
        &[
            ("", 1),
            ("B", 1),
            ("KiB", 1 << 10),
            ("MiB", 1 << 20),
            ("GiB", 1 << 30),
            ("TiB", 1 << 40),
        ],
    )
    .map_err(|()| {
        "expected bytes or a whole number with B, KiB, MiB, GiB or TiB; value must fit u64 bytes"
            .to_owned()
    })
}

fn quantity(value: &str, units: &[(&str, u64)]) -> Result<u64, ()> {
    let boundary = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, suffix) = value.split_at(boundary);
    let amount: u64 = number.parse().map_err(|_| ())?;
    let factor = units
        .iter()
        .find_map(|(unit, factor)| (*unit == suffix).then_some(*factor))
        .ok_or(())?;
    amount.checked_mul(factor).ok_or(())
}
