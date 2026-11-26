//! Candidate generation and pairwise features.
//!
//! Given one finding and a corpus of prior items (an audit report, a known-issue
//! list, or the judged findings of the same contest), this module answers a
//! narrower question than [`crate::matching`]: which prior items are *worth
//! reading* against this finding?
//!
//! A prior item is a candidate only when the finding and the item share
//! something structural:
//!
//! - a function anchor ([`AnchorKind::Function`]);
//! - a state-variable anchor ([`AnchorKind::StateVar`]);
//! - a call-graph neighbour within two edges of a function anchor; or
//! - the same root-cause class (from [`crate::taxonomy`]) and an anchor in the
//!   same contract.
//!
//! Shared identifiers alone never make a candidate. The existing identifier
//! matcher ([`crate::matching`]) is imported read-only as one *feature* per
//! pair, so the reader can see how today's score compares with the structural
//! evidence; it never admits a pair.
//!
//! Every pair carries six named features: function-anchor overlap, state-anchor
//! overlap, call-graph distance, mechanism match, impact match and the
//! identifier score. The ordering of the returned candidates is a fixed key
//! (see [`ORDERING`]), so two runs over the same inputs are byte-identical.
//!
//! When the finding cannot be compared - the corpus is empty, no repository
//! index was supplied for a text that cites code, no anchor resolved, no
//! mechanism was asserted, or nothing qualified - the report abstains with a
//! reason. An empty candidate list without a reason would be indistinguishable
//! from "no prior art exists", which this module never claims.

mod alarm;
mod corpus;
mod ranker;

pub use alarm::{
    Alarm, AlarmBlock, AlarmCalibration, AlarmReport, ALARM_CALIBRATION_SCHEMA, ALARM_WARNING,
};
pub use corpus::{
    load_corpus, prepare_corpus, Corpus, CorpusItem, CorpusKind, PrepareReport, PreparedCorpus,
};
pub use ranker::{Ranker, RANKER_ORDERING, RANKER_WEIGHTS_SCHEMA};

use crate::anchors::{Anchor, AnchorKind, AnchorSet, IndexSet, RepoIndex};
use crate::matching::{self, Profile};
use crate::model::Item;
use crate::render::text::sanitize;
use crate::taxonomy::{self, MechanismSet};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

/// Schema id for `priorart candidates ... --json`.
pub const CANDIDATES_SCHEMA: &str = "priorart.candidates/v1";
/// Schema id of a prepared corpus file (`priorart candidates --prepare`).
pub const CANDIDATES_CORPUS_SCHEMA: &str = "priorart.candidates-corpus/v1";
/// Schema id for the summary of the prepare step.
pub const CANDIDATES_PREPARE_SCHEMA: &str = "priorart.candidates-prepare/v1";
/// How many candidates the text output and JSON return unless `--top-k` says
/// otherwise.
pub const DEFAULT_TOP_K: usize = 20;
/// The largest `--top-k` accepted. Larger values are a usage error, matching
/// `priorart check`'s treatment of its own cap.
pub const MAX_TOP_K: usize = 1_000;
/// The furthest call-graph edge count that makes a pair a candidate.
pub const MAX_CANDIDATE_DISTANCE: u32 = 2;
/// The largest prior-item text this module reads, matching the finding cap.
pub const MAX_ITEM_BYTES: u64 = 16 * 1024 * 1024;
/// The largest prepared-corpus or repository-index JSON this module loads.
pub const MAX_JSON_BYTES: u64 = 256 * 1024 * 1024;

/// The documented sort key, in the order the fields are compared.
pub const ORDERING: [&str; 5] = [
    "anchor overlap total (function shared + state shared) descending",
    "call-graph distance ascending, no distance last",
    "shared root-cause count descending",
    "identifier specific-symbol count descending, then rarity descending",
    "prior item id ascending, byte order",
];

/// Why a finding produced no candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AbstainReason {
    /// The corpus holds no prior items to compare against.
    CorpusMissing,
    /// The finding cites code, but no repository index was supplied, so no
    /// anchor can resolve.
    RepoMissing,
    /// The finding resolved no code anchors.
    NoAnchor,
    /// The finding asserts no root-cause mechanism and no prior item shares an
    /// anchor.
    NoMechanism,
    /// Anchors and/or a mechanism exist, but no prior item satisfies a rule.
    NoCandidates,
}

impl AbstainReason {
    /// Stable kebab-case id used in JSON and in the report tables.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CorpusMissing => "corpus-missing",
            Self::RepoMissing => "repo-missing",
            Self::NoAnchor => "no-anchor",
            Self::NoMechanism => "no-mechanism",
            Self::NoCandidates => "no-candidates",
        }
    }

    /// Priority when more than one reason describes the run; the first found
    /// wins. A missing corpus outranks a missing index, which outranks a
    /// finding-level gap.
    pub const fn priority(self) -> u8 {
        match self {
            Self::CorpusMissing => 10,
            Self::RepoMissing => 20,
            Self::NoAnchor => 30,
            Self::NoMechanism => 40,
            Self::NoCandidates => 50,
        }
    }
}

