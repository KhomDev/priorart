# The six gates before the proof of concept

[viability-gate](https://github.com/KhomDev/viability-gate) runs these six checks before a proof of
concept is written: the duplicate check D1, and G1 to G5. In this dataset they are six of the
values `earliest_gate` can take.

Two things they are **not**:

- **Not "before you read any code".** Every one of them needs the finding in hand. You have to
  know what you found to ask whether a clause, an audit or a known issue covers it. G1 comes
  closest: the reward table tells you, before hunting, which severities can pay at all.
- **Not "preventable".** A row whose earliest gate is one of the six was not necessarily stoppable
  from the public record: the duplicate check may have had nothing on file to find. The
  `preventable_by` column answers that question, row by row.

Each section ends with the query that lists its rows, so no count here can go stale.

---

## D1 - Duplicate / known-issue check

**Question:** Is this already in the program's known-issues list, a published audit, or a public
disclosure?

**How to run it:**

1. Read the program's known-issues section verbatim.
2. Collect every linked audit report, contest result and public disclosure.
3. Compare **findings**, not titles: a prior audit naming a file is not the same as covering your
   function.

**The trap:** an unlogged duplicate of *your own* earlier submission. Your own log is a record
too, and it is the only one that holds your rejected findings. Log every submission.

```bash
python scripts/query.py --gate D1_duplicate_check -v
python scripts/query.py --preventable-by own_log -v
```

---

## G1 - Severity floor

**Question:** Is there a payout tier at the severity I can plausibly reach?

**Why it matters:** the most expensive outcome is a *confirmed valid* finding that pays nothing
because the reward table has no row for its severity.

**How to run it:**

1. Open the program's live rewards page.
2. Record every severity row in the reward table.
3. Note the **lowest severity with a non-zero reward**: that is the floor.
4. Ask: can my thesis plausibly reach that severity?

If the answer is no, **stop**. A Medium-class mechanism on a Critical-only program has an expected
value of zero.

See the [tier-shape taxonomy](https://github.com/KhomDev/bounty-economics) for the closed set of
shapes a reward table resolves to.

```bash
python scripts/query.py --gate G1_severity_floor
python scripts/query.py --triage below_payout_floor
```

---

## G2 - Exclusion clauses

**Question:** Does a written exclusion clause cover my bug class?

**How to run it:**

1. Copy every out-of-scope clause from the program's scope page, **verbatim**.
2. Classify each one against the [clause-class taxonomy](https://github.com/KhomDev/bounty-economics).
3. For each, ask whether your bug class falls inside it.

**The privileged-role distinction.** A finding that *requires* an admin to act maliciously is out
of scope. A finding where a **reasonable admin action has a non-obvious destructive second-order
effect** can be in scope: the admin is not misbehaving, the code is. Check which one you have
before abandoning a finding.

```bash
python scripts/query.py --gate G2_exclusion_clause -v
```

---

## G3 - Scope tree

**Question:** Is the root cause inside the scoped files?

**How to run it:**

1. Get the exact in-scope file list.
2. Trace your finding's **root cause** to a file - not the impact site, the root cause.
3. If the root cause is outside the scoped files, the finding is out of scope regardless of impact.

**The trap:** a real bug with a real impact whose root cause lives in a deployment script, a test
utility, or an off-chain component.

```bash
python scripts/query.py --gate G3_scope_tree -v
```

---

## G4 - Prior audits

**Question:** Is this surface already closed by a published audit?

**How to run it:**

1. Note which **files and functions** each published audit covers - not the titles.
2. Note the **post-audit delta**: what changed after the last audit.

**Two traps:**

- **A prior audit naming a file is not the same as covering your function.** Read the finding.
- **The post-audit delta is the richest surface** - and also where you are least likely to be
  duplicating.

```bash
python scripts/query.py --gate G4_prior_audit -v
```

---

## G5 - Test-suite intent

**Question:** Does the project's own test suite assert that this behaviour is intended?

**How to run it:**

1. Grep the target's test suite for the behaviour you are about to report.
2. Look for tests that **assert the current behaviour is correct**.

**The trap:** a test asserting the behaviour does not always mean it is intended - sometimes the
test encodes the bug. But it raises the bar: you now have to argue the test itself is wrong.

```bash
python scripts/query.py --gate G5_test_intent -v
```

---

## Gates this dataset does not record

viability-gate has two more gates that are not `earliest_gate` values here:

- **G6 - fork novelty.** Is this a fork whose behaviour is documented upstream? A finding that is
  novel in the fork but documented as intended upstream is not a finding.
- **G7 - own submission history.** Have I already submitted this bug family? Here, that is
  `repeat_of_own` and `preventable_by = own_log`.

## Report-level and realism gates

The remaining `earliest_gate` values need the finding *and* a judgement about it:

- **Report-level anti-patterns** - no attacker-win, headline-vs-evidence mismatch,
  cooperating-party exit, below the severity floor. The full catalogue is in viability-gate.
- **Realism gates** - attacker-model, realistic-configuration and concession checks: findings that
  are mechanically real but unreachable under the deployed configuration, or that contradict their
  own stated preconditions.

```bash
python scripts/query.py --posthunt -v
```

---

## Using the gates

The gates are a **prioritiser, not a kill-switch**. Two cautions:

- **A first-pass triage decision is not a final verdict.** A finding that looks out of scope on a
  first read may be in scope on a second - particularly around the privileged-role distinction.
- **Over-filtering is its own failure mode.** If the gates kill everything, you have calibrated
  them too aggressively and will miss real findings.

Run them in order, record what each one said, and when a gate fails, write down *why* before moving
on. The `earliest_gate` column in this dataset is exactly that record.
