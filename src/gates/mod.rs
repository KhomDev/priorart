//! G1-G4 as quoted evidence. No gate passes or fails a finding; each reports
//! what it read and what it did not.
//!
//! G1 payout: the claimed severity against the program's tier shape and floor.
//! G2 exclusion: the finding's preconditions against the program's clause
//!    classes. A match is a prompt to read the clause, never a reject.
//! G3 scope: the paths the finding cites against the in-scope file list.
//! G4 prior art: coverage by named prior audits, plus the ranked matches the
//!    matcher found.
//!
//! Every gate that cannot read its input says so. An unrun gate is not a pass.

use crate::matching::{self, file_stem, find_unnegated, in_scope, mentions_word, Severity};
use crate::model::{Dossier, ExclusionEvidence, PayoutEvidence, PriorArt};
use crate::vocab;
use regex::Regex;
use std::sync::LazyLock;

/// The preconditions detected in a finding's prose. Presence is shown to the
/// user beside the clause it may trip; absence is never a warranty.
#[derive(Debug, Default, Clone)]
pub struct Preconditions {
    pub any_user: bool,
    pub role: Option<String>,
    pub oracle: Option<String>,
    pub front_run: Option<String>,
    pub user_input: Option<String>,
    pub off_chain: Option<String>,
    pub third_party: Option<String>,
    pub known_issue: Option<String>,
    pub testing: Option<String>,
    pub non_standard_token: Option<String>,
    pub best_practice: Option<String>,
    pub design: Option<String>,
    pub atomic: bool,
    pub fund_loss: bool,
    pub dos: bool,
}

static ROLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(owner|admin|operator|governance|guardian|governor|privileged|onlyOwner|role|capability)\b")
        .expect("role")
});
static ANY_USER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(any user|anyone|unprivileged|without permission|permissionless|no role|any attacker)\b")
        .expect("any user")
});
static ORACLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(oracle|price feed|chainlink|twap)\b").expect("oracle"));
static FRONT_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(front[- ]?run|frontrun|sandwich|mempool)\b").expect("front run")
});
static USER_INPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(user input|self[- ]inflicted|malformed input|victim.s own)\b")
        .expect("user input")
});
static OFF_CHAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(off[- ]?chain|relayer|backend|indexer|node software)\b").expect("off chain")
});
static THIRD_PARTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(third[- ]party|external dependency|external protocol|bridge)\b")
        .expect("third party")
});
static KNOWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(known issue|previously reported|already known|acknowledged)\b")
        .expect("known")
});
static TESTING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(mainnet|public testnet|denial[- ]of[- ]service testing)\b")
        .expect("testing")
});
static NON_STANDARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(fee[- ]on[- ]transfer|rebasing|non[- ]standard token)\b").expect("token")
});
static BEST_PRACTICE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(best practice|convention|code style|gas optimisation|gas optimization)\b")
        .expect("best practice")
});
static DESIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(by design|as designed|intended behaviou?r|design choice|trade[- ]off)\b")
        .expect("design")
});
static ATOMIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(atomic|flash ?loan|multicall|single transaction|same transaction)\b")
        .expect("atomic")
});
static LOSS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(drain|theft|steal|stolen|extract|transfer out|loss of funds|insolvent)\b")
        .expect("loss")
});
static DOS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(freeze|frozen|locked|denial of service|grief|liveness|halt|brick)\b")
        .expect("dos")
});

/// Detect the preconditions a finding states or implies. Lexical, and
/// documented as such in docs/gates.md.
///
/// A match only counts when it is not negated in its own clause: "no role",
/// "no privileged call", "no oracle ... is needed" and "unprivileged" are the
/// exact statements that must NOT become preconditions. An explicit any-user
/// statement also clears the role precondition, because the finding has said
/// who the attacker is.
pub fn preconditions(text: &str) -> Preconditions {
    let any_user = find_unnegated(&ANY_USER_RE, text).is_some();
    let role = if any_user {
        None
    } else {
        find_unnegated(&ROLE_RE, text)
    };
    Preconditions {
        any_user,
        role,
        oracle: find_unnegated(&ORACLE_RE, text),
        front_run: find_unnegated(&FRONT_RUN_RE, text),
        user_input: find_unnegated(&USER_INPUT_RE, text),
        off_chain: find_unnegated(&OFF_CHAIN_RE, text),
        third_party: find_unnegated(&THIRD_PARTY_RE, text),
        known_issue: find_unnegated(&KNOWN_RE, text),
        testing: find_unnegated(&TESTING_RE, text),
        non_standard_token: find_unnegated(&NON_STANDARD_RE, text),
        best_practice: find_unnegated(&BEST_PRACTICE_RE, text),
        design: find_unnegated(&DESIGN_RE, text),
        atomic: find_unnegated(&ATOMIC_RE, text).is_some(),
        fund_loss: find_unnegated(&LOSS_RE, text).is_some(),
        dos: find_unnegated(&DOS_RE, text).is_some(),
    }
}

