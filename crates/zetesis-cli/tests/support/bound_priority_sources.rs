//! Original sources shared by semantic sessions and physical formula checks.

pub const SOURCES: [&str; 10] = [
    include_str!("../fixtures/bounds-priorities/upper.lp"),
    include_str!("../fixtures/bounds-priorities/lower.lp"),
    include_str!("../fixtures/bounds-priorities/endpoints.lp"),
    include_str!("../fixtures/bounds-priorities/signed.lp"),
    include_str!("../fixtures/bounds-priorities/minimize.lp"),
    include_str!("../fixtures/bounds-priorities/maximize.lp"),
    include_str!("../fixtures/bounds-priorities/weak.lp"),
    include_str!("../fixtures/bounds-priorities/composed.lp"),
    include_str!("../fixtures/bounds-priorities/zero.lp"),
    include_str!("../fixtures/bounds-priorities/correlated.lp"),
];
