# Matching: identifiers, specificity, and the false-alarm rate

The question is *has this been found?* The matcher ranks prior items against
the finding. Word overlap is not used: the author measured Jaccard between a
rejection description and the report it describes at about **0.024** (with
containment 0.61) on the rejection log. Prose that shares only ordinary words
scores nothing, so the matcher scores code identifiers instead.

## What counts as an identifier

Two classes, extracted from the finding and from every dossier item:

- **Symbols** - function, error, contract and qualified names: `snake_case`,
  `camelCase`, `Contract.function`, `module::function`, plus backticked spans
  (a backticked span is split on whitespace, commas and parentheses, so a
  backticked sentence contributes its tokens, not one giant identifier).
  Examples: `distributeSurplus`, `ReserveBackingShortfall`,
  `BalanceSheet.distributeSurplus`, `VaultEngine.open`.
- **Paths** - file paths such as `src/Vault.sol` and bare source file names with
  a source extension.

Not every token qualifies. An identifier must be at least 5 characters, must not
be a language keyword (`function`, `address`, `uint256`, `require`, ...), and
must look like an identifier: contains `_`, contains `::` or `/`, mixes upper
and lower case, or is 14+ characters. A plain lowercase word is dropped.

## The threshold

> A prior item is reported when it shares **at least two symbol identifiers**
> with the finding, and **both count as specific**.

Paths alone never qualify an item. Paths are shown in `shared_identifiers` and
used to break ties, because a shared file is useful context, but in the public
benchmark every finding in one repository shares the repository's files, so
path-only matches grouped unrelated findings ([benchmark.md](benchmark.md)).

## The 20% generic filter

An identifier is **generic** when it appears in more than 20% of the dossier's
items, unless it is named by at most two items. A symbol named by one or two
items is rare in absolute terms, which keeps the rule usable on a dossier that
holds one audit and its index. Generic symbols never count toward the threshold.

This is what stops a shared base contract (`VaultEngine` in every finding of
one repo) from making every pair of findings look related. It does not remove
the problem; it reduces it.

## Ranking and tie-break

Candidates are sorted, in order:

1. more shared **specific** symbols;
2. higher **rarity** - the sum of `items - df + 1` over those symbols, so an
   overlap resting on rarer names wins;
3. more shared symbols overall (including generic ones);
4. more shared paths;
5. the shared identifiers themselves, lexically.

Document order is never a tie-break: two dossiers holding the same items in a
different order rank them the same way, and a test asserts it. `--top-k`
defaults to 5 and accepts 1-50.

## What a hit means

A hit is a **reading-list entry**, not a duplicate finding and not a warning.
On the final public benchmark the false-alarm rate on judged-paid unique
findings is **26/60 = 43.3%** (Wilson 95% CI 31.6-55.9) and duplicate recall@5
is **39/64 = 60.9%** (48.7-71.9); known-prior recall is n=1 and is not a rate.
43.3% is above the plan's 30% trigger, so **0.1.0 ships retrieval-only**: it
ranks prior items and shows them with quotes, and prints no duplicate warning,
until the matcher is fixed. The next release must re-measure before the warning
can return. All three runs (30.0%, 25.0%, 43.3%) are in
[benchmark.md](benchmark.md).

## Errata: the 0/64 that was a harness bug

An earlier interim report said duplicate recall@5 was 0/64. That was not the
matcher. The harness compared the judged id `acc:001-M` with the dossier item
`acc-001-M.md` without normalising `:` to `-`, so every correct retrieval was
counted as a miss. After the fix the same binary scored 36/64; later matchers
scored 33/64 and 39/64 on the final build. The full paragraph, including the
intermediate char-boundary panic, is in
[benchmark.md](benchmark.md#errata-2026-11-13).

## Absence is not novelty

A finding with no hit is not new. A dossier can only hold what you saved: a
concurrent duplicate is in nobody's dossier yet, and an audit published last
week is not in the PDF you downloaded last month. The tool says so in its output
instead of implying novelty.

## Mechanism hints

Each item carries an optional `mechanism_class` from the vocabulary published
with the rejection dataset (e.g. `accounting_order_of_operations`,
`oracle_bounds_or_staleness`). It is a **lexical hint**, assigned by a keyword
table, for one purpose: to show *their mechanism* beside *yours* so you can see
whether the overlap is the same bug family. It is not used for ranking, it is
`null` when the text is negated or unrecognised, and a wrong hint is not a
claim. If it is absent, the comparison line is omitted.

## False-alarm risk, stated plainly

- A finding in a large repository shares symbols with its neighbours: helpers,
  base contracts, shared errors. The specificity rule reduces this; it does not
  remove it. 43.3% of judged-unique findings got at least one prior item on the
  final build, which is why the duplicate warning is off.
- The threshold is a stated heuristic, calibrated only on the public corpus. It
  is not a probability.
- A missed duplicate costs a rejection; a false alarm costs reading time. The
  benchmark publishes both rates so a reader can weigh them.
