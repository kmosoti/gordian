use anyhow::{Context, Result, bail};
use petgraph::Direction as PetDirection;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

const EVIDENCE_PREDICATES: &[&str] = &[
    "supportedBy",
    "qualifiedBy",
    "challengedBy",
    "groundedIn",
    "formalizedBy",
    "testedBy",
];

/// Predicates whose subgraph must stay acyclic (audit rule S6 / `KG-C003`).
///
/// The design names `supportedBy`, `qualifiedBy`, `challengedBy`, and `motivatedBy`;
/// the gap register names [`EVIDENCE_PREDICATES`]. The union is used so both hold.
const CYCLE_PREDICATES: &[&str] = &[
    "supportedBy",
    "qualifiedBy",
    "challengedBy",
    "groundedIn",
    "formalizedBy",
    "testedBy",
    "motivatedBy",
];

/// Predicates that can discharge the "cited by a Source" obligation of audit rule S1.
const SOURCE_EVIDENCE_PREDICATES: &[&str] =
    &["supportedBy", "qualifiedBy", "challengedBy", "groundedIn"];

/// Lean declaration keywords an anchor may resolve to (audit rule S3).
const LEAN_DECLARATION_KEYWORDS: &[&str] = &[
    "theorem",
    "def",
    "abbrev",
    "structure",
    "inductive",
    "lemma",
    "instance",
];

/// The single registry of audit rule identifiers. Every [`AuditFinding`] carries one of
/// these, and no identifier appears twice.
pub const AUDIT_RULES: &[&str] = &[
    RULE_MISSING_SUMMARY,
    RULE_FOUNDATION_UNSOURCED,
    RULE_SOURCE_LOCATOR,
    RULE_EVIDENCE_CYCLE,
    RULE_STATUS_VOCABULARY,
    RULE_VERIFICATION_TARGET,
    RULE_HYPOTHESIS_UNTESTED,
    RULE_THEOREM_INCOMPLETE,
    RULE_EXPERIMENT_UNTARGETED,
    RULE_CLAIM_UNSUPPORTED,
];

/// S0: a node with no prose summary.
pub const RULE_MISSING_SUMMARY: &str = "KG-D001";
/// S1: a `*Foundation` node with no evidence edge to a `Source`.
pub const RULE_FOUNDATION_UNSOURCED: &str = "KG-F001";
/// S2: a `Source` with no resolvable locator.
pub const RULE_SOURCE_LOCATOR: &str = "KG-S002";
/// S6: a directed cycle over the evidence predicates.
pub const RULE_EVIDENCE_CYCLE: &str = "KG-C003";
/// A status outside the enumeration declared for the node class.
pub const RULE_STATUS_VOCABULARY: &str = "KG-S004";
/// S3/S4: a verification target (or Lean anchor) that does not exist on disk.
pub const RULE_VERIFICATION_TARGET: &str = "KG-V005";
/// S5: a `Hypothesis` with no experiment or test.
pub const RULE_HYPOTHESIS_UNTESTED: &str = "KG-H006";
/// S3: a `Theorem` with no statement or no checker target.
pub const RULE_THEOREM_INCOMPLETE: &str = "KG-T007";
/// S4: an `Experiment` with no executable/analysis target.
pub const RULE_EXPERIMENT_UNTARGETED: &str = "KG-E008";
/// A `Claim` with no evidence, qualification, formalization, or test relation.
pub const RULE_CLAIM_UNSUPPORTED: &str = "KG-CL012";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KnowledgeGraph {
    #[serde(rename = "@context", default)]
    pub context: serde_json::Value,
    #[serde(rename = "@graph")]
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type", default)]
    pub types: Vec<String>,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// DOI, arXiv id, ISBN, or other resolvable identifier of a `Source`.
    #[serde(default)]
    pub identifier: Option<String>,
    /// ISO-8601 date on which a `Source` was retrieved.
    #[serde(default)]
    pub retrieved_at: Option<String>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub verification: Vec<Verification>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Relation {
    pub predicate: String,
    pub target: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Verification {
    pub method: String,
    pub target: String,
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone)]
pub struct Neighbor {
    pub direction: Direction,
    pub predicate: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    pub node_types: BTreeMap<String, usize>,
    pub predicates: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditFinding {
    /// Stable rule identifier drawn from [`AUDIT_RULES`].
    pub rule: &'static str,
    pub severity: FindingSeverity,
    pub node_id: String,
    pub message: String,
}

/// Allowed `status` values, per node class and for `verification[].status`.
///
/// A node class with no entry is unconstrained; a class with an entry is closed, so an
/// unlisted value is an error. The built-in table is the enumeration the repository's own
/// documents already fix; `knowledge/status-vocabulary.json`, when present, replaces it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusVocabulary {
    /// `@type` -> allowed `status` values.
    #[serde(default)]
    pub node_status: BTreeMap<String, BTreeSet<String>>,
    /// Allowed `verification[].status` values; empty means unconstrained.
    #[serde(default)]
    pub verification_status: BTreeSet<String>,
}

/// The fixed path, relative to the repository root, of an overriding vocabulary.
pub const STATUS_VOCABULARY_PATH: &str = "knowledge/status-vocabulary.json";

impl Default for StatusVocabulary {
    fn default() -> Self {
        // Theorem states are fixed by docs/formal/theorem-catalog.md "## Verification states".
        let theorem = [
            "proof-source-present",
            "machine-checked",
            "model-only",
            "planned",
        ];
        // Verification entry states: what a checker run can currently report about a target.
        let verification = ["planned", "ci-check-required", "passing", "machine-checked"];
        let mut node_status = BTreeMap::new();
        node_status.insert(
            "Theorem".to_owned(),
            theorem.iter().map(|value| (*value).to_owned()).collect(),
        );
        Self {
            node_status,
            verification_status: verification
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        }
    }
}

impl StatusVocabulary {
    /// Read a vocabulary from an explicit path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read status vocabulary {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse status vocabulary {}", path.display()))
    }

    fn allows_node(&self, node_type: &str, status: Option<&str>) -> Option<bool> {
        let allowed = self.node_status.get(node_type)?;
        Some(status.is_some_and(|status| allowed.contains(status)))
    }

    fn allows_verification(&self, status: &str) -> bool {
        self.verification_status.is_empty() || self.verification_status.contains(status)
    }
}

