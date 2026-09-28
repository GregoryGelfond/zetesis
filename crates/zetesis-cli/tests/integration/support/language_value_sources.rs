//! Original programs composing Boolean heads, local witnesses and objective values.

/// All programs share the same finite pair carrier and local arithmetic witness.
/// Their exact originals are also used by physical-device qualification.
pub const SOURCES: [&str; 10] = [
    include_str!("../../fixtures/language-values/true-disjunct.lp"),
    include_str!("../../fixtures/language-values/false-disjunct.lp"),
    include_str!("../../fixtures/language-values/negated-false.lp"),
    include_str!("../../fixtures/language-values/negated-true.lp"),
    include_str!("../../fixtures/language-values/true-head.lp"),
    include_str!("../../fixtures/language-values/false-head.lp"),
    include_str!("../../fixtures/language-values/double-negated-false.lp"),
    include_str!("../../fixtures/language-values/double-negated-true.lp"),
    include_str!("../../fixtures/language-values/extremum-presence.lp"),
    include_str!("../../fixtures/language-values/forwarded-zero.lp"),
];