impl Preconditions {
    /// One line describing the preconditions, printed under IS IT EXCLUDED?.
    pub fn summary(&self) -> String {
        let attacker = match (&self.role, self.any_user) {
            (Some(role), _) => format!("role: {role}"),
            (None, true) => "any user".to_string(),
            (None, false) => "not stated".to_string(),
        };
        let mut parts = vec![format!("attacker: {attacker}")];
        parts.push(format!(
            "oracle: {}",
            self.oracle.as_deref().unwrap_or("none")
        ));
        if let Some(f) = &self.front_run {
            parts.push(format!("ordering: {f}"));
        }
        if self.atomic {
            parts.push("atomic (one transaction)".into());
        }
        if let Some(d) = &self.design {
            parts.push(format!("design claim: {d}"));
        }
        parts.join("; ")
    }

    /// Why this precondition trips the given clause class, or None.
    fn trigger(&self, class: &str) -> Option<String> {
        match class {
            "privileged_role" => self.role.as_ref().map(|r| {
                format!("the finding needs a privileged role ({r}), and the clause excludes those")
            }),
            "human_error" => self.role.as_ref().filter(|_| {
                self.role.is_some() && (self.design.is_some() || !self.any_user)
            }).map(|r| format!("the finding's actor is a role ({r}); check whether it is admin error or a code defect")),
            "admin_input" => self.role.as_ref().filter(|_| {
                self.role.is_some()
            }).map(|r| format!("the finding depends on a value supplied by {r}; check whether a non-privileged actor can reach it")),
            "dos_no_extraction" => {
                if self.dos && !self.fund_loss {
                    Some("the impact reads as a freeze or griefing without fund extraction; state a loss path if one exists".into())
                } else {
                    None
                }
            }
            "front_run_only" => self
                .front_run
                .as_ref()
                .filter(|_| !self.atomic)
                .map(|f| format!("the finding needs ordering ({f}) and reads as non-atomic")),
            "best_practice" => self
                .best_practice
                .as_ref()
                .map(|p| format!("the finding reads as a convention critique ({p})")),
            "design_issue" => self
                .design
                .as_ref()
                .map(|d| format!("the finding argues about intended behaviour ({d})")),
            "off_chain" => self
                .off_chain
                .as_ref()
                .map(|o| format!("the root cause may be off-chain ({o})")),
            "third_party" => self
                .third_party
                .as_ref()
                .map(|t| format!("the finding names an external dependency ({t})")),
            "user_input" => self
                .user_input
                .as_ref()
                .map(|u| format!("the precondition is the victim's own input ({u})")),
            "known_issue" => self
                .known_issue
                .as_ref()
                .map(|k| format!("the finding itself says it is known ({k})")),
            "testing_limits" => self
                .testing
                .as_ref()
                .map(|t| format!("the proof would need live-network execution ({t})")),
            "non_standard_token" => self.non_standard_token.as_ref().map(|t| {
                format!("the finding depends on non-standard token behaviour ({t})")
            }),
            // scope_boundary is G3's job; it is reported there.
            _ => None,
        }
    }
}

/// G2: clause items whose class the finding's preconditions trip.
pub fn exclusions(dossier: &Dossier, finding_text: &str) -> Vec<ExclusionEvidence> {
    let pre = preconditions(finding_text);
    let mut out = Vec::new();
    for item in &dossier.items {
        if item.kind != "scope-clause" {
            continue;
        }
        let Some(class) = item.clause_class.as_deref() else {
            continue;
        };
        let Some(why) = pre.trigger(class) else {
            continue;
        };
        out.push(ExclusionEvidence {
            clause_class: Some(class.to_string()),
            precondition: why,
            clause: item.text.clone(),
            item_id: item.id.clone(),
            source_id: item.source_id.clone(),
            page: item.page,
            section: item.section.clone(),
        });
    }
    out
}

