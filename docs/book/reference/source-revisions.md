# Source revision identities

Before the first source release, archival metadata was removed or made portable.
This changed historical commit identifiers while preserving every retained Rust,
WGSL and Lean source file, commit authorship and the order of changes.

The [revision map](source-revisions.json) relates the 104 original revisions
cited by the retained measurements, fixtures and proof records to their published
source counterparts. Use `public_commit` to check out the corresponding source;
`original_commit` is the identity recorded when the evidence was collected.
Source links in the manual target the published commits. Labels and measurement
records retain their original identifiers.

Timing results, executable checksums, source inventories and dependency pins
retain their recorded values. The mapping does not represent a new benchmark or
proof run. Historical proof-command working directories and fixture executable
paths were made portable; those edited metadata files have different hashes.

The retained [membership extraction](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/membership)
provides separate source and artifact inventories. Both inventories still match
the mapped package revision identified by its replay instructions. Verify them
before replaying the extraction; a later source change requires new evidence.
