# Sustained outflow-limiter blockade via the never-shipped cross-segment netting fix, with ungated liquidation as the loss-conversion path

**Program:** Current Smart Contracts (Sui Move mainnet)
**Package in scope:** protocol `0x45bae0425e9098ce5cba3d3fa2836220ad24c9f88aa0dffffb5a52b49319fc70` (live v2; upgrade of origin `0xfe1d8929d13b00aaecd7642dec1c6d41cab82882a1b139efa46bf61dfd6380bf`)
**Reported by:** an independent security researcher (first person singular throughout)
**Discovery:** 2026-10-22, ~21:30 +02:00
**Status:** packaged for human review; nothing here has been submitted anywhere.

---

## Summary

I analyzed the live Sui Move bytecode of the Current lending protocol and measured the on-chain
state of every eMode rate limiter on MainMarket (all reads 2026-10-22 via public read-only RPC;
raw responses are attached in the evidence bundle). The daily outflow limiter stores usage in 24
one-hour segments and refunds deposits and repayments only against the CURRENT segment — the same
defect the protocol's own 2026-03 audit accepted as Sherlock issue M-1 and whose accepted fix never
shipped to mainnet (FIX-LOST on live v2 bytecode). Because a refund that crosses an hour boundary
is destroyed instead of releasing the originating segment's charge, a third party can hold the
rolling 24-hour withdrawal gate at exactly 100% of its limit indefinitely by cycling only
limit/24 of capital through hourly withdraw/redeposit rounds — measured at live limits: ~$208k
rolling for the USDC withdraw gate, ~$54k for USDSUI, ~$10.3k for the eMode-group-8 SUI gate.

While the gate is pinned, any other user's withdrawal of at least one unit aborts with limiter
error `outflow_reach_limit_error` (abort code 105). Deposits and repays remain open, but a
leveraged position whose only self-rescue is to convert collateral into the debt asset (withdraw,
swap, repay) has no in-protocol path. Liquidation, debt-ADL and collateral-ADL bypass the rate
limiter entirely, so the position can still be liquidated at the liquidator's chosen time. On a
live-parameter eMode group-8 position (100,000 SUI collateral, USDC debt at 78% raw LTV) I measure
the conversion: a -8.2% SUI move crosses the 85% liquidation threshold inside the oracle's EMA
tolerance, the required ~2,848 SUI threshold-touching withdrawal is blocked by the pinned gate,
and the forced liquidation takes a 6% penalty of **$46,098** plus forced-sale slippage. I did not
send any state-changing transaction on mainnet; the reproduction runs against a local fork of
mainnet state, and every mainnet claim below is a live read.

## Severity (argued)

**Impact class.** Two stacked classes. The floor is availability: a third party pinning the
withdraw gate denies every other user's withdrawals of that asset for as long as they cycle
capital — indefinitely, at limit/24 rolling cost. On top of it, and only when the conditions bind,
a fund-loss class: blocked users with leveraged positions whose self-rescue requires converting
collateral into the debt asset get liquidated instead and lose the liquidation penalty. Who loses:
external, non-privileged position holders (leveraged loopers in the gated asset with fully-deployed
funds). Who can attack: any unprivileged address that can pre-supply one hour's slice of the limit
(deposits are never gated, so the balance can be built at leisure). The loss is permanent for the
victim (penalty paid at forced-sale prices); the blockade is indefinite for as long as the attacker
cycles; both directions are third-party-inflicted.

