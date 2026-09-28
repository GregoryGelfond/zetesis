# zetesis-presentation

Explicit styling and text tables for human result views. The crate knows no
solver semantics and performs no terminal or environment discovery. Callers
supply typed result summaries, layout width and resolved color policy.

`Table` validates row shape before writing, escapes control characters in cells,
aligns numeric columns and retains values in a vertical layout when a horizontal
table does not fit. Callers bound the number and size of rows. Construction and
rendering take space and work proportional to the supplied text; table rendering
does not buffer answer-set families or repeat measurements.

`ColorMode` and `Role` provide the shared labels, metadata and conclusion styles.
Automatic styling stays plain for a generic library writer until a process
adapter resolves terminal capabilities. Machine views use their own typed
serializers and do not pass through styled text.

See [workflow views](../../docs/book/rust/workflows.md) for composition with the
solver, validation and measurement libraries. `tests/integration/tables.rs` checks shape,
writer failures, control-character escaping and width-sensitive presentation.
