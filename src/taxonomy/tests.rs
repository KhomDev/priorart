//! Unit tests for the taxonomy data and the extractor.
//!
//! Every case is synthetic text written for this test. The matrix gives every
//! root-cause, impact and asset class at least one positive and one negative
//! snippet, then exercises negation, anti-cues, spans, determinism and
//! abstention directly.

use super::extract::{classify, Extractor};
use super::*;
use std::collections::BTreeMap;

struct Case {
    class: &'static str,
    text: &'static str,
    positive: bool,
}

const ROOT_CAUSE_CASES: &[Case] = &[
    Case {
        class: "reentrancy",
        text: "The withdraw function re-enters the vault through the recipient callback before the balance is committed.",
        positive: true,
    },
    Case {
        class: "reentrancy",
        text: "The implementation prevents reentrancy with a checks-effects-interactions pattern.",
        positive: false,
    },
    Case {
        class: "rounding-precision",
        text: "The fee rounds down to zero, a precision loss of one wei for small deposits.",
        positive: true,
    },
    Case {
        class: "rounding-precision",
        text: "The rounding direction is correct for every deposit.",
        positive: false,
    },
    Case {
        class: "oracle-staleness",
        text: "The oracle price is stale: the last update was three days ago.",
        positive: true,
    },
    Case {
        class: "oracle-staleness",
        text: "The staleness check is enforced before every swap.",
        positive: false,
    },
    Case {
        class: "oracle-manipulation",
        text: "An attacker can manipulate the price with a flash loan before the protocol reads it.",
        positive: true,
    },
    Case {
        class: "oracle-manipulation",
        text: "The protocol reads a manipulation-resistant oracle that the attacker cannot move.",
        positive: false,
    },
    Case {
        class: "access-control",
        text: "The setter lacks authorization, so anyone can call it.",
        positive: true,
    },
    Case {
        class: "access-control",
        text: "Access control is enforced and only the owner can set the fee.",
        positive: false,
    },
    Case {
        class: "accounting-desync",
        text: "Internal accounting is out of sync with the token balance after a direct transfer.",
        positive: true,
    },
    Case {
        class: "accounting-desync",
        text: "There is no accounting mismatch because every write updates both totals.",
        positive: false,
    },
    Case {
        class: "signature-replay",
        text: "The same signature can be replayed on another chain because the nonce is not tracked.",
        positive: true,
    },
    Case {
        class: "signature-replay",
        text: "Replay protection is enforced by a per-user nonce and the chain id.",
        positive: false,
    },
    Case {
        class: "unchecked-return",
        text: "The contract ignores the return value of the ERC20 transfer and fails to check the return of the low-level call.",
        positive: true,
    },
    Case {
        class: "unchecked-return",
        text: "The return value is checked and the success flag is validated before the next step.",
        positive: false,
    },
    Case {
        class: "dos-unbounded-loop",
        text: "The loop over all holders is unbounded and can exceed the block gas limit.",
        positive: true,
    },
    Case {
        class: "dos-unbounded-loop",
        text: "No unbounded loop exists; the loop is bounded by a fixed batch size.",
        positive: false,
    },
    Case {
        class: "frontrunning-mev",
        text: "A searcher can sandwich the swap and front-runs the user for profit.",
        positive: true,
    },
    Case {
        class: "frontrunning-mev",
        text: "Front-running is not possible because the protocol keeps transactions in a private mempool.",
        positive: false,
    },
    Case {
        class: "initialisation",
        text: "The public initializer can be called by anyone after deployment.",
        positive: true,
    },
    Case {
        class: "initialisation",
        text: "The initializer is protected and initialized exactly once.",
        positive: false,
    },
    Case {
        class: "upgrade-storage-collision",
        text: "The upgrade reorders the storage layout and causes a storage collision in the proxy.",
        positive: true,
    },
    Case {
        class: "upgrade-storage-collision",
        text: "The storage layout is preserved and storage gaps are reserved.",
        positive: false,
    },
    Case {
        class: "fee-on-transfer",
        text: "The vault credits the full amount, but the token is fee-on-transfer and the amount received is less.",
        positive: true,
    },
    Case {
        class: "fee-on-transfer",
        text: "Fee-on-transfer tokens are not supported by this market.",
        positive: false,
    },
    Case {
        class: "rebasing-token",
        text: "The share price is computed from a rebasing token, so the balance increases automatically.",
        positive: true,
    },
    Case {
        class: "rebasing-token",
        text: "This market supports non-rebasing tokens only.",
        positive: false,
    },
    Case {
        class: "liquidation-logic",
        text: "The liquidation bonus is applied twice, so liquidators seize more collateral than allowed.",
        positive: true,
    },
    Case {
        class: "liquidation-logic",
        text: "Liquidation is disabled for this market.",
        positive: false,
    },
    Case {
        class: "arithmetic-overflow",
        text: "Unchecked arithmetic lets the balance wrap around and underflow.",
        positive: true,
    },
    Case {
        class: "arithmetic-overflow",
        text: "Overflow is not possible because the contract uses checked arithmetic.",
        positive: false,
    },
    Case {
        class: "delegatecall",
        text: "The proxy performs a delegatecall to an attacker-controlled target.",
        positive: true,
    },
    Case {
        class: "delegatecall",
        text: "The delegatecall target is immutable and the implementation slot is reserved.",
        positive: false,
    },
    Case {
        class: "price-impact",
        text: "The swap has no slippage limit, so a large trade moves the price against the user.",
        positive: true,
    },
    Case {
        class: "price-impact",
        text: "The slippage protection is enforced with a user-supplied bound.",
        positive: false,
    },
    Case {
        class: "donation-inflation",
        text: "A donation attack lets the first depositor inflate the share price of the empty vault.",
        positive: true,
    },
    Case {
        class: "donation-inflation",
        text: "The vault uses virtual shares, so the donation attack is mitigated.",
        positive: false,
    },
    Case {
        class: "pause-freeze",
        text: "The admin can pause the vault and users are frozen out of their funds.",
        positive: true,
    },
    Case {
        class: "pause-freeze",
        text: "The pause does not block withdrawals, and users can still withdraw.",
        positive: false,
    },
];

