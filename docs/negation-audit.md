# Negation audit (P1–P26)

The engine drops a match when a negation cue sits in the 6 tokens before it
(`no`, `not`, `never`, `without`, `missing`, `n't`, …), and when a contrastive
phrase sits in the 8 tokens before it:

`rather than`, `instead of`, `as opposed to`, `other than`, `in contrast to`,
`more than a`, `more than just`, `beyond a`, `anything but`, `far from a`,
`far from the`.

An immediately preceding `non` negates a hyphenated form (`non-best-practice`).
`non` further back does not: "a non-trivial note about best practice" is still
that class.

The single-token window stayed at 6. Widening it would also hide a real
admission that happens to have a `not` several clauses earlier in the same
sentence. The phrase window is wider because the phrase itself is two or three
tokens, and the word it negates is after it.

Every catalog rule has `negation_aware: true` (the default). None of them opts
out. The audit below is which rules that helps, and where a phrase cue can
suppress a true hit. Those residuals are accepted and listed, not patched with
another special case.

| Rule | What the window does | Residual |
|---|---|---|
| P1 | The signals are themselves negations ("not currently exploitable"). A cue *before* that phrase suppresses the admission. "rather than not currently exploitable" is not a sentence anyone writes. | A quoted denial the author then refutes, if the refutation is not a separate sentence. |
| P2 | "no admin can" and "rather than an admin can" do not fire. "missing the onlyOwner modifier" was already a cue. | "no one other than the admin can" is admin-gated and is suppressed, because both `no` and `other than` sit in front of `admin can`. |
| P3 | "there is no front-run" stays silent. "rather than a front-run" stays silent. | A front-run that is named and then, more than 8 tokens later, called the real path. |
| P4 | Contrast in front of `MockVault` drops the weak form. The strong form "instead of the real contract" starts at `instead`, so the phrase is the match, not a cue in front of it. | None new. |
| P5 | "not outside the scope" is suppressed by `not`. | "outside the scope" of a sentence the author is quoting. |
| P6 | "not if a future upgrade" is suppressed. | A future-upgrade sentence that also contains `not` for an unrelated clause inside the window. |
| P7 | **The CMP-04 miss.** "rather than a best-practice observation" no longer fires. "not a best-practice" already did not. | A real best-practice critique that uses "rather than" about something else in the same 8 tokens. |
| P8 | "not arguably" is suppressed. | The word `arguably` with no negation still fires, which is the rule. |
| P9 | An admission of wrong line numbers with `not` in front of it is suppressed. | Rare, and it would be a denial of the admission. |
| P10 | "not extrapolating" is suppressed. | None new. |
| P11 | "not testnet-only" is suppressed. | None new. |
| P12 | "does not revert" does not satisfy the failure marker in that sentence. Another sentence that does revert still can. | A revert the author is quoting in order to say the opposite, inside the window. |
| P13 | "rather than inconsistent state" is suppressed. A number anywhere still suppresses via `none_of`, which is document-scoped on purpose: one quantified delta answers the objection. | Unchanged. |
| P14 | "not an estimated payout" is suppressed. | A payout phrase the author is quoting from the program. |
| P15 | "rather than the documented behaviour" is suppressed, which is what the finding is arguing. The explicit `none_of` list stays document-scoped: "contradicts the documented behaviour" is the finding's point wherever it sits. | Unchanged. |
| P16 | "not below the payout floor" is suppressed. | A floor mentioned in one clause and a different claim in the next, inside the window. |
| P17 | The signal often *starts* with `no` ("no fund loss"), so the cue is the match, not a token in front of it. "rather than a denial of service" is suppressed. | "rather than no fund loss" would suppress a real admission. That wording is not a report. |
| P18 | "not the same root cause as" is suppressed. Suppression patterns are sentence-scoped, so a denial in one sentence does not hide an admission in the next. See the rule page. | A sentence that both admits a prior issue and contains "not previously". The whole sentence is skipped. |
| P19 | Structural. The negation window does not apply. The matcher has its own conditions. | Unchanged. |
| P20 | "does not require a cooperating party" is suppressed by `not`. | A cooperating party named and then, inside the window, denied. |
| P21 | "the operator does not choose" is suppressed. | Unchanged from the single-token window. |
| P22 | The signal often starts with `no` ("no deployed instance"). A phrase in front of that `no` would suppress it. | Same shape as P17. |
| P23–P26 | Structural claim checks. They look for the presence of a piece, not a phrase that a negation should flip. "no attacker" is still the absence of an attacker profile only if the attacker pattern itself does not match; "no attacker" does not match the attacker pattern. | A report that says "there is no attacker" and also, elsewhere, "an unprivileged attacker" satisfies P23. The second sentence is the profile. |

The CMP-04 regression is `tests/fixtures/cmp-04-report.md`. P7 must stay silent on it. P18 must fire.
