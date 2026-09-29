# zetesis-bench test fixtures

[`earlier-report.json`](earlier-report.json) and [`later-report.json`](later-report.json)
are saved benchmark reports, in the published format `zetesis-bench compare`
reads. Each measures the cases `generated/choice-2.lp` and
`generated/cycle-2.lp` with the reference and two native profiles, one and four
workers, over three timed rounds. The earlier report did not pass: in the second
case, each native profile timed out in its first round, and its later rounds
were not attempted and name the sample that blocked them. It also records a
memory sample for the reference and for the first native profile. The later
report passes every cell. The view and comparison tests read both; as fixed
samples, they detect an unintended change to the format.

[`baseline_schedule.json`](baseline_schedule.json) is the exact slot sequence of
the default schedule, extracted from the sealed ordinary CPU report whose
decompressed SHA-256 is
`f420be5da474025b194fd0c2fa3f455a74307afd6b6cfca61891643af6d93d03`.