**Magnitude — mechanism arithmetic labeled as such, inputs measured on chain.** The blockade cost
is limit/24 rolling per gate (USDC $5.0M gate -> $208k; USDSUI $1.3M gate -> $54k; g8 SUI 250,000
SUI/day gate -> $10.3k; group-0 SUI $49.25M gate -> $2.05M), and a bytecode-exact simulation seeded
with the live segment state (attached `evidence/sink-analysis.md`, section 2) shows 66 of 72
victim-hours blocked at $1M rolling capital on the USDC gate (45-66 of 72 across the g8 SUI
capital rows), versus **0 of 72** under correct cross-segment netting with identical capital — the
differential is entirely the missing fix. The attached harness's Phase B/D model reproduces the
same differential on a clean mirror with the live g8 parameters (pin from hour ~8, 65/72 pinned
hours, 51/72 victim-hours blocked, versus 0/72 under the fix shape). The loss conversion is quantified
on one live-parameter position: 100,000 SUI collateral ($985k at the live EMA price I read), USDC
debt $768.3k at 78% raw LTV, g8 parameters cf 0.80 / liquidation factor 0.85 / incentive 0.06 (all
read from chain), 6% g8 SUI penalty (the protocol's own MainMarket penalty table) -> **$46,098
penalty** on this single position, plus forced-sale slippage, executed in ~5 sequential liquidations
at the documented 20% close factor. This figure is the mechanism's arithmetic on live inputs, not
an observed incident; the aggregate exposure scales with the gated asset's leveraged TVL, not with
the attacker's capital. I claim no insolvency or principal-destruction figure — none exists in this
mechanism.

**Requirements and preconditions, stated plainly.** (1) Attacker capital: limit/24 of the target
gate, rolling (pre-supplied at leisure; deposits are ungated). (2) Timing: the pin must coincide
with a price move that crosses victims' liquidation thresholds; the attacker controls the gate, not
the move. (3) Victim profile: a leveraged looping position in the gated asset near the liquidation
threshold with no outside holdings of the debt asset. (4) No privileged step anywhere in the chain.
(5) The defect is live today: I read every group-0 limiter on chain (2026-10-22T11:49:43Z) and the
stale-segment inventory shows the refund destruction already occurring organically — e.g. the USDC
borrow limiter holds 20,081.6 USDC of dead out-of-window charge and the SUI borrow limiter holds
1,515,931.7 SUI across 19 stale segments (`evidence/limiter-segment-analysis.json` in the attached
bundle).

**Why NOT one tier higher.** The fund loss is conditional, not deterministic: it requires a
threshold-crossing market move during the blockade and a victim without outside liquidity. There is
no direct theft, no principal destruction, no insolvency, and the attacker extracts nothing — the
loss is the protocol's own liquidation penalty taken at the liquidator's chosen moment. A higher
tier would require an unconditional or attacker-directed loss path, and the blockade alone (without
the move) is availability grief inside a documented rate-limiting throttle.

**Why NOT one tier lower.** The mechanism chain is verified end-to-end on live deployed bytecode
(all four limiter call sites in the 98-module package, the ungated liquidation/ADL paths, abort
code 105) and the defective state is measured on mainnet today, not hypothetical: five of eight
group-0 limiters currently hold destroyed out-of-window refunds as dead stored value, which is the
accepted-but-unshipped fix's absence observable on chain (FIX-LOST). The fund-loss conversion
exists and is quantified at live parameters. The blockade is both indefinite and cheap (limit/24,
gas-cheap hourly loop, attacker capital re-earns supply yield while parked), and the governance
response (24 h timelock + 12 h execution window on limit changes) is slower than the 24-hour attack
window itself. The documented intent — "Net amount" in the daily-caps page, "The protective rails
cannot block users from de-risking their own positions" in the security page — is violated by the
implementation, which places this squarely in the program's in-scope class "attacks on logic (code
behavior differs from business description)" rather than a best-practice observation.

