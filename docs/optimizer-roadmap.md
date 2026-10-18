# Optimizer upgrade

The current implementation is the first CPU foundation from the proposed hybrid
CPU/GPU architecture. Source contours remain authoritative. GPU computation is
not implemented yet; Iced's GPU renderer is independent of the nesting solver.

## Shipped foundation

- A consistent objective comparison across genetic selection, worker incumbents,
  and the displayed result: more placed parts, then fewer sheets, then lower
  placement fitness. An incomplete result cannot win by reporting high utilization.
- Area-sorted, reversed, and shuffled initial orders. Seeded random generation
  stays reproducible for CPU-worker comparisons.
- Best-result preservation when a refinement run starts with an inferior layout.
- CPU validation before publishing candidates and again before export, including
  original contours, clearances, duplicate instances, copy limits, and sheet count.
- Per-run timing, evaluations, rejected candidates, improvements, first valid
  layout time, preparation/validation time, and shared NFP cache query counters.
- A headless JSON benchmark (`deepnest --benchmark [job.deepnest]`) and native UI
  views consuming those same measurements.

Cache reuse is hits divided by total lookups. Validation time is accumulated
wall time in the sequential acceptance stage; elapsed time includes preparation,
placement, and validation. A first valid layout may be partial, so coverage is
reported independently. The graph shows material use rather than the ranking
objective; filling more parts onto additional sheets can legitimately lower it.

## Next stages

1. Expand the benchmark corpus and profile NFP construction, placement evaluation,
   memory, and transfer-free CPU baselines across multiple seeds and budgets.
2. Introduce a placement-evaluator boundary and measured local reinsertion and
   compaction moves, preserving the incumbent and existing hole handling.
3. Add interchangeable CPU/GPU batch evaluators, resident occupancy masks, top-K
   selection, workload-based dispatch, and CPU refinement/validation of candidates.
4. Add overlap-minimization search and multi-sheet destroy/repair with independent
   budgets and sheet-elimination objectives.
5. Extend stock and instance models for irregular sheets, forbidden regions,
   remnants, locking, mirroring, and continuous rotation with cache-key revisions.

Each stage needs comparable quality and elapsed-time measurements on the same
jobs. Acceleration alone is not evidence of improved nesting quality.