/// Everything `audit` needs beyond the graph itself.
#[derive(Debug, Clone, Default)]
pub struct AuditContext {
    repo_root: Option<PathBuf>,
    vocabulary: StatusVocabulary,
}

impl AuditContext {
    /// Audit against a repository checkout: verification targets are resolved relative to
    /// `repo_root`, and `knowledge/status-vocabulary.json` replaces the built-in vocabulary
    /// when it exists.
    pub fn for_repo(repo_root: impl Into<PathBuf>) -> Result<Self> {
        let repo_root = repo_root.into();
        let vocabulary_path = repo_root.join(STATUS_VOCABULARY_PATH);
        let vocabulary = if vocabulary_path.is_file() {
            StatusVocabulary::load(&vocabulary_path)?
        } else {
            StatusVocabulary::default()
        };
        Ok(Self {
            repo_root: Some(repo_root),
            vocabulary,
        })
    }

    /// Audit without a checkout: rules that read the filesystem are not evaluated.
    pub fn detached() -> Self {
        Self::default()
    }

    /// Replace the status vocabulary.
    #[must_use]
    pub fn with_vocabulary(mut self, vocabulary: StatusVocabulary) -> Self {
        self.vocabulary = vocabulary;
        self
    }

    /// The repository root this audit resolves paths against, when there is one.
    #[must_use]
    pub fn repo_root(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct IndexedGraph {
    graph: StableDiGraph<String, String>,
    by_id: HashMap<String, NodeIndex>,
}

impl KnowledgeGraph {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.is_dir() {
            Self::load_directory(path)
        } else {
            Self::load_file(path)
        }
    }

    fn load_file(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read knowledge graph {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse knowledge graph {}", path.display()))
    }

    fn load_directory(path: &Path) -> Result<Self> {
        let mut files = fs::read_dir(path)
            .with_context(|| format!("failed to list knowledge graph {}", path.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<PathBuf>>>()?;
        files.retain(|file| file.extension().is_some_and(|ext| ext == "jsonld"));
        files.sort();

        if files.is_empty() {
            bail!(
                "knowledge graph directory {} has no .jsonld shards",
                path.display()
            );
        }

        let mut context = serde_json::Value::Null;
        let mut nodes = Vec::new();
        for file in files {
            let shard = Self::load_file(&file)?;
            if context.is_null() && !shard.context.is_null() {
                context = shard.context;
            }
            nodes.extend(shard.nodes);
        }

        Ok(Self { context, nodes })
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                bail!("node has an empty @id");
            }
            if node.name.trim().is_empty() {
                bail!("node {} has an empty name", node.id);
            }
            if node.types.is_empty() {
                bail!("node {} has no @type", node.id);
            }
            if !ids.insert(node.id.as_str()) {
                bail!("duplicate node id: {}", node.id);
            }
        }

        for node in &self.nodes {
            for relation in &node.relations {
                if relation.predicate.trim().is_empty() {
                    bail!("node {} has a relation with an empty predicate", node.id);
                }
                if !ids.contains(relation.target.as_str()) {
                    bail!(
                        "node {} relation {} targets unknown node {}",
                        node.id,
                        relation.predicate,
                        relation.target
                    );
                }
            }
        }
        Ok(())
    }

    /// Evaluate every audit rule in [`AUDIT_RULES`] over the merged graph.
    pub fn audit(&self, context: &AuditContext) -> Vec<AuditFinding> {
        let by_id: HashMap<&str, &Node> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut findings = Vec::new();

        for node in &self.nodes {
            if node.summary.trim().is_empty() {
                findings.push(finding(
                    RULE_MISSING_SUMMARY,
                    FindingSeverity::Warning,
                    node,
                    "missing summary".to_owned(),
                ));
            }
            if node.has_type("Source")
                && node.url.as_deref().is_none_or(str::is_empty)
                && node.identifier.as_deref().is_none_or(str::is_empty)
            {
                findings.push(finding(
                    RULE_SOURCE_LOCATOR,
                    FindingSeverity::Error,
                    node,
                    "Source node has neither a url nor an identifier (DOI/arXiv) locator"
                        .to_owned(),
                ));
            }
            if node.is_foundation()
                && !node.has_type("Source")
                && !node.cites_source(SOURCE_EVIDENCE_PREDICATES, &by_id)
            {
                findings.push(finding(
                    RULE_FOUNDATION_UNSOURCED,
                    FindingSeverity::Error,
                    node,
                    format!(
                        "Foundation node has no {} relation to a Source node",
                        SOURCE_EVIDENCE_PREDICATES.join("/")
                    ),
                ));
            }
            if node.has_type("Claim") && !node.has_any_relation(EVIDENCE_PREDICATES) {
                findings.push(finding(
                    RULE_CLAIM_UNSUPPORTED,
                    FindingSeverity::Warning,
                    node,
                    "Claim has no evidence, qualification, formalization, or test relation"
                        .to_owned(),
                ));
            }
            if node.has_type("Hypothesis")
                && !node.has_relation("testedBy")
                && node.verification.is_empty()
            {
                findings.push(finding(
                    RULE_HYPOTHESIS_UNTESTED,
                    FindingSeverity::Error,
                    node,
                    "Hypothesis has no experiment/test target".to_owned(),
                ));
            }
            if node.has_type("Theorem") {
                if node.statement.as_deref().is_none_or(str::is_empty) {
                    findings.push(finding(
                        RULE_THEOREM_INCOMPLETE,
                        FindingSeverity::Error,
                        node,
                        "Theorem has no formal statement".to_owned(),
                    ));
                }
                if node.verification.is_empty() {
                    findings.push(finding(
                        RULE_THEOREM_INCOMPLETE,
                        FindingSeverity::Error,
                        node,
                        "Theorem has no checker target".to_owned(),
                    ));
                }
            }
            if node.has_type("Experiment") && node.verification.is_empty() {
                findings.push(finding(
                    RULE_EXPERIMENT_UNTARGETED,
                    FindingSeverity::Warning,
                    node,
                    "Experiment has no executable/analysis target".to_owned(),
                ));
            }
            findings.extend(self.audit_status(node, context));
            findings.extend(self.audit_verification_targets(node, context));
        }

        findings.extend(self.audit_evidence_cycles());
        findings
    }

    /// A status outside the closed enumeration declared for one of the node's classes.
    fn audit_status(&self, node: &Node, context: &AuditContext) -> Vec<AuditFinding> {
        let mut findings = Vec::new();
        for node_type in &node.types {
            match context
                .vocabulary
                .allows_node(node_type, node.status.as_deref())
            {
                None | Some(true) => {}
                Some(false) => findings.push(finding(
                    RULE_STATUS_VOCABULARY,
                    FindingSeverity::Error,
                    node,
                    format!(
                        "status {} is not in the enumeration for @type {node_type}",
                        node.status.as_deref().unwrap_or("<absent>")
                    ),
                )),
            }
        }
        for entry in &node.verification {
            if !context.vocabulary.allows_verification(&entry.status) {
                findings.push(finding(
                    RULE_STATUS_VOCABULARY,
                    FindingSeverity::Error,
                    node,
                    format!(
                        "verification status {} is not in the enumeration for verification entries",
                        entry.status
                    ),
                ));
            }
        }
        findings
    }

    /// A verification target that names a path, or a Lean declaration, which is not there.
    fn audit_verification_targets(&self, node: &Node, context: &AuditContext) -> Vec<AuditFinding> {
        let Some(root) = context.repo_root() else {
            return Vec::new();
        };
        let mut findings = Vec::new();
        for entry in &node.verification {
            if entry.status == "planned" {
                continue;
            }
            let (path, anchor) = split_anchor(&entry.target);
            if path.is_empty() {
                continue;
            }
            let resolved = root.join(path);
            if !resolved.exists() {
                findings.push(finding(
                    RULE_VERIFICATION_TARGET,
                    FindingSeverity::Error,
                    node,
                    format!("verification target {path} does not exist"),
                ));
                continue;
            }
            let Some(anchor) = anchor else { continue };
            if entry.method != "Lean4" {
                continue;
            }
            match fs::read_to_string(&resolved) {
                Ok(source) => {
                    if !declares(&source, anchor) {
                        findings.push(finding(
                            RULE_VERIFICATION_TARGET,
                            FindingSeverity::Error,
                            node,
                            format!("{path} declares no Lean anchor {anchor}"),
                        ));
                    }
                }
                Err(error) => findings.push(finding(
                    RULE_VERIFICATION_TARGET,
                    FindingSeverity::Error,
                    node,
                    format!("verification target {path} is unreadable: {error}"),
                )),
            }
        }
        findings
    }

    /// Directed cycles over [`CYCLE_PREDICATES`], reported in cycle order.
    fn audit_evidence_cycles(&self) -> Vec<AuditFinding> {
        let index: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(position, node)| (node.id.as_str(), position))
            .collect();
        let adjacency: Vec<Vec<usize>> = self
            .nodes
            .iter()
            .map(|node| {
                node.relations
                    .iter()
                    .filter(|relation| CYCLE_PREDICATES.contains(&relation.predicate.as_str()))
                    .filter_map(|relation| index.get(relation.target.as_str()).copied())
                    .collect()
            })
            .collect();

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            Unseen,
            OnStack,
            Done,
        }

        let mut marks = vec![Mark::Unseen; self.nodes.len()];
        let mut stack = Vec::new();
        let mut reported = HashSet::new();
        let mut findings = Vec::new();

        // Iterative depth-first search: the corpus is small, but a recursive walk over an
        // adversarial graph would overflow the stack, and this rule exists to survive
        // adversarial graphs.
        for root in 0..self.nodes.len() {
            if marks[root] != Mark::Unseen {
                continue;
            }
            let mut work = vec![(root, 0_usize)];
            marks[root] = Mark::OnStack;
            stack.push(root);
            while let Some((node, cursor)) = work.last_mut() {
                let node = *node;
                if let Some(&next) = adjacency[node].get(*cursor) {
                    *cursor += 1;
                    match marks[next] {
                        Mark::Unseen => {
                            marks[next] = Mark::OnStack;
                            stack.push(next);
                            work.push((next, 0));
                        }
                        Mark::OnStack => {
                            let start = stack
                                .iter()
                                .position(|entry| *entry == next)
                                .unwrap_or_default();
                            let mut cycle: Vec<&str> = stack[start..]
                                .iter()
                                .map(|entry| self.nodes[*entry].id.as_str())
                                .collect();
                            cycle.push(self.nodes[next].id.as_str());
                            let key: Vec<&str> = {
                                let mut sorted = cycle.clone();
                                sorted.sort_unstable();
                                sorted.dedup();
                                sorted
                            };
                            if reported.insert(key) {
                                findings.push(AuditFinding {
                                    rule: RULE_EVIDENCE_CYCLE,
                                    severity: FindingSeverity::Error,
                                    node_id: self.nodes[next].id.clone(),
                                    message: format!(
                                        "evidence predicate cycle: {}",
                                        cycle.join(" -> ")
                                    ),
                                });
                            }
                        }
                        Mark::Done => {}
                    }
                } else {
                    marks[node] = Mark::Done;
                    stack.pop();
                    work.pop();
                }
            }
        }
        findings
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn nodes_of_type(&self, kind: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.has_type(kind))
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&Node> {
        let query = query.to_lowercase();
        self.nodes
            .iter()
            .filter(|node| {
                node.id.to_lowercase().contains(&query)
                    || node.name.to_lowercase().contains(&query)
                    || node.summary.to_lowercase().contains(&query)
                    || node
                        .statement
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
            })
            .collect()
    }

    pub fn stats(&self) -> GraphStats {
        let mut node_types = BTreeMap::new();
        let mut predicates = BTreeMap::new();
        let mut edges = 0;
        for node in &self.nodes {
            for node_type in &node.types {
                *node_types.entry(node_type.clone()).or_insert(0) += 1;
            }
            for relation in &node.relations {
                edges += 1;
                *predicates.entry(relation.predicate.clone()).or_insert(0) += 1;
            }
        }
        GraphStats {
            nodes: self.nodes.len(),
            edges,
            node_types,
            predicates,
        }
    }

    pub fn indexed(&self) -> Result<IndexedGraph> {
        self.validate()?;
        let mut graph = StableDiGraph::with_capacity(
            self.nodes.len(),
            self.nodes.iter().map(|node| node.relations.len()).sum(),
        );
        let mut by_id = HashMap::with_capacity(self.nodes.len());

        for node in &self.nodes {
            by_id.insert(node.id.clone(), graph.add_node(node.id.clone()));
        }
        for node in &self.nodes {
            let source = by_id[&node.id];
            for relation in &node.relations {
                graph.add_edge(source, by_id[&relation.target], relation.predicate.clone());
            }
        }
        Ok(IndexedGraph { graph, by_id })
    }

    pub fn neighbors(&self, id: &str, predicate: Option<&str>) -> Result<Vec<Neighbor>> {
        self.indexed()?.neighbors(id, predicate)
    }

    pub fn path(&self, from: &str, to: &str) -> Result<Option<Vec<String>>> {
        self.indexed()?.path(from, to)
    }

    pub fn evidence_for(&self, id: &str) -> Result<Vec<Neighbor>> {
        Ok(self
            .neighbors(id, None)?
            .into_iter()
            .filter(|neighbor| EVIDENCE_PREDICATES.contains(&neighbor.predicate.as_str()))
            .collect())
    }

    pub fn to_dot(&self) -> Result<String> {
        self.indexed().map(|indexed| indexed.to_dot())
    }
}

impl Node {
    fn has_type(&self, kind: &str) -> bool {
        self.types.iter().any(|ty| ty.eq_ignore_ascii_case(kind))
    }