/// G3: what the finding cites, and whether that is in the scope list.
#[derive(Debug, Default, Clone)]
pub struct ScopeEvidence {
    pub inside: Vec<String>,
    pub outside: Vec<String>,
    /// Scope entries the dossier recorded, for the message when none match.
    pub scope_entries: usize,
}

impl ScopeEvidence {
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.scope_entries == 0 {
            out.push(
                "no in-scope file list was indexed from the program page; scope was not traced"
                    .to_string(),
            );
            return out;
        }
        if self.inside.is_empty() && self.outside.is_empty() {
            out.push(
                "the finding names no scoped file or contract, so the root cause was not traced"
                    .to_string(),
            );
            return out;
        }
        if !self.inside.is_empty() {
            out.push(format!(
                "the finding cites scoped code: {}",
                self.inside.join(", ")
            ));
        }
        if !self.outside.is_empty() {
            out.push(format!(
                "it also cites {} - check that the root cause is not there",
                self.outside.join(", ")
            ));
        }
        out
    }
}

pub fn scope(dossier: &Dossier, finding_text: &str) -> ScopeEvidence {
    let scope_entries: Vec<&str> = dossier
        .items
        .iter()
        .filter(|i| i.kind == "scope-path")
        .map(|i| i.text.as_str())
        .collect();
    let paths = matching::extract_paths(finding_text);
    let mut ev = ScopeEvidence {
        scope_entries: scope_entries.len(),
        ..Default::default()
    };
    for p in &paths {
        let bare = p.split(':').next().unwrap_or(p);
        if scope_entries.iter().any(|s| in_scope(bare, s)) {
            ev.inside.push(p.clone());
        } else {
            ev.outside.push(p.clone());
        }
    }
    // A finding usually names the contract or function, not the path. Match
    // the scope entry's file stem against the finding's words as well.
    for entry in &scope_entries {
        let Some(stem) = file_stem(entry) else {
            continue;
        };
        let already = ev
            .inside
            .iter()
            .any(|i| i.to_lowercase().contains(&stem.to_lowercase()));
        if !already && mentions_word(finding_text, stem) {
            ev.inside.push(format!("{stem} (in {entry})"));
        }
    }
    ev
}

/// G1: the claimed severity against the tier shape and floor.
pub fn payout(dossier: &Dossier, claimed: Option<Severity>) -> PayoutEvidence {
    let claimed_label = claimed.map(|c| c.label().to_string());
    let Some(p) = dossier.program.payout.as_ref() else {
        return PayoutEvidence {
            tier_shape: None,
            floor: None,
            claimed_severity: claimed_label,
            payable: None,
            citation: None,
            section: None,
            quote: None,
            detail: Some(
                "program.yaml has no payout block, so the payout gate did not run. Unknown is not payable."
                    .to_string(),
            ),
        };
    };
    let floor = floor_of(p);
    let base = PayoutEvidence {
        tier_shape: Some(p.tier_shape.clone()),
        floor: floor.clone(),
        claimed_severity: claimed_label.clone(),
        payable: None,
        citation: p.source_url.clone(),
        section: p.section.clone(),
        quote: p.quote.clone(),
        detail: None,
    };
    if p.tier_shape == "unknown" {
        return PayoutEvidence {
            detail: Some(
                "tier_shape is 'unknown': the reward table has not been transcribed, which is not the same as permissive"
                    .to_string(),
            ),
            ..base
        };
    }
    if p.source_url.as_deref().unwrap_or("").trim().is_empty() {
        return PayoutEvidence {
            detail: Some(
                "the payout terms have no source_url, so the floor is uncited and the gate reports no result"
                    .to_string(),
            ),
            ..base
        };
    }
    let Some(floor_sev) = floor.as_deref().and_then(Severity::parse) else {
        return PayoutEvidence {
            detail: Some(format!(
                "tier_shape {:?} declares no floor this build understands",
                p.tier_shape
            )),
            ..base
        };
    };
    let Some(sev) = claimed else {
        return PayoutEvidence {
            detail: Some(format!(
                "the finding claims no severity (add a 'Severity: <level>' line); the floor is {}",
                floor_sev.display()
            )),
            ..base
        };
    };
    let payable = sev >= floor_sev;
    let detail = if payable {
        Some(format!(
            "{} is at or above the floor ({})",
            sev.display(),
            floor_sev.display()
        ))
    } else {
        Some(format!(
            "{} is below the floor ({}): it pays nothing under this tier shape",
            sev.display(),
            floor_sev.display()
        ))
    };
    PayoutEvidence {
        payable: Some(payable),
        detail,
        ..base
    }
}