const IMPACT_CASES: &[Case] = &[
    Case {
        class: "fund-loss",
        text: "An attacker can drain funds from the vault, and users suffer a permanent loss.",
        positive: true,
    },
    Case {
        class: "fund-loss",
        text: "No funds are lost because the transaction reverts.",
        positive: false,
    },
    Case {
        class: "freeze",
        text: "User funds are stuck and cannot withdraw while the market is frozen.",
        positive: true,
    },
    Case {
        class: "freeze",
        text: "Funds are not frozen and withdrawals remain open.",
        positive: false,
    },
    Case {
        class: "dos",
        text: "The attacker can cause a denial of service by exhausting the gas.",
        positive: true,
    },
    Case {
        class: "dos",
        text: "No denial of service is possible because the loop is bounded.",
        positive: false,
    },
    Case {
        class: "griefing",
        text: "Any user can block another user by front-running the withdrawal, a griefing vector.",
        positive: true,
    },
    Case {
        class: "griefing",
        text: "Griefing is not profitable here.",
        positive: false,
    },
    Case {
        class: "governance",
        text: "The attacker can hijack governance by accumulating voting power and passing a proposal.",
        positive: true,
    },
    Case {
        class: "governance",
        text: "Governance is unaffected and no proposal can pass without quorum.",
        positive: false,
    },
];

const ASSET_CASES: &[Case] = &[
    Case {
        class: "native",
        text: "The function forwards msg.value in ether to the recipient.",
        positive: true,
    },
    Case {
        class: "native",
        text: "The vault holds wrapped ether as an ERC20 token.",
        positive: false,
    },
    Case {
        class: "erc20",
        text: "The protocol accepts any ERC20 token, including fee-on-transfer tokens.",
        positive: true,
    },
    Case {
        class: "erc20",
        text: "The market lists a non-erc20 asset.",
        positive: false,
    },
    Case {
        class: "erc721-erc1155",
        text: "The vault accepts ERC721 deposits and ERC1155 batches.",
        positive: true,
    },
    Case {
        class: "erc721-erc1155",
        text: "There is no NFT support in this market.",
        positive: false,
    },
    Case {
        class: "lst-rebasing",
        text: "The strategy holds stETH, a liquid staking token.",
        positive: true,
    },
    Case {
        class: "lst-rebasing",
        text: "Only non-rebasing tokens are supported.",
        positive: false,
    },
];

