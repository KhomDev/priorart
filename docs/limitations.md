# Limitations

Stated plainly, because a tool that hides its edges gets trusted past them.

## Retrieval, not duplication

priorart ranks prior items to read. It never says two findings are the same. On
the final public benchmark the false-alarm rate on judged-paid unique findings is
**26/60 = 43.3%** (Wilson 95% CI 31.6-55.9) and duplicate recall@5 is
**39/64 = 60.9%** (48.7-71.9); known-prior recall is n=1 and is not a rate.
Because 43.3% is above the plan's 30% trigger, **0.1.0 ships retrieval-only**:
the prior-item section is a reading list and no duplicate warning is printed
until the matcher is fixed. All three runs are in [benchmark.md](benchmark.md).

## Absence is not novelty

A dossier holds only what you saved. A concurrent duplicate is in nobody's
dossier; an audit published last week is not in the PDF you downloaded last
month; a private report is in nobody's set. "Nothing ranked" means "nothing in
this file matched", and the output says so.

## The gates read what you transcribed

G1 compares your claimed severity with your transcription of the rewards page. If
the transcription is wrong, the gate is confidently wrong. That is why the ledger
validator rejects an uncited entry and why a payout block without `source_url`
produces no result rather than "payable".

## The preconditions are lexical

G2 detects preconditions with regexes and a short keyword table, negation-aware
within a clause. It will miss a precondition stated obliquely, and it can flag one
stated in an unusual way. Every match prints the clause verbatim and the
precondition that tripped it, so the reader can disagree in one line.

## The mechanism hint is a hint

`mechanism_class` comes from a keyword table. It is not used for ranking, it is
`null` when the text is negated or unrecognised, and a wrong hint is not a
claim. Do not cite it as a finding.

## PDFs are best-effort

An uncompressed PDF is scraped for text literals; otherwise `pdftotext` is
required. Without it a compressed PDF is skipped with a warning and the build
still succeeds. A scanned, image-only PDF has no text at all: OCR is not
attempted, and the file must be converted outside priorart. The exact warning is
in [dossier.md](dossier.md).

## No live rules, no update channel

The 5 tier shapes, the 14 clause classes and the rejection dataset are compiled
into the binary with `include_str!`. There is no data directory, no rules
download, no update check and no network call
([no-network.md](no-network.md)). Consequence: the vocabulary is exactly what
shipped with your binary, and a new clause class or corrected dataset arrives
only in a new release. The matcher is likewise frozen at the measured revision;
the benchmark is a retrospective measurement of that revision, not a rolling
score.

## One researcher's dataset

The rejection dataset is one person's log with no positive class. It supplies a
vocabulary, not a base rate. See [rejections.md](rejections.md).

## A weak separator is the reason for the evidence-only design

The deleted viability-gate rules did not separate paid from unpaid findings
(12 of 25 hard kills were paid findings). priorart replaces the score with ranked
quotes and publishes its own error rate, but it does not claim the separator is
strong: on the final build 43.3% of judged-unique findings still get a prior
item. That is the number the next fix must move.

## What this tool is not

It is not a proof checker. Whether the PoC proves the claim is realism-gate's
question, and priorart does not attempt it. It is not a payout oracle: it
compares your claimed severity with a table you transcribed. It does not decide
scope, exclusion or duplication for a platform, and it is not a verdict.

## Windows, and what "tested" means

The test suite runs on Windows and on Linux in CI; there is no unix-only code.
The benchmark corpus is public contest material; behaviour on private reports,
other languages or non-markdown program pages is not measured.