/// An abstention with a deterministic, host-path-free explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abstention {
    pub reason: AbstainReason,
    pub detail: String,
}

/// The structural relation that admitted a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateRule {
    /// Both texts resolve to the same function declaration.
    SharedFunction,
    /// Both texts resolve to the same state-variable declaration.
    SharedState,
    /// A function anchor of each text is at call-graph distance 1 or 2.
    CallGraph,
    /// Both texts assert a common root cause and hold an anchor in the same
    /// contract.
    MechanismContract,
}

impl CandidateRule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SharedFunction => "shared-function",
            Self::SharedState => "shared-state",
            Self::CallGraph => "call-graph",
            Self::MechanismContract => "mechanism-contract",
        }
    }
}

/// Why one prior item is a candidate, with the witness that admitted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub rule: CandidateRule,
    /// Shared anchor symbols for `shared-function` and `shared-state`.
    pub anchors: Vec<String>,
    /// The distance for `call-graph`.
    pub distance: Option<u32>,
    /// The finding's function anchors and the item's function anchors, for
    /// `call-graph`. Sorted; the witness for the distance.
    pub finding_functions: Vec<String>,
    pub prior_functions: Vec<String>,
    /// Shared root-cause ids for `mechanism-contract`.
    pub root_causes: Vec<String>,
    /// Shared contract names for `mechanism-contract`.
    pub contracts: Vec<String>,
}

impl Reason {
    fn skeleton(rule: CandidateRule) -> Self {
        Self {
            rule,
            anchors: Vec::new(),
            distance: None,
            finding_functions: Vec::new(),
            prior_functions: Vec::new(),
            root_causes: Vec::new(),
            contracts: Vec::new(),
        }
    }
}

/// A set overlap with its denominator. `ratio` is the Jaccard index
/// (`shared / union`), `null` when the union is empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlap {
    pub shared: Vec<String>,
    pub finding: usize,
    pub prior: usize,
    pub ratio: Option<f64>,
}

/// The mechanism class comparison for a pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismMatch {
    /// Root-cause class ids asserted by both texts, sorted.
    pub shared_root_causes: Vec<String>,
    /// Whether the two anchor sets hold at least one contract in common.
    pub same_contract: bool,
    /// Contract names held by both anchor sets, sorted.
    pub contracts: Vec<String>,
}

/// The impact class comparison for a pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactMatch {
    /// Impact class ids asserted by both texts, sorted.
    pub shared_impacts: Vec<String>,
}

/// The existing matcher's identifier evidence, imported read-only as a feature.
/// `specific_symbols` is the matcher's primary rank key; `rarity` is its
/// secondary key. `ranked` is false when the pair would not be returned by
/// today's matcher at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifierScore {
    pub ranked: bool,
    pub specific_symbols: usize,
    pub shared_symbols: usize,
    pub rarity: u64,
    pub shared: Vec<String>,
}

impl IdentifierScore {
    fn none() -> Self {
        Self {
            ranked: false,
            specific_symbols: 0,
            shared_symbols: 0,
            rarity: 0,
            shared: Vec::new(),
        }
    }
}

/// The six named features of one pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Features {
    pub anchor_function_overlap: Overlap,
    pub anchor_state_overlap: Overlap,
    pub call_graph_distance: Option<u32>,
    pub mechanism_match: MechanismMatch,
    pub impact_match: ImpactMatch,
    pub identifier_score: IdentifierScore,
}

/// One candidate prior item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub prior_id: String,
    pub prior_title: Option<String>,
    pub reasons: Vec<Reason>,
    pub features: Features,
    /// The PA-B4 ranker score, present only when `--ranker` supplied a
    /// weights table. Omitted from the JSON when absent, so the default
    /// output is unchanged byte for byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranker_score: Option<f64>,
}

impl Candidate {
    /// The overlap total used as the first sort key.
    fn overlap_total(&self) -> usize {
        self.features.anchor_function_overlap.shared.len()
            + self.features.anchor_state_overlap.shared.len()
    }

    fn shared_root_causes(&self) -> usize {
        self.features.mechanism_match.shared_root_causes.len()
    }
}

/// What the finding resolved, for the report summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorSummary {
    pub cites_code: bool,
    pub resolved: usize,
    pub functions: usize,
    pub state_variables: usize,
    pub contracts: Vec<String>,
    /// Citation abstentions by reason id; every citation that did not resolve
    /// is counted once.
    pub abstentions: BTreeMap<String, usize>,
}

/// The finding's mechanism classes, for the report summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismSummary {
    pub root_causes: Vec<String>,
    pub impacts: Vec<String>,
    pub abstain_reason: Option<String>,
}