/// The one asset class whose positive case needs a quoted symbol.
#[test]
fn specific_token_is_extracted_only_when_quoted() {
    let quoted = classify("The pool pairs `USDC` with WETH.");
    assert!(quoted
        .assets
        .iter()
        .any(|m| m.class == "specific-token" && m.term == "USDC"));
    let unquoted = classify("The pool pairs USDC with WETH.");
    assert!(
        !unquoted.assets.iter().any(|m| m.class == "specific-token"),
        "{unquoted:?}"
    );
    let lower = classify("The prize is paid in \"usdc\".");
    assert!(lower
        .assets
        .iter()
        .any(|m| m.class == "specific-token" && m.term == "usdc"));
}

#[test]
fn every_root_cause_class_has_a_positive_and_a_negative_case() {
    run_matrix(ROOT_CAUSE_CASES, ROOT_CAUSES, |text, class| {
        !classify(text).root_causes.iter().any(|m| m.class == class)
    });
}

#[test]
fn every_impact_class_has_a_positive_and_a_negative_case() {
    run_matrix(IMPACT_CASES, IMPACTS, |text, class| {
        !classify(text).impacts.iter().any(|m| m.class == class)
    });
}

#[test]
fn every_asset_class_except_specific_token_has_a_positive_and_a_negative_case() {
    run_matrix(ASSET_CASES, ASSETS - 1, |text, class| {
        !classify(text).assets.iter().any(|m| m.class == class)
    });
}

fn run_matrix(cases: &[Case], expected_classes: usize, absent: impl Fn(&str, &str) -> bool) {
    let mut positives: BTreeMap<&str, usize> = BTreeMap::new();
    let mut negatives: BTreeMap<&str, usize> = BTreeMap::new();
    for case in cases {
        let is_absent = absent(case.text, case.class);
        if case.positive {
            assert!(
                !is_absent,
                "class {} did not fire on its positive case: {:?}",
                case.class, case.text
            );
            *positives.entry(case.class).or_default() += 1;
        } else {
            assert!(
                is_absent,
                "class {} fired on its negative case: {:?}",
                case.class, case.text
            );
            *negatives.entry(case.class).or_default() += 1;
        }
    }
    assert_eq!(positives.len(), expected_classes, "positive coverage");
    assert_eq!(negatives.len(), expected_classes, "negative coverage");
    for (class, count) in &positives {
        assert!(*count >= 1, "{class}: positive");
        assert!(
            negatives.get(class).is_some_and(|n| *n >= 1),
            "{class}: negative"
        );
    }
}

// ---------------------------------------------------------------- negation

#[test]
fn negation_is_per_clause_and_another_clause_can_assert_the_class() {
    let text = "There is no reentrancy risk in the deposit path. However, the withdraw path re-enters the vault before the balance update.";
    let set = classify(text);
    assert!(
        set.root_causes.iter().any(|m| m.class == "reentrancy"),
        "{set:?}"
    );
    assert!(
        set.root_causes
            .iter()
            .any(|m| m.class == "reentrancy" && m.term == "re-enters"),
        "the asserting clause is the evidence: {set:?}"
    );

    let two = classify("No reentrancy risk. The oracle price is stale.");
    assert!(!two.root_causes.iter().any(|m| m.class == "reentrancy"));
    assert!(two
        .root_causes
        .iter()
        .any(|m| m.class == "oracle-staleness"));
}

#[test]
fn a_prevention_verb_negates_the_class() {
    assert!(
        !classify("The implementation prevents reentrancy with a mutex.")
            .root_causes
            .iter()
            .any(|m| m.class == "reentrancy")
    );
    assert!(
        !classify("Reentrancy is prevented by the checks-effects-interactions pattern.")
            .root_causes
            .iter()
            .any(|m| m.class == "reentrancy")
    );
    assert!(!classify("The code is immune to reentrancy.")
        .root_causes
        .iter()
        .any(|m| m.class == "reentrancy"));
}

