# Limitations

Stated plainly, because a tool that hides its edges gets trusted past them.

## Retrieval, not duplication

priorart ranks prior items to read. It never says two findings are the same. The
current public benchmark file `eval/benchmark/results.json` (priorart 0.2.2)
records a false-alarm rate on judged-paid unique findings of
**15/60 = 25.0%** (Wilson 95% CI 15.8-37.2) and duplicate recall@5 of
**35/64 = 54.7%** (42.6-66.3); known-prior recall is n=1 and is not a rate.
The 0.1.0 measurement on the same corpus was 26/60 = 43.3% (31.6-55.9). The
0.2.2 interval still reaches above the plan's 30% trigger, and later corpora
sit above it, so the release stays retrieval-only: the prior-item section is a
reading list and no duplicate warning is printed. The runs are in
[benchmark.md](benchmark.md).

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

## HTML is converted, not rendered

A saved `scope.html` (or `scope.htm`, or a file under `pages/`) is converted to
text by a small in-repo normalizer: headings, bullets, table rows, `href`s,
blocks, entities, and script/style removal. It is **not a browser**. CSS
layout, JavaScript-rendered text, images and text carried only in attributes
other than `href` are not read. A page whose text exists only in a script
yields no items, and the build says so. The rules are in
[dossier.md](dossier.md#html-program-pages).

## PDFs are best-effort, and the fallback is native code

An uncompressed PDF is scraped for text literals; otherwise `pdftotext` is
required. Without it a compressed PDF is skipped with a warning and the build
still succeeds. A scanned, image-only PDF has no text at all: OCR is not
attempted, and the file must be converted outside priorart. The exact warning is
in [dossier.md](dossier.md). A `pdftotext` that runs past 60 seconds or writes
more than 16 MiB is killed and its source skipped; the limits and the reason for
them are in [dossier.md](dossier.md#input-limits).

`pdftotext` is poppler: **native third-party code, run as the same user with the
same privileges, without a sandbox**, on an untrusted PDF. priorart bounds its
lookup, arguments, time and output, but does not isolate it. `priorart build
--no-pdftotext` disables the fallback entirely; a PDF the built-in scrape cannot
read is then reported unread and skipped.

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
strong: on the current `results.json` measurement, 15/60 = 25.0% of
judged-unique findings still get a prior item. That is the number the next
fix must move. The 0.1.0 build on the same corpus was 26/60 = 43.3%.

## What this tool is not

It is not a proof checker. Whether the PoC proves the claim is realism-gate's
question, and priorart does not attempt it. It is not a payout oracle: it
compares your claimed severity with a table you transcribed. It does not decide
scope, exclusion or duplication for a platform, and it is not a verdict.

## Windows, and what "tested" means

There is no unix-only production code. OS-specific tests are gated: the FIFO
test runs on Unix where `mkfifo` exists, and the Windows symlink tests skip
without SeCreateSymbolicLinkPrivilege (the Ubuntu CI job runs the Unix cases).
The benchmark corpus is public contest material; behaviour on private reports,
other languages or real-world program pages saved as HTML is not measured. The
HTML path is tested against one committed list-based fixture and an inline
table-limitation test, not against live platform pages
([dossier.md](dossier.md#html-program-pages)).

## `fetch` and HTML import do not run JavaScript

`priorart fetch` saves the HTML the server returns, and `dossier import` reads
only that markup. A page that builds its content in the browser (for example
the Sherlock contest index, or any single-page app whose body is an empty root
element plus scripts) yields little or no text, and the import notes say the
source has no readable text. Nothing is rendered, no script runs, and no API
the page calls is fetched. Save such a page from a browser (or transcribe it)
and import the saved file instead.

Adjacent sibling elements with no text between them are read as separate
words (`<span>$2,000,000</span><span>Min</span>` becomes `$2,000,000 Min`).
A word a page splits across two sibling elements (`<b>Lo</b><i>an</i>`) is
therefore read as two words; text that follows a closing tag directly
(`<b>Loan</b>Book`) is still joined.