    fn has_relation(&self, predicate: &str) -> bool {
        self.relations
            .iter()
            .any(|relation| relation.predicate == predicate)
    }

    fn has_any_relation(&self, predicates: &[&str]) -> bool {
        self.relations
            .iter()
            .any(|relation| predicates.contains(&relation.predicate.as_str()))
    }

    /// `ScientificFoundation`, `EngineeringFoundation`, `StandardFoundation`, ...
    fn is_foundation(&self) -> bool {
        self.types
            .iter()
            .any(|kind| kind.to_ascii_lowercase().ends_with("foundation"))
    }

    fn cites_source(&self, predicates: &[&str], by_id: &HashMap<&str, &Node>) -> bool {
        self.relations.iter().any(|relation| {
            predicates.contains(&relation.predicate.as_str())
                && by_id
                    .get(relation.target.as_str())
                    .is_some_and(|target| target.has_type("Source"))
        })
    }
}

impl IndexedGraph {
    pub fn neighbors(&self, id: &str, predicate: Option<&str>) -> Result<Vec<Neighbor>> {
        let Some(&index) = self.by_id.get(id) else {
            bail!("unknown node: {id}");
        };
        let mut neighbors = Vec::new();
        for edge in self.graph.edges_directed(index, PetDirection::Outgoing) {
            if predicate.is_none_or(|wanted| edge.weight() == wanted) {
                neighbors.push(Neighbor {
                    direction: Direction::Outgoing,
                    predicate: edge.weight().clone(),
                    node_id: self.graph[edge.target()].clone(),
                });
            }
        }
        for edge in self.graph.edges_directed(index, PetDirection::Incoming) {
            if predicate.is_none_or(|wanted| edge.weight() == wanted) {
                neighbors.push(Neighbor {
                    direction: Direction::Incoming,
                    predicate: edge.weight().clone(),
                    node_id: self.graph[edge.source()].clone(),
                });
            }
        }
        neighbors.sort_by(|left, right| {
            left.predicate
                .cmp(&right.predicate)
                .then_with(|| left.node_id.cmp(&right.node_id))
        });
        Ok(neighbors)
    }