#[test]
fn a_double_negative_asserts_the_defect() {
    let set = classify("The code is not protected against reentrancy.");
    assert!(
        set.root_causes.iter().any(|m| m.class == "reentrancy"),
        "{set:?}"
    );
    let missing = classify("Reentrancy is not prevented by the current design.");
    assert!(
        missing.root_causes.iter().any(|m| m.class == "reentrancy"),
        "{missing:?}"
    );
}

// ---------------------------------------------------------------- anti-cues

#[test]
fn an_anti_cue_suppresses_only_when_it_is_the_sole_evidence() {
    assert!(!classify("The fix adds a reentrancy guard.")
        .root_causes
        .iter()
        .any(|m| m.class == "reentrancy"));
    let later =
        classify("The fix adds a reentrancy guard. The withdrawal callback re-enters the vault.");
    assert!(
        later.root_causes.iter().any(|m| m.class == "reentrancy"),
        "{later:?}"
    );
}

#[test]
fn an_anti_cue_described_as_missing_counts_as_evidence() {
    for text in [
        "The reentrancy guard is missing.",
        "No reentrancy guard protects the function.",
        "There is not a reentrancy guard on the withdrawal path.",
    ] {
        let set = classify(text);
        assert!(
            set.root_causes.iter().any(|m| m.class == "reentrancy"),
            "{text:?} -> {set:?}"
        );
    }
}

#[test]
fn a_negated_protection_is_the_defect() {
    for (text, class) in [
        ("The code has no slippage protection.", "price-impact"),
        ("The function has no access control.", "access-control"),
        (
            "There is no replay protection on the signed order.",
            "signature-replay",
        ),
    ] {
        let set = classify(text);
        assert!(
            set.root_causes.iter().any(|m| m.class == class),
            "{text:?} -> {set:?}"
        );
    }
}

// ------------------------------------------------------------- extraction

#[test]
fn spans_are_exact_byte_ranges_into_the_input() {
    let text = "A reentrancy bug lets an attacker drain `USDC` from the vault: a fund loss.";
    let set = classify(text);
    for m in set
        .root_causes
        .iter()
        .chain(set.impacts.iter())
        .chain(set.assets.iter())
    {
        assert_eq!(
            &text[m.span.start_byte..m.span.end_byte],
            m.term,
            "span for {m:?}"
        );
    }
}

#[test]
fn matching_is_case_insensitive() {
    assert!(classify("REENTRANCY in the withdrawal path.")
        .root_causes
        .iter()
        .any(|m| m.class == "reentrancy"));
    assert!(classify("A Denial Of Service blocks the queue.")
        .impacts
        .iter()
        .any(|m| m.class == "dos"));
}

#[test]
fn word_boundaries_prevent_matches_inside_longer_words() {
    let set = classify("Kudos to the team; the macOS build is fine.");
    assert!(!set.impacts.iter().any(|m| m.class == "dos"), "{set:?}");
    let replaying = classify("The game is replaying an old match.");
    assert!(
        !replaying
            .root_causes
            .iter()
            .any(|m| m.class == "signature-replay"),
        "{replaying:?}"
    );
}

#[test]
fn non_ascii_text_does_not_panic_and_spans_stay_exact() {
    // The negation window is a byte window; a multi-byte character must not
    // make it slice a character in half. "€" is three bytes, so the 96-byte
    // window lands mid-character.
    let negated = format!("{} prevents reentrancy.", "€".repeat(60));
    let set = classify(&negated);
    assert!(
        !set.root_causes.iter().any(|m| m.class == "reentrancy"),
        "{set:?}"
    );

    let text = format!("préfixe {} reentrancy bug", "€".repeat(60));
    let set = classify(&text);
    let hit = set
        .root_causes
        .iter()
        .find(|m| m.class == "reentrancy")
        .expect("reentrancy in non-ASCII text");
    assert_eq!(&text[hit.span.start_byte..hit.span.end_byte], hit.term);
}

#[test]
fn output_is_deterministic() {
    let text = "A reentrancy bug lets an attacker drain `USDC` from the vault: a fund loss.";
    let first = classify(text);
    let second = classify(text);
    assert_eq!(first, second);
    let a = serde_json::to_string(&first).unwrap();
    let b = serde_json::to_string(&second).unwrap();
    assert_eq!(a, b);
}

