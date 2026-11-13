//! Text output. Human-readable, deterministic, and never wider than a
//! terminal needs it to be.

use crate::dossier::build::BuildOutcome;
use crate::matching::{self, Severity};
use crate::model::{CheckReport, Dossier};
use crate::rejections;
use crate::vocab;

/// Strip control characters except newline and tab. Every user-supplied string
/// reaches the terminal through this, so a crafted finding cannot move the
/// cursor or clear the screen.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .collect()
}

fn section_header(label: &str) -> String {
    format!("{label:<22}")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// What `priorart build` prints: where the dossier went and what is in it.
pub fn dossier_summary(outcome: &BuildOutcome) -> String {
    let d = &outcome.dossier;
    let mut out = String::new();
    out.push_str(&format!(
        "priorart {} - dossier {} ({})\n",
        env!("CARGO_PKG_VERSION"),
        d.program.id,
        outcome.path.display()
    ));
    out.push_str(&format!(
        "  {} source(s), {} item(s)\n",
        d.sources.len(),
        d.items.len()
    ));
    let mut kinds: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for item in &d.items {
        *kinds.entry(item.kind.as_str()).or_insert(0) += 1;
    }
    for (kind, count) in kinds {
        out.push_str(&format!("    {kind:<16} {count}\n"));
    }
    for source in &d.sources {
        out.push_str(&format!(
            "  [{}] {}  {}  p.{}  sha256 {}\n",
            source.id,
            source.kind,
            source.path,
            source.pages,
            &source.sha256[..12.min(source.sha256.len())]
        ));
    }
    if d.program.payout.is_none() {
        out.push_str("  no payout block in program.yaml: the payout gate will report no result\n");
    }
    for note in &outcome.notes {
        out.push_str(&format!("  note: {note}\n"));
    }
    out.push_str(&format!(
        "Next: priorart check <finding.md> --dossier {}\n",
        outcome.path.display()
    ));
    sanitize(&out)
}

fn source_label(d: &Dossier, item_source: &str, item_title: Option<&str>) -> String {
    let source = d.sources.iter().find(|s| s.id == item_source);
    match source {
        Some(s) => {
            let title = s.title.clone().or_else(|| item_title.map(str::to_string));
            match title {
                Some(t) => format!("{}: {}", s.kind, t),
                None => s.kind.clone(),
            }
        }
        None => item_title.unwrap_or("source").to_string(),
    }
}

/// The three-section check output.
pub fn check(report: &CheckReport, dossier: &Dossier, finding_text: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "priorart {} - {} against dossier {} ({})\n\n",
        report.tool_version, report.finding.path, report.dossier.program_id, report.dossier.path
    ));

    // ---------------------------------------------------- HAS THIS BEEN FOUND?
    // Retrieval-only wording, by release decision: the measured false-alarm
    // rate on judged unique findings is above the plan's 30% kill criterion, so
    // 0.1.0 ranks a reading list and carries no duplicate caution. The numbers
    // and the decision live in docs/benchmark.md; no benchmark number is
    // printed by the tool or embedded in its source.
    let found_head = if !report.prior_art.is_empty() {
        format!(
            "{} to read ({} >=2 symbol identifiers)",
            plural(report.prior_art.len(), "prior item", "prior items"),
            if report.prior_art.len() == 1 {
                "shares"
            } else {
                "share"
            }
        )
    } else if report.finding.identifiers.len() < matching::MIN_SHARED_IDENTIFIERS {
        "nothing ranked (the finding names fewer than 2 symbol identifiers)".to_string()
    } else {
        "nothing ranked (no prior item shares >=2 symbol identifiers)".to_string()
    };
    out.push_str(&format!(
        "{}{}\n",
        section_header("HAS THIS BEEN FOUND?"),
        found_head
    ));
    for item in &report.prior_art {
        let label = source_label(dossier, &item.source_id, item.source_title.as_deref());
        let section = item
            .section
            .as_deref()
            .map(|s| format!(", section {s}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "  [{} p.{}{}]  \"{}\"\n",
            label,
            item.page,
            section,
            matching::collapse(&item.quote)
        ));
        out.push_str(&format!(
            "     shared: {}\n",
            item.shared_identifiers.join(", ")
        ));
        match (&item.mechanism_class, &report.finding.mechanism_class) {
            (Some(theirs), Some(yours)) if theirs == yours => {
                out.push_str(&format!("     mechanism: {theirs} (same as yours)\n"));
            }
            (Some(theirs), Some(yours)) => {
                out.push_str(&format!(
                    "     their mechanism: {theirs}   yours: {yours}\n"
                ));
            }
            (Some(theirs), None) => {
                out.push_str(&format!("     their mechanism: {theirs}\n"));
            }
            _ => {}
        }
    }
    if !report.prior_art.is_empty() {
        out.push_str(
            "  -> reading list only. priorart does not decide duplicates; see docs/benchmark.md\n",
        );
    }
    out.push('\n');

    // -------------------------------------------------------- IS IT EXCLUDED?
    let pre = crate::gates::preconditions(finding_text).summary();
    let excluded_head = if report.exclusions.is_empty() {
        format!("no clause class matches your preconditions ({pre})")
    } else {
        format!(
            "{} match your preconditions ({pre})",
            plural(report.exclusions.len(), "clause class", "clause classes")
        )
    };
    out.push_str(&format!(
        "{}{}\n",
        section_header("IS IT EXCLUDED?"),
        excluded_head
    ));
    for hit in &report.exclusions {
        let label = source_label(dossier, &hit.source_id, None);
        let section = hit
            .section
            .as_deref()
            .map(|s| format!(", section {s}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "  [{} p.{}{}]  \"{}\"\n",
            label, hit.page, section, hit.clause
        ));
        out.push_str(&format!(
            "     clause class: {}   your precondition: {}\n",
            hit.clause_class.as_deref().unwrap_or("unclassified"),
            hit.precondition
        ));
    }
    if !report.exclusions.is_empty() {
        out.push_str("  -> a match is a prompt to read the clause, not a reject.\n");
    }
    out.push('\n');

    // -------------------------------------------------------- CAN IT BE PAID?
    let paid = &report.payout;
    let head = match paid.payable {
        Some(true) => {
            let shape = paid.tier_shape.as_deref().unwrap_or("unknown");
            let pays = vocab::tier_shape(shape)
                .map(|s| s.pays.join("/"))
                .unwrap_or_default();
            format!(
                "tier shape: {shape} (pays {pays}{}); {} is payable",
                paid.floor
                    .as_deref()
                    .map(|f| format!(", floor {f}"))
                    .unwrap_or_default(),
                paid.claimed_severity
                    .as_deref()
                    .map(capitalise)
                    .unwrap_or_else(|| "the claimed severity".into())
            )
        }
        Some(false) => {
            let shape = paid.tier_shape.as_deref().unwrap_or("unknown");
            format!(
                "tier shape: {shape} (floor {}); {} is below the floor and pays nothing",
                paid.floor.as_deref().unwrap_or("unknown"),
                paid.claimed_severity
                    .as_deref()
                    .map(capitalise)
                    .unwrap_or_else(|| "the claimed severity".into())
            )
        }
        None => paid
            .detail
            .clone()
            .unwrap_or_else(|| "the payout gate reported no result".to_string()),
    };
    let cite = paid
        .citation
        .as_deref()
        .map(|c| {
            let section = paid
                .section
                .as_deref()
                .map(|s| format!(", section {s}"))
                .unwrap_or_default();
            format!(" (cited: {c}{section})")
        })
        .unwrap_or_default();
    out.push_str(&format!(
        "{}{}{}\n",
        section_header("CAN IT BE PAID?"),
        head,
        cite
    ));
    if let Some(quote) = &paid.quote {
        out.push_str(&format!("  \"{}\"\n", matching::collapse(quote)));
    }
    if let Some(detail) = &paid.detail {
        if paid.payable.is_some() {
            out.push_str(&format!("  {detail}\n"));
        }
    }
    out.push('\n');

    if !report.notes.is_empty() {
        out.push_str("NOTES\n");
        for note in &report.notes {
            out.push_str(&format!("  - {note}\n"));
        }
    }
    sanitize(&out)
}

