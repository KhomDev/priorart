# The control set — measuring the false-positive rate

**Status: an external public-corpus measurement now exists.** The numbers are
in [benchmark.md](benchmark.md), and they are the rate on that corpus, not a
property of the rules. The false-positive rate on the author's own paid
findings is still unmeasured: that log contains no paid findings. This
document is how that second measurement would be built. It is not the same
question as the public contest set, and the two numbers are not substitutes.

## Why this is the number that matters

The published calibration is a **recall-side** measurement: it fired on 48 of 59
rejected findings (81%). Recall on rejections is not the number a user needs.

The number a user needs is **precision**: when `vg` says KILL, how often is it
wrong? A tool that fires on everything has perfect recall on a set of rejections
and is worthless. Until an accepted-or-paid set is run through it, that question
is open, and the honest answer is "unmeasured".

The author's own log cannot answer it: all 97 rows have `paid: no`. There is no
positive class in the data.

## What a control set is

A directory of findings that **were accepted, paid, or otherwise survived
triage** — the positive class. Run:

```bash
vg bench --rejected your-rejections/ --accepted your-accepted/ --labels labels.csv
```

and read the **ACC FALSE-KILL** column: how often a rule hard-fails a finding
that a program paid for. That is the false-positive rate, per rule and overall.
See [`docs/bench.md`](bench.md).

A soft rule firing on an accepted finding is not a false kill. HOLD is the
tool's expected output for most drafts. Only the hard verdict costs a submission.

## How to build one from public reports

### 1. Choose a source with a licensing story you have actually read

Candidate sources, in rough order of how usable their terms usually are:

| Source | What it gives you | What to check first |
|---|---|---|
| A program's own published audit reports | Findings the auditors raised and the protocol fixed | The report's own terms. Many are public PDFs with no explicit licence |
| Public contest results (awarded findings) | Findings that were judged valid and paid | The platform's terms of service, and whether results are republished elsewhere under a different licence |
| Public disclosure write-ups by researchers | Findings the program acknowledged | The author's licence on the write-up |
| A protocol's own post-mortems | Acknowledged incidents | Usually the most permissive, and the smallest |

**This is a STOP gate, and it is not a formality.** Do not fetch a third-party
report corpus into this repository. The tool ships no data; that is the whole
design. Downloading reports here would make this repository a redistributor, and
the licensing analysis for a corpus of hundreds of reports from a dozen platforms
is not something a README can do.

### 2. Keep the corpus outside the repository

Put it wherever your private working data lives. Add the path to your own
`.gitignore`, not this one — this repository's `.gitignore` is for the public
tree.

### 3. Extract the finding text, not the PDF

`vg bench` reads `.md` and `.txt`. Extract each finding's body into one file.
Record, separately, in a labels file:

```csv
filename,label
2025-acme-vault-reentrancy.md,accepted
2025-acme-oracle-staleness.md,accepted
```

One row per finding, matched on filename. `vg bench` reports how many label rows
matched, so a typo shows up as a count rather than as silence.

### 4. Match the rejected set

If you are reusing your own rejection log as the negative class, run both halves
through the same command so the two rates are comparable. A rejected set from one
source and an accepted set from another measures the two corpora as much as it
measures the tool.

### 5. Report the set before the number

Every false-positive number needs, next to it:

- **N** and the source of each half.
- **The selection rule.** "Every paid finding from program X between dates A and
  B" is a measurement. "Findings I had to hand" is an anecdote.
- **The extraction method.** Whether the text is the researcher's report, the
  triager's summary, or the auditor's PDF changes what the regexes see.
- **Whether the findings are redacted.** A redacted finding is shorter and
  differently worded; the catalog reads prose.

## Sizing

Ten accepted findings give you a false-kill rate with a confidence interval wider
than the number. If you measure 0/10, the honest reading is "somewhere under
roughly 30%", not "zero". Report the interval, or report the count and let the
reader compute it.

There is no useful shortcut. A precision figure needs a positive class, and a
positive class needs findings that someone paid for.

## What this document is not

It is not a plan to fetch anything. No step above downloads a report into this
repository, and none should be added. If a future maintainer wants a published
control set, the work is a licensing review plus a network allowlist, which is a
decision for the repository owner and not for a script.
