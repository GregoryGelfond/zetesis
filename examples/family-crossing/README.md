# Family Crossing

`integrated.lp` is the unchanged entry program. It includes
`family_crossing_complete_domain.lp`, which includes `eec_htp_integration_v5.lp`.
The adjacent manifest records the exact original bytes and include spellings.
These supplied programs are preserved fixtures; they are not cleaned copies
from the historical correctness catalog.

The maintained benchmark corpus includes this bundle as one regular case after
the 94 historical catalog cases. Its expected result is the complete family of
eight optimal ties at cost 29. Each campaign requires a fresh clingo comparison;
the recorded count and cost alone do not establish model identity or completeness.

From the repository root:

```sh
zetesis-bench run --case family-crossing/integrated.lp --clingo /absolute/path/to/clingo \
  --time-limit 120 --timeout-seconds 125 --campaign-seconds 900
```

The same case is included without `--case`. Normal campaign resource and capture
limits apply. The [benchmark guide](../../docs/book/reference/benchmarking.md)
explains the full-model comparison, qualification evidence and refusal behavior.