/// `priorart vocab list`.
pub fn vocab_list() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "tier shapes ({}) - vocab/tier-shapes.yaml\n",
        vocab::tier_shapes().len()
    ));
    for shape in vocab::tier_shapes() {
        let pays = if shape.pays.is_empty() {
            "nothing".to_string()
        } else {
            shape.pays.join("/")
        };
        out.push_str(&format!("  {:<18} pays: {}\n", shape.id, pays));
    }
    out.push_str(&format!(
        "\nclause classes ({}) - vocab/clause-classes.yaml\n",
        vocab::clause_classes().len()
    ));
    for class in vocab::clause_classes() {
        out.push_str(&format!(
            "  {:<20} {}\n",
            class.id,
            class.kills.as_deref().unwrap_or("")
        ));
    }
    let cross = vocab::gate_crossref();
    out.push_str(&format!(
        "\ngate cross-reference ({}) - vocab/gate-crossref.yaml\n",
        cross.answered_here.len()
    ));
    for (gate, row) in &cross.answered_here {
        out.push_str(&format!("  {:<22} -> {}\n", gate, row.catalog));
    }
    sanitize(&out)
}

/// `priorart vocab show <name>`.
pub fn vocab_show(name: &str) -> Result<String, String> {
    let mut out = String::new();
    match name {
        "tier-shapes" => {
            out.push_str(&format!(
                "tier shapes ({}) - vocab/tier-shapes.yaml, version {}\n",
                vocab::tier_shapes().len(),
                vocab::tier_shapes_doc().version
            ));
            for shape in vocab::tier_shapes() {
                out.push_str(&format!("\n{}\n", shape.id));
                out.push_str(&format!(
                    "  pays: {}\n",
                    if shape.pays.is_empty() {
                        "nothing".to_string()
                    } else {
                        shape.pays.join(", ")
                    }
                ));
                if let Some(f) = &shape.floor {
                    out.push_str(&format!("  floor: {f}\n"));
                }
                if let Some(i) = &shape.implication {
                    out.push_str(&format!("  implication: {}\n", one_paragraph(i)));
                }
                if let Some(r) = &shape.risk {
                    out.push_str(&format!("  risk: {}\n", one_paragraph(r)));
                }
                for d in &shape.detection {
                    out.push_str(&format!("  detection: {d}\n"));
                }
                if let Some(v) = &shape.verify {
                    out.push_str(&format!("  verify: {}\n", one_paragraph(v)));
                }
            }
        }
        "clause-classes" => {
            out.push_str(&format!(
                "clause classes ({}) - vocab/clause-classes.yaml, version {}\n",
                vocab::clause_classes().len(),
                vocab::clause_classes_doc().version
            ));
            for class in vocab::clause_classes() {
                out.push_str(&format!("\n{}\n", class.id));
                if let Some(k) = &class.kills {
                    out.push_str(&format!("  kills: {}\n", one_paragraph(k)));
                }
                for w in &class.typical_wording {
                    out.push_str(&format!("  typical wording: {w}\n"));
                }
                if let Some(e) = &class.escape {
                    out.push_str(&format!("  escape: {}\n", one_paragraph(e)));
                }
                if let Some(p) = &class.suggested_pattern {
                    out.push_str(&format!("  suggested pattern: {p}\n"));
                }
            }
        }
        "gate-crossref" => {
            let cross = vocab::gate_crossref();
            out.push_str(&format!(
                "gate cross-reference - vocab/gate-crossref.yaml, version {}\n",
                cross.version
            ));
            out.push_str("\nanswered by the program's terms:\n");
            for (gate, row) in &cross.answered_here {
                out.push_str(&format!("  {gate}\n    catalog: {}\n", row.catalog));
                if let Some(read) = &row.read {
                    out.push_str(&format!("    read: {}\n", one_paragraph(read)));
                }
            }
            out.push_str("\nnot answered by the program's terms:\n");
            for (gate, note) in &cross.not_program_terms {
                out.push_str(&format!("  {gate}: {}\n", one_paragraph(note)));
            }
        }
        other => {
            return Err(format!(
                "no vocabulary named {other:?}. Use one of: tier-shapes, clause-classes, gate-crossref."
            ))
        }
    }
    Ok(sanitize(&out))
}