**Suggested severity: Medium** (this program's Medium tier). This is an argued suggestion grounded
in the measured magnitude and conditionality above; the final severity and the duplicate
disposition are the program's call.

## Vulnerability type

**Business logic / accounting manipulation (rate-limiter accounting).** One line: `limiter::reduce_outflow`
credits refunds only to the CURRENT hour segment, so a redeposit or repayment made after a segment
boundary destroys the refund instead of releasing the earlier segment's charge — letting a third
party pin the rolling 24-hour withdraw/borrow gate at 100% with only limit/24 of rolling capital
while the protocol's forced-exit paths are not rate-limited at all.

**Component-to-class mapping.**

- **Component A — M-06 limiter refund destruction** (`limiter::reduce_outflow` same-segment-only
  credit): a known, previously-reported defect whose accepted fix never shipped to mainnet
  (FIX-LOST). Class: accounting manipulation.
- **Component B — ungated liquidation / ADL exit paths** (`market::handle_liquidation`,
  `market::handle_debt_auto_deleverage`, `market::handle_collateral_auto_deleverage` contain no
  limiter interaction): a new observation of this report. Class: missing authorization boundary on
  the exit side (one-sided rate limiting).
- **Composite — gated self-rescue vs ungated attacker liquidation**: an asymmetric blockade that
  converts an availability throttle into conditional fund loss for leveraged position holders.
  Class: business logic (code behavior differs from business description).

## Root cause

`limiter::reduce_outflow` (live disassembly `limiter.mv.txt` lines 172-234) resolves exactly one
segment — the current one, `(now / segment_duration) % 24` — and if that slot is stale it resets
it to zero BEFORE subtracting, so the refund vanishes whenever the current segment holds no charge;
the segment that recorded the original outflow is never touched. The `Segment` struct stores only
`{index, value}` (no timestamps, no contribution record), so cross-segment unwinding is
structurally impossible. The consequences compose at the market layer:

- `market::handle_withdraw` charges the eMode group's DEPOSIT limiter via `limiter::add_outflow`
  (market.mv.txt:768) — withdrawals are gated by the "deposit" limiter's 24-hour window.
- `market::handle_mint` calls `limiter::reduce_outflow` (market.mv.txt:594) — the deposit refund,
  the documented counterweight that keeps the gate from pinning, is destroyed the moment it crosses
  an hour boundary.
- `market::handle_borrow` / `market::handle_repay` mirror the same pair on the borrow limiter
  (market.mv.txt:914 / market.mv.txt:1201).
- `market::handle_liquidation` (market.mv.txt:1221-1377), `market::handle_debt_auto_deleverage`
  (1378-1560) and `market::handle_collateral_auto_deleverage` (1562-1735) contain zero limiter
  interaction — exactly four limiter call sites exist in the entire protocol package, none of them
  on a forced-exit path.

`limiter::add_outflow` (limiter.mv.txt:103-170) aborts with `error::outflow_reach_limit_error` —
abort code 105 (error.mv.txt:22-25) — when `count_current_outflow(now) + amount > limit`, where
`count_current_outflow` (limiter.mv.txt:285-365) sums the 24 live segments. At `usage == limit`,
any other user's withdrawal of one unit aborts.

This is the same root cause as accepted Sherlock issue M-1 (warden issue M-06 in the public
contest corpus); the accepted fix never shipped — the live v2 package still contains the unfixed
code, and my on-chain reads show its effects (destroyed out-of-window refunds) stored in five of
eight group-0 limiters today.

## Attack path / exploit scenario

1. The attacker pre-supplies limit/24 of the target asset (deposits are never gated; the balance
   can be accumulated over days) — e.g. ~10,417 SUI for the g8 SUI withdraw gate (250,000 SUI/day),
   or ~$208k for the USDC gate.
2. Each hour boundary: redeposit the previous hour's withdrawal (the refund lands on the CURRENT,
   charge-free segment and is destroyed), then withdraw fresh funds up to the remaining room. Each
   hour's charge persists for the full 24-hour window.
3. Within 1-8 hours (measured, depending on capital vs. organic draw) the trailing window sits at
   exactly 100% of the limit and stays there for as long as the attacker cycles. The attacker's
   capital is outside the protocol at most one hour per round and otherwise parked as supply
   earning yield.
4. Every other user's withdrawal of the gated asset now aborts with limiter code 105. Deposits and
   repays still work; the withdraw-to-swap-to-repay self-rescue path is closed.
5. If a price move pushes leveraged positions in the gated asset over the liquidation threshold
   during the blockade, whitelisted liquidators (whose repay-in and seizure flows are not
   rate-limited) liquidate them at the liquidator's chosen time; the blocked users eat the
   liquidation penalty (3.08-15% per the protocol's MainMarket penalty table; 6% in the worked g8
   example) plus slippage.

Governance cannot outrun the window: Param-Modifier limit changes carry a 24 h timelock plus a 12 h
execution window — a full day of blockade before relief can land, against an attack window that is
itself 24 hours.

## Reproduction steps

Environment: Node >= 20, npm. The attached bundle contains everything: `poc/` (harness + shared
client + pinned addresses) and `evidence/` (live measurements). All paths below are relative to
the bundle root; no host-specific paths are used anywhere.

1. Unzip the bundle. From `poc/`: `npm install` (installs @mysten/sui 2.37.0), then
   `npx tsc --noEmit` — expected exit 0 (the bundle's tsconfig compiles exactly the three CMP-04
   sources; verified at package time).