    pub fn path(&self, from: &str, to: &str) -> Result<Option<Vec<String>>> {
        let Some(&from_index) = self.by_id.get(from) else {
            bail!("unknown node: {from}");
        };
        let Some(&to_index) = self.by_id.get(to) else {
            bail!("unknown node: {to}");
        };
        if from_index == to_index {
            return Ok(Some(vec![from.to_owned()]));
        }

        let mut queue = VecDeque::from([from_index]);
        let mut visited = HashSet::from([from_index]);
        let mut previous = HashMap::<NodeIndex, NodeIndex>::new();

        while let Some(current) = queue.pop_front() {
            for next in self
                .graph
                .neighbors_directed(current, PetDirection::Outgoing)
            {
                if !visited.insert(next) {
                    continue;
                }
                previous.insert(next, current);
                if next == to_index {
                    let mut path = vec![to_index];
                    let mut cursor = to_index;
                    while cursor != from_index {
                        cursor = previous[&cursor];
                        path.push(cursor);
                    }
                    path.reverse();
                    return Ok(Some(
                        path.into_iter()
                            .map(|index| self.graph[index].clone())
                            .collect(),
                    ));
                }
                queue.push_back(next);
            }
        }
        Ok(None)
    }

    pub fn to_dot(&self) -> String {
        let mut out = String::from("digraph gordian_knowledge {\n");
        for index in self.graph.node_indices() {
            let id = escape_dot(&self.graph[index]);
            out.push_str(&format!("  \"{id}\";\n"));
        }
        for edge in self.graph.edge_references() {
            let source = escape_dot(&self.graph[edge.source()]);
            let target = escape_dot(&self.graph[edge.target()]);
            let predicate = escape_dot(edge.weight());
            out.push_str(&format!(
                "  \"{source}\" -> \"{target}\" [label=\"{predicate}\"];\n"
            ));
        }
        out.push_str("}\n");
        out
    }
}

fn finding(
    rule: &'static str,
    severity: FindingSeverity,
    node: &Node,
    message: String,
) -> AuditFinding {
    AuditFinding {
        rule,
        severity,
        node_id: node.id.clone(),
        message,
    }
}

/// Split `path#anchor` into its path and optional anchor.
fn split_anchor(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((path, anchor)) if !anchor.is_empty() => (path, Some(anchor)),
        Some((path, _)) => (path, None),
        None => (target, None),
    }
}

