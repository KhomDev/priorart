# priorart P2R-2 analysis plan (committed before any V1 or FINAL measurement)

Supplements `eval/P2R_PREREGISTRATION.md`, `eval/P2R_PREREGISTRATION_AMENDMENT_1.md` and
`eval/P2R_AMENDMENT_2.md`. Committed 2026-11-27; no sealed pair and no FINAL contest has been run by any
P2 binary.

## 1. Contract drift found and resolved (stricter reading)
- The preregistration's verdict header says FINAL is "Dup-Bench contests after T2 **plus the new sealed
  pairs**"; its per-round table (§6) and Amendment 1 item 5 assign the six sealed pairs (Basin, Revert
  Finance, Size, Ethena Labs, LoopFi, Othernet Global / Salty) to **V1**.
- A slice opened in V1 is in-sample afterwards, so it cannot also be fresh in FINAL. **Resolution: the six
  pairs are V1 only; the priorart FINAL verdict uses Dup-Bench FINAL alone** (17 contests, 158 paid-unique,
  776 judged duplicates — both preregistered minima, ≥ 60 and ≥ 300, hold without the pairs).
- **There is no V2 slice**: Amendment 1's temporal split is train / validation / FINAL. Round 2's
  preregistered V2 targets (recall@5 ≥ 55%, FAR ≤ 20%, exploratory) therefore cannot be measured; Round 2
  for priorart is reported as "no fresh slice exists" rather than re-using validation or carving FINAL.

## 2. Cells
- **recall@5**: judged duplicates whose judge-grouped representative (a paid-unique primary in the same
  contest) is among the top five candidates, over all judged duplicates queried (abstentions count as misses
  and are reported separately).
- **FAR at the shipped threshold**: paid-unique primaries on which the calibrated alarm (`alarm-v1.json`,
  calibrated value ≥ 0.9016, raw score ≥ 0.854897) fires, over all paid-unique primaries queried.
- **Warning precision** (the alarm's FINAL lower bound ≥ 0.80 decides ON/OFF): among queries where the
  alarm fires, the share whose top candidate is the judged primary (the definition in `alarm-v1.json`).
- **Abstention**: queries that abstain, by cause, over all queries.
- Wilson 95% on every k/n; contest-clustered bootstrap (10,000 draws) on pooled cells.
- `--prose-anchors` stays **off** (the ranker and the alarm were fitted without it).
- The validation split is reported as **model-selection data**, never as a fresh number.

## 3. Runs
- Round 1: the six pairs (V1), by the frozen Round-1 binary, once.
- FINAL: Dup-Bench FINAL, by the final frozen binary, once.