2. Live state read (read-only, no fork needed), from `poc/`:
   `npx tsx src/cmp04-limiter-blockade.ts`. Phase A re-derives the SUI withdraw-gate limiter from
   the on-chain eMode registry, prints limit / live-window usage / stale out-of-window inventory,
   and asserts the 24 x 3600 s segment layout; Phase A2 asserts the harness's bytecode-exact
   mirror reproduces the live window sum; the Phase B/D model runs the hourly attacker loop
   (redeposit last hour's withdrawal, withdraw up to the remaining room) against both reduce
   semantics with the live g8 gate parameters and prints the sustained-blockade differential —
   under the live (buggy) reduce the window pins at 100% and the victim's 1-unit withdraw aborts
   code 105 for tens of the 72 modeled hours; under the netting-fix reduce the identical attacker
   blocks zero. This is the same reading path that produced `evidence/limiter-segment-analysis.json`
   and `evidence/capacity-table.json`.
3. Start a local mainnet fork (no mainnet transaction is ever signed or submitted):
   `sui-fork start --network mainnet --checkpoint <recent_checkpoint>`, then `sui-fork status`.
   The fork serves its API on 127.0.0.1:9000.
4. Run the blockade harness against the fork:
   `POC_MODE=fork SUI_FULLNODE_URL=http://127.0.0.1:9000 POC_EXECUTION_APPROVED=cmp04 npx tsx
   src/cmp04-limiter-blockade.ts`. Fork phases: attacker pin (hourly deposit/withdraw cycle
   against a funded fork account, mirrored in the harness), victim withdraw (expect transaction
   failure with MoveAbort in `limiter::add_outflow`, abort code 105), liquidation of the same
   position (expect success with a LiquidateEvent in the fork effects and no limiter abort
   anywhere), and the same-segment negative control (repeat the cycle without crossing the hour
   boundary; the victim withdraw then succeeds, isolating the cross-segment refund destruction as
   the cause).
5. Negative control on the fix semantics: the harness re-runs the victim decision against the
   fixed-state mirror — the same segment state with redeposit refunds credited to the segments that
   recorded the withdrawals, i.e. the state the accepted M-1 fix would produce — and asserts the
   withdrawal passes. (sui-fork has no arbitrary state editor; the executed-fork control is the
   same-segment route above.)
6. Independent assertions: the abort module/function/code is parsed from the fork's execution
   record (`parseMoveAbort` in the harness), and the liquidation outcome from the executed
   transaction record — both facts come from the fork's execution record, not from harness echo.

On the attached local fork replica I executed the core proof of this chain against mainnet state:
a 640,000 SUI withdrawal charged the eMode-0 SUI withdraw-gate segment 497970 with
640,000,000,655,161 MIST (window 6.93% -> 19.73%); redepositing the same coin at the SAME clock
netted the charge back to 0 (window -> 6.93% — the same-segment negative control); the identical
redeposit one hour later was DESTROYED — the originating charge and the window read byte-identical
before and after (19.71% -> 19.71%), the refund written into the charge-free current segment and
clamped at zero. Transaction digests and the limiter state around every step are in the bundle
under `evidence/fork-run/` (`14-p1-execution-results.md`, `15-p2-execution-results.md`,
`EXECUTION-REPORT.md`). Two legs of the fork demonstration were blocked by a live-protocol
condition, each with its causal chain recorded in `evidence/fork-run/16-blocked-legs.md`: the
x_oracle SUI/USD feed was frozen on mainnet at session time (with the 30 s one-sided staleness
gate, live withdraw transactions were failing too), so the 100% window pin was unreachable (the
reserve's cash caps the reachable ceiling at 58.48%) and the victim's 1-unit MoveAbort-105 record
could not be produced; and the whitelisted-integrator PackageCallerCap was not resolvable, so the
liquidation leg was not executed (its structural fact — zero limiter references across liquidation
and both ADL paths — rests on the bytecode). The bundle README states the achieved demonstration
level exactly.

## PoC outline

- Harness: `poc/src/cmp04-limiter-blockade.ts` (TypeScript, @mysten/sui SDK 2.37.0). Phases: (A)
  live read of the SUI withdraw-gate limiter segments via the EModeRateLimiter dynamic field
  re-derived from the eMode object; (A2) bytecode-exact mirror vs live cross-check; (B) attacker
  hourly redeposit+withdraw pin loop with a bytecode-exact 24-segment mirror; (C) victim withdraw
  PTB (`withdraw::withdraw`) expecting MoveAbort 105 in `limiter::add_outflow`, plus a liquidation
  PTB (`liquidate::liquidate`, PackageCallerCap-gated) on the same obligation expected to succeed;
  (D) netting-fix negative control; (E) independent assertions parsed from the fork execution
  record.
- Shared client: `poc/src/client.ts`; pinned live object ids: `poc/src/addresses.ts`
  (re-verified against live RPC at fork time; the CoinDecimalsRegistry pin in this bundle is the
  corrected live id `0x53785858...538d7` — the earlier pinned value does not exist on chain).
- Fork tool: MystenLabs `sui-fork` (verified build 1.76.1+; API on 127.0.0.1:9000).
- Abort constant cited from the live disassembly: `error::outflow_reach_limit_error` = 105
  (error.mv.txt:22-25), raised by `limiter::add_outflow` (limiter.mv.txt:125).

## Impact

Sustained withdrawal/borrow denial on any gated asset for limit/24 of rolling attacker capital
(measured at live limits: $208k USDC, $2.05M SUI group-0, $54k USDSUI, $10.3k g8 SUI); conditional
conversion into liquidation-penalty losses for leveraged position holders (mechanism arithmetic at
live parameters: $46,098 on one $985k group-8 SUI position); the blockade outlasts the governance
response window (24 h timelock + 12 h execution). The stale-segment inventory proves the refund
destruction is already occurring on mainnet today.

## Recommendation

Ship the accepted M-1 fix: credit refunds to the segment that recorded the outflow (timestamped
contributions unwound oldest-first, or a signed rolling-flow structure — both shapes were already
specified in the 2026-03 judged report). Until then: treat deposits as full same-cycle gate relief
(net the current window sum by the refund amount regardless of segment), or abort-gate
liquidations on the same limiter the victim's self-rescue faces. A regression test already exists
in the judged report's text: charge near the end of segment N, reduce in N+1, assert usage returns
to zero.

## Notes on scope and duplicates

The surface is in the in-scope protocol package. The same-segment refund defect was reported and
accepted in the protocol's 2026-03 audit (Sherlock issue M-1; warden issue M-06 in the public
contest corpus) and its fix never shipped — the live v2 bytecode still carries the unfixed form,
which I verified line-by-line against the live disassembly and the on-chain limiter state. What
that report covered: the refund root cause, the one-shot cap-saturation grief, and the availability
impact ("legitimate users can be blocked from borrowing / withdrawing"). What this report adds on
top — three deltas absent from M-1's report text:

1. **The sustained-blockade economics.** M-1 shows a one-shot saturation from one round trip. It
   never models the hourly redeposit+withdraw loop, the limit/24 rolling capital, the pinning
   timeline, the 66/72 victim-hours steady state, or the 0/72 counterfactual under the fix. M-1's
   attacker parks the full amount outside for the rest of the cycle; this report's attacker never
   parks more than one hour's slice.
2. **The ungated-liquidation one-sidedness.** M-1's impact section lists blocked borrowing and
   withdrawing only. The observation that liquidation, debt-ADL and collateral-ADL bypass the gate
   entirely while the victim's self-rescue outflows are gated — the asymmetric blockade against
   position holders — appears nowhere in M-1's text.
3. **The penalty-conversion sink.** M-1 frames the loss as availability. The conversion of the
   blockade into forced liquidation-penalty losses for a defined victim class (leveraged loopers,
   fully-deployed funds, threshold-crossing move during the blockade) with a quantified example at
   live parameters is not in M-1's text in any form.

I state this plainly so the program can judge the delta itself.

## Timeline and the 24-hour eligibility clause

- 2026-10-22, ~21:30 +02:00 — discovery of the composite (the sustained-blockade economics over the
  never-shipped fix, plus the ungated-liquidation conversion path). All underlying chain reads and
  the bytecode verification predate this timestamp on the same day.
- Packaged for submission well inside the program's "report no later than 24 hours after discovery"
  clause; the 24-hour mark falls at ~2026-10-23, ~21:30 +02:00.
- Timestamps in this report are 24-hour clock, +02:00 (Europe/Budapest) unless marked UTC; on-chain
  reads are marked UTC where the source records UTC.