/// A reference to the corpus that was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusRef {
    pub kind: String,
    pub path: String,
    pub items: usize,
    pub prepared: bool,
    /// The `repo@commit` pins a prepared corpus was built against.
    pub repositories: Vec<String>,
}

/// A reference to the repository indexes that were supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRef {
    pub supplied: bool,
    pub repositories: Vec<String>,
}

/// The finding reference in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingRef {
    pub path: String,
    pub text_sha256: String,
    pub text_bytes: usize,
}

/// Limits recorded with a run, so two runs are comparable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub top_k: usize,
    pub max_candidate_distance: u32,
    pub excluded: usize,
}

/// The whole output of `priorart candidates ... --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateReport {
    pub schema: String,
    pub tool_version: String,
    pub finding: FindingRef,
    pub corpus: CorpusRef,
    pub index: IndexRef,
    pub anchors: AnchorSummary,
    pub mechanism: MechanismSummary,
    /// `null` exactly when `candidates` is non-empty.
    pub abstention: Option<Abstention>,
    pub candidates: Vec<Candidate>,
    pub ordering: Vec<String>,
    pub limits: Limits,
    /// The PA-B5 calibrated alarm, present only when `--alarm` supplied a
    /// calibration table. Omitted from the JSON when absent, so every other
    /// output is unchanged byte for byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarm: Option<AlarmReport>,
}

/// Options for one generation run.
#[derive(Clone)]
pub struct GenerateOptions<'a> {
    /// The audited repository or repositories. `None` means no checkout was
    /// supplied; anchors then cannot resolve and the report says so.
    pub index: Option<&'a IndexSet>,
    /// The finding text SHA-256, stored in the report.
    pub finding_sha256: String,
    /// How many candidates to return, after sorting.
    pub top_k: usize,
    /// Prior item ids to leave out (for example the finding's own corpus
    /// entry).
    pub exclude: BTreeSet<String>,
    /// The PA-B4 ranker weights, when `--ranker` supplied a table. Every
    /// candidate is scored and the list is ordered by score descending, with
    /// the documented default order as the tie-break, before `top_k` applies.
    pub ranker: Option<&'a Ranker>,
    /// The PA-B5 calibration table, when `--alarm` supplied one. It reads the
    /// top candidate's ranker score, so it requires `ranker`; without a ranker
    /// the alarm reports no top score and never fires.
    pub alarm: Option<&'a Alarm>,
}

impl<'a> GenerateOptions<'a> {
    pub fn new(index: Option<&'a IndexSet>, finding_sha256: impl Into<String>) -> Self {
        Self {
            index,
            finding_sha256: finding_sha256.into(),
            top_k: DEFAULT_TOP_K,
            exclude: BTreeSet::new(),
            ranker: None,
            alarm: None,
        }
    }
}

/// One prior item after anchors and mechanism are available.
struct PreparedPrior {
    id: String,
    title: Option<String>,
    anchors: Vec<Anchor>,
    mechanism: MechanismSet,
    identifiers: Vec<String>,
}