/// Whether `source` contains a top-level Lean declaration named `anchor`.
fn declares(source: &str, anchor: &str) -> bool {
    source.lines().any(|line| {
        LEAN_DECLARATION_KEYWORDS.iter().any(|keyword| {
            line.strip_prefix(keyword)
                .and_then(|rest| rest.strip_prefix(char::is_whitespace))
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix(anchor))
                .is_some_and(|rest| {
                    rest.chars()
                        .next()
                        .is_none_or(|next| !next.is_alphanumeric() && next != '_' && next != '\'')
                })
        })
    })
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn node(id: &str, kind: &str) -> Node {
        Node {
            id: id.to_owned(),
            types: vec![kind.to_owned()],
            name: id.to_owned(),
            summary: format!("summary of {id}"),
            status: None,
            statement: None,
            url: None,
            identifier: None,
            retrieved_at: None,
            relations: vec![],
            verification: vec![],
        }
    }

    fn relation(predicate: &str, target: &str) -> Relation {
        Relation {
            predicate: predicate.to_owned(),
            target: target.to_owned(),
            note: None,
        }
    }

    fn graph(nodes: Vec<Node>) -> KnowledgeGraph {
        KnowledgeGraph {
            context: serde_json::Value::Null,
            nodes,
        }
    }

    fn fixture() -> KnowledgeGraph {
        let mut claim = node("claim:a", "Claim");
        claim.statement = Some("A is supported".into());
        claim.relations = vec![relation("supportedBy", "source:b")];

        let mut source = node("source:b", "Source");
        source.url = Some("https://example.test/b".into());
        source.retrieved_at = Some("2026-08-30".into());
        source.relations = vec![relation("motivates", "theorem:c")];

        let mut theorem = node("theorem:c", "Theorem");
        theorem.status = Some("proof-source-present".into());
        theorem.statement = Some("P -> P".into());
        theorem.verification = vec![Verification {
            method: "Lean4".into(),
            target: "formal/Gordian/Graph.lean#no_dependency_cycle".into(),
            status: "ci-check-required".into(),
            note: None,
        }];

        let mut foundation = node("foundation:d", "ScientificFoundation");
        foundation.status = Some("established".into());
        foundation.relations = vec![relation("groundedIn", "source:b")];

        graph(vec![claim, source, theorem, foundation])
    }

    /// A throwaway directory holding a synthetic Lean file, so the on-disk rules can be
    /// tested without depending on the repository's own formal sources.
    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new(lean_body: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("gordian-kg-audit-{}-{unique}", std::process::id()));
            fs::create_dir_all(root.join("formal/Gordian")).expect("create sandbox");
            fs::write(root.join("formal/Gordian/Graph.lean"), lean_body).expect("write lean");
            Self { root }
        }

        fn context(&self) -> AuditContext {
            AuditContext::for_repo(&self.root).expect("sandbox context")
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn rules_fired(findings: &[AuditFinding], rule: &str) -> usize {
        findings
            .iter()
            .filter(|finding| finding.rule == rule)
            .count()
    }

    #[test]
    fn validates_well_formed_graph() {
        fixture().validate().unwrap();
    }

    #[test]
    fn detects_dangling_relation() {
        let mut graph = fixture();
        graph.nodes[0].relations[0].target = "missing".into();
        assert!(graph.validate().is_err());
    }

    #[test]
    fn finds_directed_path() {
        assert_eq!(
            fixture().path("claim:a", "theorem:c").unwrap(),
            Some(vec![
                "claim:a".to_string(),
                "source:b".to_string(),
                "theorem:c".to_string()
            ])
        );
    }

    #[test]
    fn emits_dot_edges() {
        let dot = fixture().to_dot().unwrap();
        assert!(dot.contains("claim:a\" -> \"source:b"));
        assert!(dot.contains("supportedBy"));
    }

    #[test]
    fn audit_rule_registry_has_no_duplicates() {
        let unique: HashSet<&&str> = AUDIT_RULES.iter().collect();
        assert_eq!(unique.len(), AUDIT_RULES.len());
    }

    #[test]
    fn every_finding_carries_a_registered_rule() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        // Break one instance of every rule at once.
        graph
            .nodes
            .push(node("foundation:orphan", "EngineeringFoundation"));
        graph.nodes.push(node("claim:bare", "Claim"));
        graph.nodes.push(node("hypothesis:bare", "Hypothesis"));
        graph.nodes.push(node("theorem:bare", "Theorem"));
        graph.nodes.push(node("experiment:bare", "Experiment"));
        graph.nodes.push(node("source:bare", "Source"));
        let mut nosummary = node("concept:bare", "Concept");
        nosummary.summary = String::new();
        graph.nodes.push(nosummary);
        let findings = graph.audit(&sandbox.context());
        assert!(!findings.is_empty());
        for finding in &findings {
            assert!(
                AUDIT_RULES.contains(&finding.rule),
                "unregistered rule {}",
                finding.rule
            );
        }
    }

    #[test]
    fn conforming_fixture_is_clean() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let findings = fixture().audit(&sandbox.context());
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn foundation_without_a_source_edge_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[3].relations.clear();
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_FOUNDATION_UNSOURCED), 1);
        assert_eq!(findings[0].node_id, "foundation:d");
        assert_eq!(findings[0].severity, FindingSeverity::Error);
    }

    #[test]
    fn foundation_citing_a_non_source_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        // groundedIn a Claim rather than a Source: the edge exists, the provenance does not.
        graph.nodes[3].relations = vec![relation("groundedIn", "claim:a")];
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_FOUNDATION_UNSOURCED), 1);
    }

    #[test]
    fn source_without_a_locator_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[1].url = None;
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_SOURCE_LOCATOR), 1);

        graph.nodes[1].identifier = Some("10.1109/71.993206".into());
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_SOURCE_LOCATOR), 0);
    }

    #[test]
    fn evidence_predicate_cycles_are_errors() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut left = node("foundation:crdt", "ScientificFoundation");
        left.relations = vec![
            relation("groundedIn", "source:b"),
            relation("qualifiedBy", "claim:crdt-not-semantic-correctness"),
        ];
        let mut right = node("claim:crdt-not-semantic-correctness", "Claim");
        right.relations = vec![relation("supportedBy", "foundation:crdt")];
        let mut source = node("source:b", "Source");
        source.url = Some("https://example.test/b".into());

        let findings = graph(vec![left, right, source]).audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_EVIDENCE_CYCLE), 1);
        let cycle = findings
            .iter()
            .find(|finding| finding.rule == RULE_EVIDENCE_CYCLE)
            .expect("cycle finding");
        assert!(cycle.message.contains("foundation:crdt"));
        assert!(
            cycle
                .message
                .contains("claim:crdt-not-semantic-correctness")
        );
    }

    #[test]
    fn acyclic_evidence_graph_reports_no_cycle() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let findings = fixture().audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_EVIDENCE_CYCLE), 0);
    }

    #[test]
    fn status_outside_the_vocabulary_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[2].status = Some("made-up-status".into());
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_STATUS_VOCABULARY), 1);
    }

    #[test]
    fn missing_status_where_the_class_enumerates_one_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[2].status = None;
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_STATUS_VOCABULARY), 1);
    }

    #[test]
    fn verification_status_outside_the_vocabulary_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[2].verification[0].status = "green-ish".into();
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_STATUS_VOCABULARY), 1);
    }

    #[test]
    fn an_unenumerated_class_accepts_any_status() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[3].status = Some("anything-at-all".into());
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_STATUS_VOCABULARY), 0);
    }

    #[test]
    fn an_external_vocabulary_replaces_the_built_in_one() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        fs::create_dir_all(sandbox.root.join("knowledge")).unwrap();
        fs::write(
            sandbox.root.join(STATUS_VOCABULARY_PATH),
            r#"{"node_status": {"ScientificFoundation": ["established"]},
                "verification_status": ["ci-check-required"]}"#,
        )
        .unwrap();
        let context = sandbox.context();
        let mut graph = fixture();
        // Theorem is unconstrained under this vocabulary ...
        graph.nodes[2].status = Some("whatever".into());
        assert_eq!(
            rules_fired(&graph.audit(&context), RULE_STATUS_VOCABULARY),
            0
        );
        // ... and ScientificFoundation is now closed.
        graph.nodes[3].status = Some("whatever".into());
        assert_eq!(
            rules_fired(&graph.audit(&context), RULE_STATUS_VOCABULARY),
            1
        );
    }

    #[test]
    fn missing_lean_anchor_is_an_error() {
        let sandbox = Sandbox::new("theorem some_other_theorem : True := trivial\n");
        let findings = fixture().audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_VERIFICATION_TARGET), 1);
        let finding = findings
            .iter()
            .find(|finding| finding.rule == RULE_VERIFICATION_TARGET)
            .expect("anchor finding");
        assert!(finding.message.contains("no_dependency_cycle"));
    }

    #[test]
    fn missing_verification_path_is_an_error() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[2].verification[0].target = "formal/Gordian/Absent.lean#whatever".into();
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_VERIFICATION_TARGET), 1);
    }

    #[test]
    fn planned_verification_targets_are_not_resolved() {
        let sandbox = Sandbox::new("theorem no_dependency_cycle : True := trivial\n");
        let mut graph = fixture();
        graph.nodes[2].verification[0].target = "experiments/not-yet/protocol.json".into();
        graph.nodes[2].verification[0].status = "planned".into();
        let findings = graph.audit(&sandbox.context());
        assert_eq!(rules_fired(&findings, RULE_VERIFICATION_TARGET), 0);
    }

    #[test]
    fn a_detached_audit_skips_the_on_disk_rules() {
        let mut graph = fixture();
        graph.nodes[2].verification[0].target = "formal/Gordian/Absent.lean#whatever".into();
        let findings = graph.audit(&AuditContext::detached());
        assert_eq!(rules_fired(&findings, RULE_VERIFICATION_TARGET), 0);
    }

    #[test]
    fn lean_anchors_match_whole_declaration_names_only() {
        assert!(declares("theorem foo : True := trivial\n", "foo"));
        assert!(declares("def foo (n : Nat) : Nat := n\n", "foo"));
        assert!(!declares("theorem foobar : True := trivial\n", "foo"));
        assert!(!declares("-- theorem foo : True\n", "foo"));
        assert!(declares("structure Frontier where\n", "Frontier"));
    }

    #[test]
    fn shards_without_the_new_source_fields_still_parse() {
        let shard: KnowledgeGraph = serde_json::from_str(
            r#"{"@graph": [{"@id": "source:x", "@type": ["Source"], "name": "X",
                            "summary": "s", "url": "https://example.test/x"}]}"#,
        )
        .expect("legacy shard parses");
        assert_eq!(shard.nodes[0].identifier, None);
        assert_eq!(shard.nodes[0].retrieved_at, None);
    }
}