#[test]
fn empty_and_no_class_text_abstain_with_no_mechanism() {
    for text in [
        "",
        "   \n\t\n",
        "The quick brown fox jumps over the lazy dog.",
    ] {
        let set = classify(text);
        assert!(set.root_causes.is_empty(), "{set:?}");
        assert!(set.impacts.is_empty(), "{set:?}");
        assert!(set.assets.is_empty(), "{set:?}");
        assert_eq!(set.abstain_reason.as_deref(), Some("no-mechanism"));
    }
}

#[test]
fn one_match_per_root_cause_and_impact_class() {
    let text = "Reentrancy. Reentrancy. The callback re-enters and the reentrancy risk is real.";
    let set = classify(text);
    let count = set
        .root_causes
        .iter()
        .filter(|m| m.class == "reentrancy")
        .count();
    assert_eq!(count, 1, "{set:?}");
}

// ---------------------------------------------------------------- taxonomy

#[test]
fn the_taxonomy_has_the_required_ids_and_counts() {
    let doc = taxonomy();
    assert_eq!(doc.version, TAXONOMY_VERSION);
    assert_eq!(doc.root_causes.len(), ROOT_CAUSES);
    assert_eq!(doc.impacts.len(), IMPACTS);
    assert_eq!(doc.assets.len(), ASSETS);
    assert_eq!(
        root_cause_ids(),
        vec![
            "reentrancy",
            "rounding-precision",
            "oracle-staleness",
            "oracle-manipulation",
            "access-control",
            "accounting-desync",
            "signature-replay",
            "unchecked-return",
            "dos-unbounded-loop",
            "frontrunning-mev",
            "initialisation",
            "upgrade-storage-collision",
            "fee-on-transfer",
            "rebasing-token",
            "liquidation-logic",
            "arithmetic-overflow",
            "delegatecall",
            "price-impact",
            "donation-inflation",
            "pause-freeze",
        ]
    );
    assert_eq!(
        impact_ids(),
        vec!["fund-loss", "freeze", "dos", "griefing", "governance"]
    );
    assert_eq!(
        asset_ids(),
        vec![
            "native",
            "erc20",
            "erc721-erc1155",
            "lst-rebasing",
            "specific-token",
        ]
    );
}

#[test]
fn every_class_is_well_formed() {
    let doc = taxonomy();
    for class in doc
        .root_causes
        .iter()
        .chain(doc.impacts.iter())
        .chain(doc.assets.iter())
    {
        assert!(!class.id.is_empty(), "empty id");
        assert!(
            class
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{}: id is not kebab-case",
            class.id
        );
        assert!(!class.name.is_empty(), "{}: no name", class.id);
        assert!(!class.definition.is_empty(), "{}: no definition", class.id);
    }
    for class in doc.root_causes.iter().chain(doc.impacts.iter()) {
        assert!(!class.cues.is_empty(), "{}: no cues", class.id);
        assert!(!class.anti_cues.is_empty(), "{}: no anti-cues", class.id);
    }
    for class in doc
        .root_causes
        .iter()
        .chain(doc.impacts.iter())
        .chain(doc.assets.iter())
    {
        for cue in class.cues.iter().chain(class.anti_cues.iter()) {
            assert_eq!(
                cue.trim(),
                cue,
                "{}: cue {cue:?} has outer whitespace",
                class.id
            );
            assert!(!cue.is_empty(), "{}: empty cue", class.id);
        }
    }
}

#[test]
fn the_extractor_compiles_and_classifies() {
    let extractor = Extractor::new(taxonomy());
    let set = extractor.classify("An attacker can drain funds with a reentrancy bug.");
    assert!(set.root_causes.iter().any(|m| m.class == "reentrancy"));
    assert!(set.impacts.iter().any(|m| m.class == "fund-loss"));
}

#[test]
fn class_lookup_spans_all_three_lists() {
    assert_eq!(
        class("reentrancy").map(|c| c.id.as_str()),
        Some("reentrancy")
    );
    assert_eq!(
        class("governance").map(|c| c.id.as_str()),
        Some("governance")
    );
    assert_eq!(class("native").map(|c| c.id.as_str()), Some("native"));
    assert!(class("not-a-class").is_none());
}
