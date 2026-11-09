"""The closed vocabularies of the published dataset - ONE definition, imported everywhere.

The build, the aggregates, the query tool, the vocabulary cross-check and the tests
all import from here, so no two of them can disagree about what a value means.

Nothing in this file is derived from, or describes, any private record. It is the
public half of the schema: the set of values a published cell may hold.
"""

from __future__ import annotations

#: Published columns, in file order.
COLUMNS: tuple[str, ...] = (
    "id",
    "family_id",
    "bug_family",
    "mechanism_class",
    "triage_class",
    "earliest_gate",
    "root_cause_class",
    "repeat_of_own",
    "duplicate_source",
    "preventable_by",
)

#: Literal value for a field whose payload is withheld.
WITHHELD = "withheld"

#: Rows with this triage class are confirmed-valid findings. They ship only
#: bug_family, triage_class, earliest_gate, root_cause_class and
#: mechanism_class = WITHHELD_MECHANISM; every other field is WITHHELD.
WITHHELD_TRIAGE = "below_payout_floor"
WITHHELD_MECHANISM = "withheld_valid"
WITHHELD_FIELDS: tuple[str, ...] = ("family_id", "repeat_of_own", "duplicate_source", "preventable_by")

#: Columns that must never appear: a target dimension, or free text.
FORBIDDEN_COLUMNS: frozenset[str] = frozenset(
    {
        "protocol", "protocol_class", "protocol_name", "chain", "ecosystem", "language",
        "severity", "severity_claimed", "severity_assigned", "date", "submission_date",
        "module", "function", "line", "mechanism", "paid", "report_title",
        "rejection_reason", "source_row",
    }
)

BUG_FAMILIES: tuple[str, ...] = (
    "access_control", "accounting", "async_ordering", "config", "fees", "governance",
    "input_validation", "liquidation", "liveness", "oracle", "panic", "reentrancy",
    "replay", "rewards", "rounding", "secrets", "signature_binding", "state_machine",
    "token_integration",
)

#: 32 mechanism classes. Assigned privately from each report title; only the
#: class name is published.
MECHANISM_CLASSES: tuple[str, ...] = (
    "secret_material_exposure", "approval_or_allowance_overwrite", "callback_reentrancy_window",
    "signature_context_binding", "replay_scope", "input_length_validation_missing",
    "slippage_protection_missing", "boundary_condition_off_by_one", "missing_zero_check",
    "no_op_state_transition", "stale_cached_value", "unused_state_flag", "key_binding_missing",
    "role_asymmetry", "parameter_bound_missing", "missing_input_validation",
    "cross_boundary_message_ordering", "oracle_bounds_or_staleness",
    "panic_without_persisted_impact", "accounting_bias_direction", "rounding_direction",
    "liveness_block", "missing_consent_or_authorisation", "privileged_path_bypass",
    "incomplete_state_predicate", "token_classification_assumption",
    "accounting_order_of_operations", "fee_base_or_ordering", "reward_dilution",
    "self_harm_precondition", "config_dependent", "incorrect_formula_or_constant",
)
OTHER_MECHANISM = "other_mechanism"
VALID_MECHANISM: frozenset[str] = frozenset(MECHANISM_CLASSES) | {OTHER_MECHANISM, WITHHELD_MECHANISM}

#: Triage outcomes, in the private bucketing order (first matching rule wins).
TRIAGE_CLASSES: tuple[str, ...] = (
    "below_payout_floor", "spam", "duplicate", "out_of_scope", "informational", "intentional_design",
)

TRIAGE_LABELS: dict[str, str] = {
    "duplicate": "Duplicate / previously reported",
    "out_of_scope": "Out of scope - boundary or exclusion clause",
    "informational": "Informational - no security impact / no attacker profit",
    "intentional_design": "Intentional design / not a bug",
    "below_payout_floor": "Valid, but below the payout floor",
    "spam": "SPAM - fabricated impact contradicted by the PoC",
}

#: Gate id -> what the gate asks. 16 gates plus the UNCLASSIFIED sentinel.
GATE_MEANING: dict[str, str] = {
    "D1_duplicate_check": "is this already a known issue, a prior audit finding or a public disclosure?",
    "G1_severity_floor": "is there a payout tier at this severity?",
    "G2_exclusion_clause": "does an exclusion clause cover this?",
    "G3_scope_tree": "is the root cause inside the scoped files?",
    "G4_prior_audit": "is this surface already closed by a published audit?",
    "G5_test_intent": "does the project's test suite assert this is intended?",
    "design_review": "intentional design / no impact review",
    "program_ceiling": "program-level severity ceiling",
    "P7_best_practice": "best-practice critique",
    "P12_no_persisted_impact": "revert-only, no persisted impact",
    "P16_below_floor": "confirmed mechanics, below the severity floor",
    "P17_no_attacker_win": "no attacker-win / self-harm / no net-positive economics",
    "P18_design_tradeoff": "design tradeoff",
    "P19_headline_mismatch": "headline contradicted by PoC evidence",
    "P20_cooperating_party": "requires a cooperating party",
    "REALISM": "realism gates (attacker-model, config, concession)",
    "UNCLASSIFIED": "no gate token recognised",
}
VALID_GATES: frozenset[str] = frozenset(GATE_MEANING)