/// Title-case a severity label for prose ("high" -> "High").
fn capitalise(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn one_paragraph(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `priorart explain <id>`.
pub fn explain(id: &str) -> Result<String, String> {
    let mut out = String::new();
    if let Some(class) = vocab::clause_class(id) {
        out.push_str(&format!("clause class: {}\n", class.id));
        if let Some(k) = &class.kills {
            out.push_str(&format!("  kills: {}\n", one_paragraph(k)));
        }
        for w in &class.typical_wording {
            out.push_str(&format!("  typical wording: {w}\n"));
        }
        if let Some(e) = &class.escape {
            out.push_str(&format!("  escape: {}\n", one_paragraph(e)));
        }
        if let Some(p) = &class.suggested_pattern {
            out.push_str(&format!("  suggested pattern: {p}\n"));
        }
        out.push_str(
            "  a match is a prompt to read the clause, not a reject. The gate shows the clause verbatim.\n",
        );
        return Ok(sanitize(&out));
    }
    if let Some(gate) = vocab::gate(id) {
        out.push_str(&format!(
            "{}: {}\n  checks: {}\n",
            gate.id, gate.question, gate.checks
        ));
        out.push_str(
            "  this gate reports evidence. It has no pass or fail, and an unread input is reported as unread, never as a pass.\n",
        );
        return Ok(sanitize(&out));
    }
    if let Some(shape) = vocab::tier_shape(id) {
        return vocab_show("tier-shapes").map(|all| {
            let mut out = String::new();
            out.push_str(&format!("tier shape: {}\n", shape.id));
            out.push_str(&format!(
                "  pays: {}\n",
                if shape.pays.is_empty() {
                    "nothing".to_string()
                } else {
                    shape.pays.join(", ")
                }
            ));
            if let Some(f) = &shape.floor {
                out.push_str(&format!("  floor: {f}\n"));
            }
            if let Some(i) = &shape.implication {
                out.push_str(&format!("  implication: {}\n", one_paragraph(i)));
            }
            if let Some(r) = &shape.risk {
                out.push_str(&format!("  risk: {}\n", one_paragraph(r)));
            }
            let _ = all;
            sanitize(&out)
        });
    }
    Err(format!(
        "no clause class, gate or tier shape named {id:?}. Try priorart vocab list, or priorart explain G1."
    ))
}

/// `priorart rules`: the whole check surface on one page.
pub fn rules() -> String {
    let mut out = String::new();
    out.push_str("priorart answers three questions with quoted evidence, never a verdict.\n\n");
    for gate in crate::model::GATES {
        out.push_str(&format!(
            "{}  {}\n    {}\n",
            gate.id, gate.question, gate.checks
        ));
    }
    out.push_str(&format!(
        "\nHow a prior item is ranked: >= {} shared symbol identifiers (function, error, contract,\n  qualified names, snake_case, camelCase, backticked spans) move it into the list. Paths are\n  evidence and tie-break only; symbols named by more than 20% of dossier items are generic\n  and never qualify. Word overlap is not scored (measured Jaccard about 0.024). Top-k is\n  specificity, then rarity, then identifier order - never document order.\n",
        matching::MIN_SHARED_IDENTIFIERS
    ));
    out.push_str(&format!(
        "\nVocabularies: {} payout tier shapes, {} exclusion clause classes, 1 gate cross-reference.\n",
        vocab::tier_shapes().len(),
        vocab::clause_classes().len()
    ));
    out.push_str(
        "\nWhat priorart does NOT do: it does not fetch a platform page (everything is a file you\n  saved), it does not decide duplicates, and it does not check whether the PoC proves the\n  claim - that is realism-gate.\n\nExit codes: 0 analysis produced; 2 usage, IO or parse error; 1 only with\n  --fail-on-excluded or --fail-on-unpayable.\n",
    );
    sanitize(&out)
}

/// `priorart rejections summary`: the descriptive layer of the dataset.
pub fn rejections_summary() -> String {
    let data = rejections::summary();
    let mut out = String::new();
    out.push_str(&format!(
        "rejection dataset - {} logged submissions, {} published, {} withheld\n",
        rejections::u64_field(data, "total"),
        rejections::u64_field(data, "published_rows"),
        rejections::u64_field(data, "withheld_rows")
    ));
    out.push_str(&format!(
        "  pre-hunt catchable from the public record: {} of {} ({}%)\n",
        rejections::u64_field(data, "public_record_preventable"),
        rejections::u64_field(data, "total"),
        data.get("public_record_preventable_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    ));
    out.push_str("  one researcher's log: no positive class, no target dimension. See docs/rejections.md.\n\n");
    for (key, title) in [
        ("triage", "triage outcome"),
        ("gate", "earliest catchable gate"),
        ("bug_family", "bug family"),
        ("duplicate_source", "duplicate source"),
        ("preventable_by", "preventable by"),
    ] {
        out.push_str(&format!("{title}\n"));
        if let Some(map) = data.get(key).and_then(|v| v.as_object()) {
            for (name, count) in map {
                out.push_str(&format!("  {:<24} {}\n", name, count.as_u64().unwrap_or(0)));
            }
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "provenance: source sha256 {}, dataset v{}\n",
        rejections::provenance()
            .get("source_sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        rejections::provenance()
            .get("dataset_version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    ));
    out.push_str("licence: data CC BY 4.0, code MIT. See data/rejections/LICENSE-DATA.\n");
    sanitize(&out)
}

/// The severity label for a parsed severity, used by --json consumers.
pub fn severity_label(s: Severity) -> &'static str {
    s.label()
}
