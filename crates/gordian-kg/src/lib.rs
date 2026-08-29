use anyhow::{Context, Result, bail};
use petgraph::Direction as PetDirection;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

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
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read knowledge graph {}", path.display()))?;
        let graph = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse knowledge graph {}", path.display()))?;
        Ok(graph)
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
                findings.push(AuditFinding {
                    severity: FindingSeverity::Warning,
                    node_id: node.id.clone(),
                    message: "missing summary".to_owned(),
                });
            }

            if node.has_type("Source") && node.url.as_deref().is_none_or(str::is_empty) {
                findings.push(AuditFinding {
                    severity: FindingSeverity::Error,
                    node_id: node.id.clone(),
                    message: "Source node has no URL/provenance locator".to_owned(),
                });
            }

            if node.has_type("Claim") && !node.has_any_relation(EVIDENCE_PREDICATES) {
                findings.push(AuditFinding {
                    severity: FindingSeverity::Warning,
                    node_id: node.id.clone(),
                    message: "Claim has no evidence, qualification, formalization, or test relation"
                        .to_owned(),
                });
            }

            if node.has_type("Hypothesis")
                && !node.has_relation("testedBy")
                && node.verification.is_empty()
            {
                findings.push(AuditFinding {
                    severity: FindingSeverity::Error,
                    node_id: node.id.clone(),
                    message: "Hypothesis has no experiment/test target".to_owned(),
                });
            }

            if node.has_type("Theorem") {
                if node.statement.as_deref().is_none_or(str::is_empty) {
                    findings.push(AuditFinding {
                        severity: FindingSeverity::Error,
                        node_id: node.id.clone(),
                        message: "Theorem has no formal statement".to_owned(),
                    });
                }
                if node.verification.is_empty() {
                    findings.push(AuditFinding {
                        severity: FindingSeverity::Error,
                        node_id: node.id.clone(),
                        message: "Theorem has no checker target".to_owned(),
                    });
                }
            }

            if node.has_type("Experiment") && node.verification.is_empty() {
                findings.push(AuditFinding {
                    severity: FindingSeverity::Warning,
                    node_id: node.id.clone(),
                    message: "Experiment has no executable/analysis target".to_owned(),
                });
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
            let index = graph.add_node(node.id.clone());
            by_id.insert(node.id.clone(), index);
        }

        for node in &self.nodes {
            let source = by_id[&node.id];
            for relation in &node.relations {
                let target = by_id[&relation.target];
                graph.add_edge(source, target, relation.predicate.clone());
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

    pub fn evidence_for(&self, claim_id: &str) -> Result<Vec<Neighbor>> {
        Ok(self
            .neighbors(claim_id, None)?
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
        let mut previous: HashMap<NodeIndex, NodeIndex> = HashMap::new();

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
                        let parent = previous[&cursor];
                        path.push(parent);
                        cursor = parent;
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
        let graph = fixture();
        assert_eq!(
            graph.path("claim:a", "theorem:c").unwrap(),
            Some(vec![
                "claim:a".to_string(),
                "source:b".to_string(),
                "theorem:c".to_string()
            ])
        );
    }

    #[test]
    fn finds_incoming_and_outgoing_neighbors() {
        let graph = fixture();
        let neighbors = graph.neighbors("source:b", None).unwrap();
        assert!(neighbors.iter().any(|neighbor| {
            neighbor.direction == Direction::Incoming
                && neighbor.predicate == "supportedBy"
                && neighbor.node_id == "claim:a"
        }));
        assert!(neighbors.iter().any(|neighbor| {
            neighbor.direction == Direction::Outgoing
                && neighbor.predicate == "motivates"
                && neighbor.node_id == "theorem:c"
        }));
    }

    #[test]
    fn audits_epistemic_requirements() {
        let graph = fixture();
        assert!(graph.audit().is_empty());
    }

    #[test]
    fn emits_dot_edges() {
        let dot = fixture().to_dot().unwrap();
        assert!(dot.contains("claim:a\" -> \"source:b"));
        assert!(dot.contains("supportedBy"));
    }
}