/// Read a repository index written by `priorart anchors index --json`.
///
/// The index is read as-is: it is the caller's pinned checkout, and this
/// function neither re-parses Solidity nor checks the commit. A missing or
/// malformed file is a usage error naming the file.
pub fn load_index(path: &Path) -> Result<RepoIndex, String> {
    let meta = crate::dossier::import::require_regular_file_from(path, "repository index")?;
    if meta.len() > MAX_JSON_BYTES {
        return Err(format!(
            "repository index {} is {}; priorart reads up to {} MiB. Index a smaller checkout.",
            path.display(),
            crate::dossier::import::mib(meta.len()),
            MAX_JSON_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read repository index {}: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(format!(
            "repository index {} grew to {}; priorart reads up to {} MiB. Index a smaller checkout.",
            path.display(),
            crate::dossier::import::mib(bytes.len() as u64),
            MAX_JSON_BYTES / (1024 * 1024)
        ));
    }
    let index: RepoIndex = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{}: not a repository index JSON file: {e}. Write one with priorart anchors index --json.", path.display()))?;
    if index.repo.trim().is_empty() {
        return Err(format!(
            "{}: the repository index has an empty repository id. Reindex with priorart anchors index --repo-id.",
            path.display()
        ));
    }
    Ok(index)
}

/// Generate the candidate report for one finding text against one corpus.
///
/// The corpus items are resolved and classified here when the corpus was not
/// prepared; a prepared corpus (see [`prepare_corpus`]) skips that work and
/// uses the anchors and mechanism it records.
pub fn generate(
    finding_text: &str,
    finding_path: &Path,
    corpus: &Corpus,
    options: &GenerateOptions<'_>,
) -> CandidateReport {
    let index = options.index.filter(|i| !i.repos.is_empty());
    let function_index = index.map(|i| FunctionIndex::new(i.repos.as_slice()));

    // The finding's own anchors and mechanism.
    let finding_anchors: AnchorSet = match index {
        Some(index) => crate::anchors::resolve_text(finding_text, index),
        None => {
            let mut abstentions = Vec::new();
            if crate::anchors::cites_code(finding_text) {
                abstentions.push(crate::anchors::Abstention {
                    citation: None,
                    reason: crate::anchors::AbstainReason::RepoUnavailable,
                    detail: "no audited repository was supplied".to_string(),
                });
            } else {
                abstentions.push(crate::anchors::Abstention {
                    citation: None,
                    reason: crate::anchors::AbstainReason::NoCitation,
                    detail: "the text contains no code citation".to_string(),
                });
            }
            AnchorSet {
                schema: crate::anchors::ANCHORS_SCHEMA.to_string(),
                cites_code: crate::anchors::cites_code(finding_text),
                citations: crate::anchors::citations(finding_text),
                anchors: Vec::new(),
                abstentions,
            }
        }
    };
    let finding_mechanism = taxonomy::classify(finding_text);
    let anchor_summary = anchor_summary(&finding_anchors);
    let mechanism_summary = MechanismSummary {
        root_causes: root_cause_ids(&finding_mechanism),
        impacts: impact_ids(&finding_mechanism),
        abstain_reason: finding_mechanism.abstain_reason.clone(),
    };

    // Resolve the prior items once.
    let mut priors: Vec<PreparedPrior> = Vec::with_capacity(corpus.items.len());
    for item in &corpus.items {
        if options.exclude.contains(&item.id) {
            continue;
        }
        let anchors = match &item.anchors {
            Some(anchors) => anchors.clone(),
            None => match index {
                Some(index) => crate::anchors::resolve_text(&item.text, index).anchors,
                None => Vec::new(),
            },
        };
        let mechanism = match &item.mechanism {
            Some(mechanism) => mechanism.clone(),
            None => taxonomy::classify(&item.text),
        };
        let identifiers = if item.identifiers.is_empty() {
            Profile::from_text(&item.text).all_identifiers()
        } else {
            item.identifiers.clone()
        };
        priors.push(PreparedPrior {
            id: item.id.clone(),
            title: item.title.clone(),
            anchors,
            mechanism,
            identifiers,
        });
    }

    let identifiers = identifier_scores(&priors, finding_text);
    let distances = function_index
        .as_ref()
        .map(|fi| fi.distances(&finding_anchors.anchors));

    let mut candidates = Vec::new();
    for (i, prior) in priors.iter().enumerate() {
        let score = identifiers
            .get(&i)
            .cloned()
            .unwrap_or_else(IdentifierScore::none);
        let (features, reasons) = pair_features(
            &finding_anchors.anchors,
            &prior.anchors,
            &finding_mechanism,
            &prior.mechanism,
            function_index.as_ref(),
            distances.as_ref(),
            score,
        );
        if reasons.is_empty() {
            continue;
        }
        candidates.push(Candidate {
            prior_id: prior.id.clone(),
            prior_title: prior.title.clone(),
            reasons,
            features,
            ranker_score: None,
        });
    }
    sort_candidates(&mut candidates);
    if let Some(ranker) = options.ranker {
        for candidate in &mut candidates {
            let score = ranker.score(candidate);
            candidate.ranker_score = Some(score);
        }
        // Stable: equal scores keep the documented default order above.
        candidates.sort_by(|a, b| {
            b.ranker_score
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&a.ranker_score.unwrap_or(f64::NEG_INFINITY))
        });
    }
    let excluded = corpus
        .items
        .iter()
        .filter(|i| options.exclude.contains(&i.id))
        .count();
    let total_candidates = candidates.len();
    // The top candidate survives truncation (top_k >= 1), but evaluate first
    // so the alarm never depends on --top-k.
    let alarm = match (options.alarm, options.ranker) {
        (Some(alarm), Some(_)) => Some(alarm.evaluate(&candidates)),
        (Some(alarm), None) => Some(alarm.evaluate(&[])),
        (None, _) => None,
    };
    candidates.truncate(options.top_k.max(1));

    let abstention = if !candidates.is_empty() {
        None
    } else {
        Some(abstention_for(
            corpus,
            &finding_anchors,
            &finding_mechanism,
            index.is_some(),
            total_candidates,
        ))
    };

    CandidateReport {
        schema: CANDIDATES_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        finding: FindingRef {
            path: finding_path.to_string_lossy().replace('\\', "/"),
            text_sha256: options.finding_sha256.clone(),
            text_bytes: finding_text.len(),
        },
        corpus: CorpusRef {
            kind: corpus.kind.as_str().to_string(),
            path: corpus.path.clone(),
            items: corpus.items.len(),
            prepared: corpus.prepared,
            repositories: corpus.repositories.clone(),
        },
        index: IndexRef {
            supplied: index.is_some(),
            repositories: index.map(repository_ids).unwrap_or_default(),
        },
        anchors: anchor_summary,
        mechanism: mechanism_summary,
        abstention,
        candidates,
        ordering: if options.ranker.is_some() {
            RANKER_ORDERING.iter().map(|s| (*s).to_string()).collect()
        } else {
            ORDERING.iter().map(|s| (*s).to_string()).collect()
        },
        limits: Limits {
            top_k: options.top_k,
            max_candidate_distance: MAX_CANDIDATE_DISTANCE,
            excluded,
        },
        alarm,
    }
}

fn abstention_for(
    corpus: &Corpus,
    anchors: &AnchorSet,
    mechanism: &MechanismSet,
    has_index: bool,
    total_candidates: usize,
) -> Abstention {
    debug_assert_eq!(total_candidates, 0);
    if corpus.items.is_empty() {
        return Abstention {
            reason: AbstainReason::CorpusMissing,
            detail: "the corpus holds no prior items".to_string(),
        };
    }
    if !has_index && anchors.cites_code {
        return Abstention {
            reason: AbstainReason::RepoMissing,
            detail: "the finding cites code but no repository index was supplied".to_string(),
        };
    }
    if anchors.anchors.is_empty() {
        return Abstention {
            reason: AbstainReason::NoAnchor,
            detail: "the finding resolved no code anchors".to_string(),
        };
    }
    if mechanism.root_causes.is_empty() {
        return Abstention {
            reason: AbstainReason::NoMechanism,
            detail:
                "the finding asserts no root-cause mechanism and no prior item shares an anchor"
                    .to_string(),
        };
    }
    Abstention {
        reason: AbstainReason::NoCandidates,
        detail:
            "no prior item shares an anchor, a call-graph neighbour, or a mechanism in the same contract"
                .to_string(),
    }
}

fn anchor_summary(set: &AnchorSet) -> AnchorSummary {
    let mut functions = BTreeSet::new();
    let mut state_variables = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    for anchor in &set.anchors {
        match anchor.kind {
            AnchorKind::Function => {
                functions.insert((
                    anchor.repo.clone(),
                    anchor.file.clone(),
                    anchor.symbol.clone(),
                ));
            }
            AnchorKind::StateVar => {
                state_variables.insert((
                    anchor.repo.clone(),
                    anchor.file.clone(),
                    anchor.symbol.clone(),
                ));
            }
            AnchorKind::Contract | AnchorKind::File => {}
        }
        if let Some(contract) = contract_of(anchor) {
            contracts.insert(contract.to_string());
        }
    }
    let mut abstentions: BTreeMap<String, usize> = BTreeMap::new();
    for abstention in &set.abstentions {
        *abstentions
            .entry(abstention.reason.as_str().to_string())
            .or_default() += 1;
    }
    AnchorSummary {
        cites_code: set.cites_code,
        resolved: set.anchors.len(),
        functions: functions.len(),
        state_variables: state_variables.len(),
        contracts: contracts.into_iter().collect(),
        abstentions,
    }
}

fn repository_ids(index: &IndexSet) -> Vec<String> {
    let mut ids: Vec<String> = index
        .repos
        .iter()
        .map(|r| format!("{}@{}", r.repo, r.commit))
        .collect();
    ids.sort();
    ids
}

fn root_cause_ids(mechanism: &MechanismSet) -> Vec<String> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for m in &mechanism.root_causes {
        ids.insert(m.class.clone());
    }
    ids.into_iter().collect()
}

