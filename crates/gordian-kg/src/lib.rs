use anyhow::{Context, Result, bail};
use petgraph::Direction as PetDirection;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
    pub severity: FindingSeverity,
    pub node_id: String,
    pub message: String,
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
            bail!("knowledge graph directory {} has no .jsonld shards", path.display());
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

    pub fn audit(&self) -> Vec<AuditFinding> {
        let mut findings = Vec::new();
        for node in &self.nodes {
            if node.summary.trim().is_empty() {
                findings.push(finding(FindingSeverity::Warning, node, "missing summary"));
            }
            if node.has_type("Source") && node.url.as_deref().is_none_or(str::is_empty) {
                findings.push(finding(
                    FindingSeverity::Error,
                    node,
                    "Source node has no URL/provenance locator",
                ));
            }
            if node.has_type("Claim") && !node.has_any_relation(EVIDENCE_PREDICATES) {
                findings.push(finding(
                    FindingSeverity::Warning,
                    node,
                    "Claim has no evidence, qualification, formalization, or test relation",
                ));
            }
            if node.has_type("Hypothesis")
                && !node.has_relation("testedBy")
                && node.verification.is_empty()
            {
                findings.push(finding(
                    FindingSeverity::Error,
                    node,
                    "Hypothesis has no experiment/test target",
                ));
            }
            if node.has_type("Theorem") {
                if node.statement.as_deref().is_none_or(str::is_empty) {
                    findings.push(finding(
                        FindingSeverity::Error,
                        node,
                        "Theorem has no formal statement",
                    ));
                }
                if node.verification.is_empty() {
                    findings.push(finding(
                        FindingSeverity::Error,
                        node,
                        "Theorem has no checker target",
                    ));
                }
            }
            if node.has_type("Experiment") && node.verification.is_empty() {
                findings.push(finding(
                    FindingSeverity::Warning,
                    node,
                    "Experiment has no executable/analysis target",
                ));
            }
        }
        findings
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn nodes_of_type(&self, kind: &str) -> Vec<&Node> {
        self.nodes.iter().filter(|node| node.has_type(kind)).collect()
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

fn finding(severity: FindingSeverity, node: &Node, message: &str) -> AuditFinding {
    AuditFinding {
        severity,
        node_id: node.id.clone(),
        message: message.to_owned(),
    }
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> KnowledgeGraph {
        KnowledgeGraph {
            context: serde_json::Value::Null,
            nodes: vec![
                Node {
                    id: "claim:a".into(),
                    types: vec!["Claim".into()],
                    name: "A".into(),
                    summary: "alpha".into(),
                    status: None,
                    statement: Some("A is supported".into()),
                    url: None,
                    relations: vec![Relation {
                        predicate: "supportedBy".into(),
                        target: "source:b".into(),
                        note: None,
                    }],
                    verification: vec![],
                },
                Node {
                    id: "source:b".into(),
                    types: vec!["Source".into()],
                    name: "B".into(),
                    summary: "beta".into(),
                    status: None,
                    statement: None,
                    url: Some("https://example.test/b".into()),
                    relations: vec![Relation {
                        predicate: "motivates".into(),
                        target: "theorem:c".into(),
                        note: None,
                    }],
                    verification: vec![],
                },
                Node {
                    id: "theorem:c".into(),
                    types: vec!["Theorem".into()],
                    name: "C".into(),
                    summary: "gamma".into(),
                    status: None,
                    statement: Some("P -> P".into()),
                    url: None,
                    relations: vec![],
                    verification: vec![Verification {
                        method: "Lean4".into(),
                        target: "formal/Gordian/Test.lean".into(),
                        status: "source-present".into(),
                        note: None,
                    }],
                },
            ],
        }
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
    fn audits_epistemic_requirements() {
        assert!(fixture().audit().is_empty());
    }

    #[test]
    fn emits_dot_edges() {
        let dot = fixture().to_dot().unwrap();
        assert!(dot.contains("claim:a\" -> \"source:b"));
        assert!(dot.contains("supportedBy"));
    }
}