#: Gates answerable from a PUBLIC RECORD: the program's own pages, its published
#: audits, or its test suite. Each still needs the finding in hand - you have to
#: know what you found to ask whether a clause or an audit covers it.
PUBLIC_RECORD_GATES: frozenset[str] = frozenset(
    {"G1_severity_floor", "G2_exclusion_clause", "G3_scope_tree", "G4_prior_audit", "G5_test_intent"}
)

#: The six gates viability-gate runs before the proof of concept is written: the
#: duplicate check plus G1-G5. Historically called "pre-hunt"; the name is kept
#: for the `prehunt_catchable` aggregate, but the gates need the finding in hand
#: and D1 is not always answerable from a public record (see preventable_by).
PREHUNT_GATES: frozenset[str] = PUBLIC_RECORD_GATES | {"D1_duplicate_check"}

ROOT_CAUSES: tuple[str, ...] = (
    "duplicate_known_family", "oos_privileged_role", "admin_input_validation",
    "oos_scope_boundary", "oos_exclusion_clause", "intentional_design", "no_attacker_win",
    "no_fund_loss", "below_severity_floor", "function_visibility", "prior_audit_known",
    "gas_or_dos_only", "third_party_dependency", "cooperating_party", "spam_fabricated_impact",
    "config_dependent", "auxiliary_accounting", "best_practice", "no_demonstrated_impact", "other",
)

#: Where a duplicate signal lived. dup_signal is the private stage's reading of
#: the log; it is computed for duplicate rows and for rows whose earliest gate is
#: D1, and it is published (as duplicate_source) on duplicate rows only.
PUBLIC_DUP_SIGNALS: frozenset[str] = frozenset({"known_issue", "prior_audit", "public_disclosure"})
DUP_SIGNALS: frozenset[str] = PUBLIC_DUP_SIGNALS | {"concurrent_submission", "own_prior", "unknown", "none"}

DUPLICATE_SOURCES: frozenset[str] = (DUP_SIGNALS - {"none"}) | {"not_duplicate", WITHHELD}
PREVENTABLE_BY: frozenset[str] = frozenset({"public_record", "own_log", "none", "unknown", WITHHELD})

#: column -> allowed values, for every closed-vocabulary column.
ALLOWED: dict[str, frozenset[str]] = {
    "bug_family": frozenset(BUG_FAMILIES),
    "mechanism_class": VALID_MECHANISM,
    "triage_class": frozenset(TRIAGE_CLASSES),
    "earliest_gate": VALID_GATES,
    "root_cause_class": frozenset(ROOT_CAUSES),
    "repeat_of_own": frozenset({"true", "false", WITHHELD}),
    "duplicate_source": DUPLICATE_SOURCES,
    "preventable_by": PREVENTABLE_BY,
}


def derive_duplicate_source(triage: str, dup_signal: str, repeat_of_own: bool) -> str:
    """Published duplicate_source, from vocabulary values only.

    Not a duplicate -> not_duplicate. A duplicate takes the signal the log names;
    when the log names none but the row repeats one of the author's own earlier
    families, the signal lived in the author's own log.
    """
    if triage != "duplicate":
        return "not_duplicate"
    if dup_signal not in {"unknown", "none"}:
        return dup_signal
    return "own_prior" if repeat_of_own else "unknown"


def derive_preventable_by(gate: str, dup_signal: str, repeat_of_own: bool) -> str:
    """What could have stopped the rejection, cheapest source first.

    1. A public duplicate signal (known issue, prior audit, public disclosure),
       whatever gate the row records.
    2. A gate answerable from the program's public record (G1-G5).
    3. The author's own log: the row repeats an earlier family of the author's,
       or the log says it duplicated the author's own earlier submission.
    4. A concurrent duplicate: nothing on file could have stopped it.
    5. A duplicate-check gate, or no gate, with no recorded signal: unknown.
    6. Everything else needed the finding in hand and a judgement: none.
    """
    if dup_signal in PUBLIC_DUP_SIGNALS:
        return "public_record"
    if gate in PUBLIC_RECORD_GATES:
        return "public_record"
    if repeat_of_own or dup_signal == "own_prior":
        return "own_log"
    if dup_signal == "concurrent_submission":
        return "none"
    if gate in {"D1_duplicate_check", "UNCLASSIFIED"}:
        return "unknown"
    return "none"