fn impact_ids(mechanism: &MechanismSet) -> Vec<String> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for m in &mechanism.impacts {
        ids.insert(m.class.clone());
    }
    ids.into_iter().collect()
}

/// The contract an anchor names, when it names one. A file anchor never does.
fn contract_of(anchor: &Anchor) -> Option<&str> {
    match anchor.kind {
        AnchorKind::Contract => Some(anchor.symbol.as_str()).filter(|s| !s.is_empty()),
        AnchorKind::Function | AnchorKind::StateVar => anchor
            .symbol
            .split_once('.')
            .map(|(contract, _)| contract)
            .filter(|contract| !contract.is_empty()),
        AnchorKind::File => None,
    }
}

/// The anchor identity used for overlap: repository, pinned commit, file and
/// symbol. The span is deliberately excluded, so a whole-function citation and
/// a line-range citation of the same declaration still overlap; the commit is
/// included, so two checkouts of one repository never compare as one.
fn anchor_key(anchor: &Anchor) -> (String, String, String, String) {
    (
        anchor.repo.clone(),
        anchor.commit.clone(),
        anchor.file.clone(),
        anchor.symbol.clone(),
    )
}

fn overlap(finding: &[Anchor], prior: &[Anchor], kind: AnchorKind) -> Overlap {
    let mine: BTreeSet<(String, String, String, String)> = finding
        .iter()
        .filter(|a| a.kind == kind)
        .map(anchor_key)
        .collect();
    let theirs: BTreeSet<(String, String, String, String)> = prior
        .iter()
        .filter(|a| a.kind == kind)
        .map(anchor_key)
        .collect();
    let shared: Vec<String> = mine
        .intersection(&theirs)
        .map(|(_, _, _, symbol)| symbol.clone())
        .collect();
    let union = mine.len() + theirs.len() - shared.len();
    let ratio = if union == 0 {
        None
    } else {
        Some(shared.len() as f64 / union as f64)
    };
    Overlap {
        shared,
        finding: mine.len(),
        prior: theirs.len(),
        ratio,
    }
}