fn floor_of(p: &crate::model::Payout) -> Option<String> {
    let implied = vocab::floor_of_shape(&p.tier_shape).map(str::to_string);
    let declared = p
        .floor_severity
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);
    match (implied, declared) {
        (Some(a), Some(b)) => {
            let (sa, sb) = (Severity::parse(&a), Severity::parse(&b));
            match (sa, sb) {
                (Some(a), Some(b)) => Some(if a >= b { a } else { b }.label().to_string()),
                _ => Some(a),
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// G4: prior-audit coverage named in program.yaml, as evidence lines.
pub fn prior_audit_coverage(dossier: &Dossier, finding_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let paths = matching::extract_paths(finding_text);
    let audit_sources = dossier
        .items
        .iter()
        .filter(|i| i.kind == "audit-finding" || i.kind == "audit-section")
        .count();
    for audit in &dossier.program.prior_audits {
        let firm = audit.firm.as_deref().unwrap_or("unnamed firm");
        if audit.covers.is_empty() {
            out.push(format!(
                "{firm}: no 'covers' list recorded, so coverage could not be read"
            ));
            continue;
        }
        let mut hit: Vec<String> = Vec::new();
        for cover in &audit.covers {
            let by_path = paths
                .iter()
                .find(|p| in_scope(p.split(':').next().unwrap_or(p), cover));
            if let Some(p) = by_path {
                hit.push(format!("{cover} (cited path {p})"));
                continue;
            }
            if let Some(stem) = file_stem(cover) {
                if mentions_word(finding_text, stem) {
                    hit.push(format!("{cover} (cited identifier {stem})"));
                }
            }
        }
        if hit.is_empty() {
            out.push(format!(
                "{firm}: none of the cited paths is in its coverage ({})",
                audit.covers.join(", ")
            ));
        } else {
            out.push(format!(
                "{firm} covers {} - read that report's findings for this function",
                hit.join(", ")
            ));
        }
        for known in &audit.known_issues {
            let profile = matching::Profile::from_text(known);
            let finding_profile = matching::Profile::from_text(finding_text);
            let shared = profile
                .symbols
                .intersection(&finding_profile.symbols)
                .count();
            if shared >= matching::MIN_SHARED_IDENTIFIERS {
                out.push(format!(
                    "a prior-audit known issue shares {shared} identifier(s) with the finding: {}",
                    matching::collapse(known)
                ));
            }
        }
    }
    if out.is_empty() {
        if audit_sources > 0 {
            out.push(format!(
                "{audit_sources} audit section(s) are indexed; no named prior-audit coverage was recorded in program.yaml"
            ));
        } else {
            out.push("no prior-audit surface in this dossier".to_string());
        }
    }
    out
}

/// Turn ranked matches into the JSON shape, resolving each item to its source.
pub fn prior_art(
    dossier: &Dossier,
    ranked: &[matching::Ranked],
    finding_mechanism: Option<&str>,
) -> Vec<PriorArt> {
    let mut out = Vec::new();
    for row in ranked {
        let item = &dossier.items[row.index];
        let source = dossier.sources.iter().find(|s| s.id == item.source_id);
        let _ = finding_mechanism;
        out.push(PriorArt {
            item_id: item.id.clone(),
            source_id: item.source_id.clone(),
            source_kind: source.map(|s| s.kind.clone()).unwrap_or_default(),
            source_title: source
                .and_then(|s| s.title.clone())
                .or_else(|| item.title.clone()),
            path: source.map(|s| s.path.clone()).unwrap_or_default(),
            page: item.page,
            section: item.section.clone(),
            quote: item.quote.clone(),
            shared_identifiers: row.shared_identifiers.clone(),
            mechanism_class: item.mechanism_class.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Payout, Program};

    fn dossier_with(items: Vec<Item>, payout: Option<Payout>) -> Dossier {
        Dossier {
            schema: crate::model::DOSSIER_SCHEMA.into(),
            tool: crate::model::ToolInfo {
                name: "priorart".into(),
                version: "0.1.0".into(),
            },
            program: Program {
                id: "p".into(),
                payout,
                ..Default::default()
            },
            sources: vec![crate::model::Source {
                id: "src-0001".into(),
                kind: "program-page".into(),
                title: Some("scope.md".into()),
                path: "scope.md".into(),
                sha256: "0".into(),
                pages: 1,
            }],
            items,
        }
    }

    fn clause_item(text: &str, class: Option<&str>) -> Item {
        Item {
            id: "item-0001".into(),
            source_id: "src-0001".into(),
            kind: "scope-clause".into(),
            title: None,
            section: Some("Out of scope".into()),
            page: 1,
            quote: text.into(),
            identifiers: vec![],
            mechanism_class: None,
            clause_class: class.map(str::to_string),
            text: text.into(),
        }
    }

    fn terms(tier: &str, floor: Option<&str>) -> Payout {
        Payout {
            tier_shape: tier.into(),
            floor_status: Some("documented".into()),
            floor_severity: floor.map(str::to_string),
            source_url: Some("https://example.invalid/rewards".into()),
            retrieved: Some("2026-11-01".into()),
            section: Some("Rewards".into()),
            quote: Some("High: $10,000".into()),
        }
    }

    #[test]
    fn a_privileged_finding_trips_the_privileged_role_clause() {
        let d = dossier_with(
            vec![clause_item(
                "Attacks requiring access to privileged addresses",
                Some("privileged_role"),
            )],
            None,
        );
        let hits = exclusions(&d, "The admin can call setFee to drain the vault.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].clause_class.as_deref(), Some("privileged_role"));
        assert!(hits[0].precondition.contains("privileged role"));
    }

    #[test]
    fn an_any_user_atomic_finding_trips_nothing() {
        let d = dossier_with(
            vec![
                clause_item(
                    "Attacks requiring access to privileged addresses",
                    Some("privileged_role"),
                ),
                clause_item(
                    "Vulnerabilities exploitable through front-running only",
                    Some("front_run_only"),
                ),
            ],
            None,
        );
        let hits = exclusions(
            &d,
            "Any user can call withdraw(uint256) in one atomic transaction to drain the vault.",
        );
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn payout_compares_against_the_floor() {
        let d = dossier_with(vec![], Some(terms("high_and_above", Some("high"))));
        assert_eq!(payout(&d, Some(Severity::High)).payable, Some(true));
        assert_eq!(payout(&d, Some(Severity::Medium)).payable, Some(false));
        assert_eq!(payout(&d, None).payable, None);
    }

    #[test]
    fn an_unknown_tier_shape_is_not_permissive() {
        let d = dossier_with(vec![], Some(terms("unknown", None)));
        let ev = payout(&d, Some(Severity::Critical));
        assert_eq!(ev.payable, None);
        assert!(ev.detail.unwrap().contains("not the same as permissive"));
    }

    #[test]
    fn an_uncited_floor_reports_no_result() {
        let mut p = terms("high_and_above", Some("high"));
        p.source_url = None;
        let d = dossier_with(vec![], Some(p));
        let ev = payout(&d, Some(Severity::High));
        assert_eq!(ev.payable, None);
        assert!(ev.detail.unwrap().contains("no source_url"));
    }

    #[test]
    fn scope_splits_inside_from_outside() {
        let mut d = dossier_with(vec![], None);
        d.items.push(Item {
            id: "item-0002".into(),
            source_id: "src-0001".into(),
            kind: "scope-path".into(),
            title: None,
            section: Some("In scope".into()),
            page: 1,
            quote: "src/Vault.sol".into(),
            identifiers: vec![],
            mechanism_class: None,
            clause_class: None,
            text: "src/Vault.sol".into(),
        });
        let ev = scope(&d, "The bug is in src/Vault.sol and test/Vault.t.sol");
        assert_eq!(ev.inside, vec!["src/Vault.sol".to_string()]);
        assert_eq!(ev.outside.len(), 1);
        assert!(ev.lines().join(" ").contains("scoped code"));
    }
}
