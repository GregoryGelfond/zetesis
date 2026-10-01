//! Stable human records beside deliberately variable timing values.

pub(crate) fn before_timing(text: &str) -> &str {
    text.split_once("\nTime:")
        .or_else(|| text.split_once("\n\u{1b}[34mTime:"))
        .map_or(text, |(prefix, _)| prefix)
}

pub(crate) fn banner() -> &'static str {
    concat!(
        "zetesis ",
        env!("CARGO_PKG_VERSION"),
        " | Copyright (c) 2026 Gregory Gelfond | MIT License\n"
    )
}

pub(crate) fn exhausted(text: &str) -> bool {
    text.lines().any(|line| {
        line.strip_prefix("Models: ")
            .is_some_and(|count| count.parse::<usize>().is_ok())
    }) && text
        .lines()
        .any(|line| matches!(line, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"))
}

pub(crate) fn preamble(text: &str) -> bool {
    if text.is_empty() || text == banner() {
        return true;
    }
    text.strip_prefix(banner())
        .and_then(|rest| rest.strip_prefix("Backend: "))
        .and_then(|rest| rest.strip_suffix("\n\n"))
        .is_some_and(|configuration| !configuration.contains('\n'))
}