/// A function id qualified by its repository and pinned commit, so two
/// checkouts - or two commits of one repository - never share graph nodes.
type FunctionKey = (String, String, String);

/// Anchor-to-function-id lookup and an undirected call graph.
pub(crate) struct FunctionIndex {
    by_anchor: BTreeMap<(String, String, String, String), FunctionKey>,
    adjacency: BTreeMap<FunctionKey, BTreeSet<FunctionKey>>,
}

impl FunctionIndex {
    pub(crate) fn new(repos: &[RepoIndex]) -> Self {
        let mut by_anchor = BTreeMap::new();
        let mut adjacency: BTreeMap<FunctionKey, BTreeSet<FunctionKey>> = BTreeMap::new();
        for repo in repos {
            for function in &repo.functions {
                let symbol = match &function.contract {
                    Some(contract) => format!("{contract}.{}", function.name),
                    None => function.name.clone(),
                };
                by_anchor.insert(
                    (
                        repo.repo.clone(),
                        repo.commit.clone(),
                        function.file.clone(),
                        symbol,
                    ),
                    (repo.repo.clone(), repo.commit.clone(), function.id.clone()),
                );
            }
            for (caller, callees) in &repo.call_graph {
                let from: FunctionKey = (repo.repo.clone(), repo.commit.clone(), caller.clone());
                for callee in callees {
                    let to: FunctionKey = (repo.repo.clone(), repo.commit.clone(), callee.clone());
                    adjacency
                        .entry(from.clone())
                        .or_default()
                        .insert(to.clone());
                    adjacency.entry(to).or_default().insert(from.clone());
                }
            }
        }
        Self {
            by_anchor,
            adjacency,
        }
    }

    fn key_of(&self, anchor: &Anchor) -> Option<FunctionKey> {
        if anchor.kind != AnchorKind::Function {
            return None;
        }
        self.by_anchor
            .get(&(
                anchor.repo.clone(),
                anchor.commit.clone(),
                anchor.file.clone(),
                anchor.symbol.clone(),
            ))
            .cloned()
    }

    /// Multi-source breadth-first distances from the finding's function
    /// anchors. One walk per finding; every prior item then costs one lookup.
    fn distances(&self, finding: &[Anchor]) -> BTreeMap<FunctionKey, u32> {
        let sources: BTreeSet<FunctionKey> =
            finding.iter().filter_map(|a| self.key_of(a)).collect();
        let mut dist: BTreeMap<FunctionKey, u32> = BTreeMap::new();
        let mut frontier: VecDeque<FunctionKey> = VecDeque::new();
        for key in sources {
            dist.insert(key.clone(), 0);
            frontier.push_back(key);
        }
        while let Some(node) = frontier.pop_front() {
            let next_distance = dist[&node] + 1;
            let Some(neighbours) = self.adjacency.get(&node) else {
                continue;
            };
            for neighbour in neighbours {
                if !dist.contains_key(neighbour) {
                    dist.insert(neighbour.clone(), next_distance);
                    frontier.push_back(neighbour.clone());
                }
            }
        }
        dist
    }
}

