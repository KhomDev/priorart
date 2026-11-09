//! Structured claim checks (P23-P26).
//!
//! The author's post-mortems describe reports that proved a mechanism and never
//! stated who attacks, what must already be true, what the victim does, what
//! the loss is in numbers, or which sentence of the program's own rules the
//! severity maps to. These four checks look for those pieces. They are notes
//! (weight 0): a missing heading is not the same evidence as an admission, and
//! a zero-weight note does not hide a gate that could not run.
//!
//! A `finding.yaml` manifest next to the report can supply the same pieces.
//! The CLI drops the corresponding note when the manifest field is filled in.
//! The check itself stays a text heuristic. It cannot tell a real attacker
//! model from a sentence that merely contains the word.

use crate::engine::{compile_pattern, Match};
use crate::model::Verdict;
use regex::Regex;
use std::sync::LazyLock;

fn note(detail: &str) -> Match {
    Match {
        evidence: vec![detail.to_string()],
        sentences: vec![detail.to_string()],
        verdict: Verdict::Warn,
    }
}

fn matches(re: &Regex, text: &str) -> bool {
    re.is_match(text)
}

static ATTACKER: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(attacker|adversary|malicious (user|actor|caller|address)|unprivileged (user|caller|attacker|address|account)|permissionless (attacker|caller|user)|any (user|caller|address|account) can|(admin|owner|operator|guardian|curator|manager) (can|may|could|is able to))\b",
    )
    .expect("attacker profile pattern")
});

static GENERIC_USER: LazyLock<Regex> =
    LazyLock::new(|| compile_pattern(r"\b(users?|callers?|accounts?)\b").expect("generic user"));

static PRECONDITION: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(pre-?conditions?|requirements?|requires that|must already|already (true|the case|holds)|provided that|given that|assuming)\b",
    )
    .expect("precondition")
});

static VICTIM: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(victims?|depositors?|lenders?|borrowers?|shareholders?|liquidity providers?|position holders?)\b",
    )
    .expect("victim")
});

static VICTIM_ACTION: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(victims?|users?|depositors?|borrowers?|lenders?)[^.]{0,50}\b(must|has to|needs to|calls?|withdraws?|deposits?|signs?|approves?|repays?|holds?|is forced|are forced)\b",
    )
    .expect("victim action")
});

static IMPACT_WORD: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(impact|loss|losses|drain|drains|theft|steal|stolen|freeze|frozen|lock|locked|brick|insolvency)\b",
    )
    .expect("impact word")
});

/// A number with a unit, a currency mark, or a percent. A bare year is not a
/// quantity: "2026" does not answer "how much".
static QUANTIFIED: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"(\$\s?\d|\b\d[\d,]*(?:\.\d+)?\s*(%|percent|tokens?|shares?|units?|wei|gwei|usd|dollars?)\b|(?-i:\b\d[\d,]*\s+[A-Z]{2,8}\b)|\b(loss|drain|delta|profit|penalty) of \d)",
    )
    .expect("quantified impact")
});

static SEVERITY_QUOTE: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r#"(severity rule|program rule|reward criteria|impacts? in scope)\s*:|["“][^"”]{20,160}(critical|high|medium|low)[^"”]{0,80}(reward|payout|theft|freeze|funds|eligible|bounty|scope)[^"”]{0,40}["”]"#,
    )
    .expect("quoted severity rule")
});

static SEVERITY_ASSERTION: LazyLock<Regex> = LazyLock::new(|| {
    compile_pattern(
        r"\b(suggested |claimed |assessed )?severity\s*:\s*(critical|high|medium|low|informational)\b",
    )
    .expect("severity assertion")
});

/// P23. Who can do this, and with what privilege?
pub fn attacker(text: &str) -> Option<Match> {
    if matches(&ATTACKER, text) {
        return None;
    }
    if matches(&GENERIC_USER, text) {
        return Some(note(
            "weak: a user or caller is named, but not an attacker profile (privilege, or that any unprivileged address can call)",
        ));
    }
    Some(note(
        "missing: no attacker profile (who can call this, and whether they are unprivileged)",
    ))
}

/// P24. What must already be true, and what the victim does.
///
/// Both pieces live in one check because the post-mortems record them as one
/// attacker-model entry. The manifest supplies them as `preconditions` and
/// `victim`. A report that has one and not the other is weak, not silent.
pub fn preconditions(text: &str) -> Option<Match> {
    let pre = matches(&PRECONDITION, text);
    let victim = matches(&VICTIM, text);
    let action = matches(&VICTIM_ACTION, text);
    if pre && (victim || action) {
        if victim && !action {
            return Some(note(
                "weak: a victim is named and a precondition is stated, but no victim action (what they call, sign, or are forced to do)",
            ));
        }
        return None;
    }
    if !pre && !victim && !action {
        return Some(note(
            "missing: no precondition and no victim (what must already be true, and who is harmed)",
        ));
    }
    if !pre {
        return Some(note(
            "weak: a victim is named but no precondition (what must already be true for the call to matter)",
        ));
    }
    Some(note(
        "weak: a precondition is stated but no victim and no victim action",
    ))
}

/// P25. An impact, and ideally a number.
pub fn impact(text: &str) -> Option<Match> {
    let words = matches(&IMPACT_WORD, text);
    let qty = matches(&QUANTIFIED, text);
    if qty {
        return None;
    }
    if words {
        return Some(note(
            "weak: impact is described but not quantified (no amount, percent, or currency figure)",
        ));
    }
    Some(note(
        "missing: no impact (what is lost, frozen, or stolen, and how much)",
    ))
}

/// P26. The program's own severity sentence, not the author's label.
pub fn severity_rule(text: &str) -> Option<Match> {
    if matches(&SEVERITY_QUOTE, text) {
        return None;
    }
    if matches(&SEVERITY_ASSERTION, text) {
        return Some(note(
            "weak: a severity is asserted, but the program's own severity rule is not quoted",
        ));
    }
    Some(note(
        "missing: no quoted program severity rule for the claimed tier",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unprivileged_attacker_satisfies_p23() {
        assert!(attacker("An unprivileged attacker calls withdraw.").is_none());
    }

    #[test]
    fn a_bare_user_is_a_weak_attacker_profile() {
        let m = attacker("A user can lose funds if the cache is stale.").unwrap();
        assert!(m.evidence[0].starts_with("weak:"));
    }

    #[test]
    fn a_quantified_loss_satisfies_p25() {
        assert!(impact("Impact: the attacker extracts 12.5 tokens.").is_none());
        assert!(impact("The penalty is $46,098 on this position.").is_none());
    }

    #[test]
    fn an_unquantified_loss_is_weak() {
        let m = impact("Impact: complete loss of funds, with no figure attached.").unwrap();
        assert!(m.evidence[0].starts_with("weak:"));
    }

    #[test]
    fn a_year_is_not_a_quantity() {
        let m = impact("Discovered in 2026. Impact: funds are frozen.").unwrap();
        assert!(m.evidence[0].starts_with("weak:"));
    }
}
