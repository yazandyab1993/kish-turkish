# Local Performance Workflow

## Goals
- Measure engine core performance independently from UI rendering overhead.
- Track improvements from search and transposition-table tuning.

## Benchmark command

```bash
cargo run --profile benchopt --bin bench_engine -- --depth 16 --seconds 5 --runs 5
```

## Recommended baseline scenarios

1. Time-limited
   - `--depth 24 --seconds 3 --runs 5`
2. Depth-limited
   - `--depth 14 --seconds 30 --runs 5`
3. Node-limited
   - `--depth 24 --seconds 30 --nodes-m 20 --runs 5`

## Report template

Record median values:

- completed depth
- nodes
- qnodes
- nps
- tt_hits
- cutoffs
- elapsed_ms

Use this template for each optimization batch:

```text
Batch:
Date:
Command:
Median depth:
Median NPS:
Delta vs previous:
Notes:
```