/// The pair features and the rules that admit the pair. Exposed to this crate
/// so the feature-value tests can build anchors by hand, without `solar`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pair_features(
    finding_anchors: &[Anchor],
    prior_anchors: &[Anchor],
    finding_mechanism: &MechanismSet,
    prior_mechanism: &MechanismSet,
    function_index: Option<&FunctionIndex>,
    distances: Option<&BTreeMap<FunctionKey, u32>>,
    identifier_score: IdentifierScore,
) -> (Features, Vec<Reason>) {
    let function_overlap = overlap(finding_anchors, prior_anchors, AnchorKind::Function);
    let state_overlap = overlap(finding_anchors, prior_anchors, AnchorKind::StateVar);

    let finding_functions: Vec<String> = finding_anchors
        .iter()
        .filter(|a| a.kind == AnchorKind::Function)
        .map(|a| a.symbol.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let prior_functions: Vec<String> = prior_anchors
        .iter()
        .filter(|a| a.kind == AnchorKind::Function)
        .map(|a| a.symbol.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let call_graph_distance = match (function_index, distances) {
        (Some(index), Some(dist)) => prior_anchors
            .iter()
            .filter_map(|a| index.key_of(a))
            .filter_map(|key| dist.get(&key).copied())
            .min(),
        _ => None,
    };

    let shared_root_causes =
        shared_classes(&finding_mechanism.root_causes, &prior_mechanism.root_causes);
    let shared_impacts = shared_classes(&finding_mechanism.impacts, &prior_mechanism.impacts);
    let mut finding_contracts: BTreeSet<String> = finding_anchors
        .iter()
        .filter_map(contract_of)
        .map(str::to_string)
        .collect();
    let prior_contracts: BTreeSet<String> = prior_anchors
        .iter()
        .filter_map(contract_of)
        .map(str::to_string)
        .collect();
    finding_contracts.retain(|contract| prior_contracts.contains(contract));
    let shared_contracts: Vec<String> = finding_contracts.into_iter().collect();

    let mechanism_match = MechanismMatch {
        shared_root_causes: shared_root_causes.clone(),
        same_contract: !shared_contracts.is_empty(),
        contracts: shared_contracts.clone(),
    };
    let impact_match = ImpactMatch { shared_impacts };

    let mut reasons = Vec::new();
    if !function_overlap.shared.is_empty() {
        let mut reason = Reason::skeleton(CandidateRule::SharedFunction);
        reason.anchors = function_overlap.shared.clone();
        reasons.push(reason);
    }
    if !state_overlap.shared.is_empty() {
        let mut reason = Reason::skeleton(CandidateRule::SharedState);
        reason.anchors = state_overlap.shared.clone();
        reasons.push(reason);
    }
    if let Some(distance) = call_graph_distance {
        // Distance 0 is a shared function anchor, already reported above.
        if (1..=MAX_CANDIDATE_DISTANCE).contains(&distance) {
            let mut reason = Reason::skeleton(CandidateRule::CallGraph);
            reason.distance = Some(distance);
            reason.finding_functions = finding_functions;
            reason.prior_functions = prior_functions;
            reasons.push(reason);
        }
    }
    if !shared_root_causes.is_empty() && !shared_contracts.is_empty() {
        let mut reason = Reason::skeleton(CandidateRule::MechanismContract);
        reason.root_causes = shared_root_causes;
        reason.contracts = shared_contracts;
        reasons.push(reason);
    }

    (
        Features {
            anchor_function_overlap: function_overlap,
            anchor_state_overlap: state_overlap,
            call_graph_distance,
            mechanism_match,
            impact_match,
            identifier_score,
        },
        reasons,
    )
}

fn shared_classes(mine: &[taxonomy::Match], theirs: &[taxonomy::Match]) -> Vec<String> {
    let mine: BTreeSet<&str> = mine.iter().map(|m| m.class.as_str()).collect();
    let theirs: BTreeSet<&str> = theirs.iter().map(|m| m.class.as_str()).collect();
    mine.intersection(&theirs)
        .map(|s| (*s).to_string())
        .collect()
}

/// Today's matcher, one row per prior item. The matcher's own threshold and
/// specificity rule decide `ranked`; this function adds no policy.
fn identifier_scores(
    priors: &[PreparedPrior],
    finding_text: &str,
) -> BTreeMap<usize, IdentifierScore> {
    if priors.is_empty() {
        return BTreeMap::new();
    }
    let items: Vec<Item> = priors
        .iter()
        .map(|prior| Item {
            id: prior.id.clone(),
            source_id: String::new(),
            kind: "prior-item".to_string(),
            title: prior.title.clone(),
            section: None,
            page: 1,
            quote: String::new(),
            identifiers: prior.identifiers.clone(),
            mechanism_class: None,
            clause_class: None,
            text: String::new(),
        })
        .collect();
    let mine = Profile::from_text(finding_text);
    let rows = matching::rank_with_profile(&items, &mine, items.len());
    let mut scores = BTreeMap::new();
    for row in rows {
        scores.insert(
            row.index,
            IdentifierScore {
                ranked: true,
                specific_symbols: row.specific_symbols,
                shared_symbols: row.shared_symbols,
                rarity: row.rarity,
                shared: row.shared_identifiers,
            },
        );
    }
    scores
}

/// The documented sort key from [`ORDERING`].
fn sort_candidates(candidates: &mut [Candidate]) {
    candidates.sort_by(|a, b| {
        b.overlap_total()
            .cmp(&a.overlap_total())
            .then_with(|| {
                match (
                    a.features.call_graph_distance,
                    b.features.call_graph_distance,
                ) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            })
            .then_with(|| b.shared_root_causes().cmp(&a.shared_root_causes()))
            .then_with(|| {
                b.features
                    .identifier_score
                    .specific_symbols
                    .cmp(&a.features.identifier_score.specific_symbols)
            })
            .then_with(|| {
                b.features
                    .identifier_score
                    .rarity
                    .cmp(&a.features.identifier_score.rarity)
            })
            .then_with(|| a.prior_id.cmp(&b.prior_id))
    });
}

/// Render one report as deterministic text, for `priorart candidates`.
pub fn render_text(report: &CandidateReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}  sha256 {}  {} bytes\n",
        sanitize(&report.finding.path),
        report.finding.text_sha256,
        report.finding.text_bytes
    ));
    out.push_str(&format!(
        "anchors: {} resolved (functions {}, state variables {})  cites code: {}\n",
        report.anchors.resolved,
        report.anchors.functions,
        report.anchors.state_variables,
        if report.anchors.cites_code {
            "yes"
        } else {
            "no"
        }
    ));
    if report.anchors.abstentions.is_empty() {
        out.push_str("anchor abstentions: none\n");
    } else {
        let parts: Vec<String> = report
            .anchors
            .abstentions
            .iter()
            .map(|(reason, count)| format!("{reason} {count}"))
            .collect();
        out.push_str(&format!("anchor abstentions: {}\n", parts.join(", ")));
    }
    out.push_str(&format!(
        "mechanism: {}  impacts: {}\n",
        joined(&report.mechanism.root_causes, "none"),
        joined(&report.mechanism.impacts, "none")
    ));
    out.push_str(&format!(
        "corpus: {}  {} item(s){}  {}\n",
        report.corpus.kind,
        report.corpus.items,
        if report.corpus.prepared {
            " (prepared)"
        } else {
            ""
        },
        sanitize(&report.corpus.path)
    ));
    if report.index.supplied {
        out.push_str(&format!(
            "index: {}\n",
            joined(&report.index.repositories, "empty index")
        ));
    } else {
        out.push_str("index: none supplied\n");
    }
    if let Some(abstention) = &report.abstention {
        out.push_str(&format!(
            "ABSTAIN {}: {}\n",
            abstention.reason.as_str(),
            sanitize(&abstention.detail)
        ));
        out.push_str("candidates: 0\n");
        if let Some(alarm) = &report.alarm {
            out.push_str(&render_alarm(alarm));
        }
        return sanitize(&out);
    }
    out.push_str(&format!(
        "candidates: {} (top-k {})\n",
        report.candidates.len(),
        report.limits.top_k
    ));
    for (i, candidate) in report.candidates.iter().enumerate() {
        let rules: Vec<&str> = candidate.reasons.iter().map(|r| r.rule.as_str()).collect();
        out.push_str(&format!(
            "  {}. {}  [{}]\n",
            i + 1,
            sanitize(&candidate.prior_id),
            rules.join(", ")
        ));
        if let Some(title) = &candidate.prior_title {
            out.push_str(&format!("     {}\n", sanitize(title)));
        }
        let f = &candidate.features;
        out.push_str(&format!(
            "     function {}/{} prior {} ratio {}  state {}/{} prior {} ratio {}\n",
            f.anchor_function_overlap.shared.len(),
            f.anchor_function_overlap.finding,
            f.anchor_function_overlap.prior,
            ratio(f.anchor_function_overlap.ratio),
            f.anchor_state_overlap.shared.len(),
            f.anchor_state_overlap.finding,
            f.anchor_state_overlap.prior,
            ratio(f.anchor_state_overlap.ratio)
        ));
        out.push_str(&format!(
            "     distance {}  mechanism {} (same contract: {})  impacts {}  identifier {} symbols (ranked: {})\n",
            f.call_graph_distance
                .map(|d| d.to_string())
                .unwrap_or_else(|| "none".to_string()),
            joined(&f.mechanism_match.shared_root_causes, "none"),
            if f.mechanism_match.same_contract { "yes" } else { "no" },
            joined(&f.impact_match.shared_impacts, "none"),
            f.identifier_score.specific_symbols,
            if f.identifier_score.ranked { "yes" } else { "no" }
        ));
        if let Some(score) = candidate.ranker_score {
            out.push_str(&format!("     ranker score {score:.6}\n"));
        }
    }
    if let Some(alarm) = &report.alarm {
        out.push_str(&render_alarm(alarm));
    }
    sanitize(&out)
}

fn render_alarm(alarm: &AlarmReport) -> String {
    let threshold = alarm
        .threshold
        .map(|t| format!("{t:.6}"))
        .unwrap_or_else(|| "none (warning off)".to_string());
    match (&alarm.top_candidate, alarm.calibrated) {
        (Some(top), Some(calibrated)) if alarm.fires => format!(
            "WARNING {}: {}  calibrated {calibrated:.6} >= threshold {threshold}\n",
            ALARM_WARNING,
            sanitize(top)
        ),
        (Some(top), Some(calibrated)) => format!(
            "alarm: no warning  top {}  calibrated {calibrated:.6}  threshold {threshold}\n",
            sanitize(top)
        ),
        _ => format!("alarm: no warning  no ranked candidate  threshold {threshold}\n"),
    }
}

fn joined(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else {
        values.join(", ")
    }
}

fn ratio(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.2}"),
        None => "none".to_string(),
    }
}

#[cfg(test)]
mod tests;
