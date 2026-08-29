use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

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

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn nodes_of_type(&self, kind: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.types.iter().any(|ty| ty.eq_ignore_ascii_case(kind)))
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

    pub fn neighbors(&self, id: &str, predicate: Option<&str>) -> Vec<Neighbor> {
        let mut neighbors = Vec::new();

        if let Some(node) = self.node(id) {
            for relation in &node.relations {
                if predicate.is_none_or(|p| relation.predicate == p) {
                    neighbors.push(Neighbor {
                        direction: Direction::Outgoing,
                        predicate: relation.predicate.clone(),
                        node_id: relation.target.clone(),
                    });
                }
            }
        }

        for node in &self.nodes {
            for relation in &node.relations {
                if relation.target == id && predicate.is_none_or(|p| relation.predicate == p) {
                    neighbors.push(Neighbor {
                        direction: Direction::Incoming,
                        predicate: relation.predicate.clone(),
                        node_id: node.id.clone(),
                    });
                }
            }
        }

        neighbors
    }

    pub fn path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        if self.node(from).is_none() || self.node(to).is_none() {
            return None;
        }
        if from == to {
            return Some(vec![from.to_owned()]);
        }

        let index: HashMap<&str, &Node> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut queue = VecDeque::from([from.to_owned()]);
        let mut visited = HashSet::from([from.to_owned()]);
        let mut previous: HashMap<String, String> = HashMap::new();

        while let Some(current) = queue.pop_front() {
            let Some(node) = index.get(current.as_str()) else {
                continue;
            };

            for relation in &node.relations {
                if visited.insert(relation.target.clone()) {
                    previous.insert(relation.target.clone(), current.clone());
                    if relation.target == to {
                        let mut path = vec![to.to_owned()];
                        let mut cursor = to.to_owned();
                        while let Some(parent) = previous.get(&cursor) {
                            path.push(parent.clone());
                            if parent == from {
                                break;
                            }
                            cursor = parent.clone();
                        }
                        path.reverse();
                        return Some(path);
                    }
                    queue.push_back(relation.target.clone());
                }
            }
        }

        None
    }

    pub fn evidence_for(&self, claim_id: &str) -> Vec<Neighbor> {
        self.neighbors(claim_id, None)
            .into_iter()
            .filter(|neighbor| {
                matches!(
                    neighbor.predicate.as_str(),
                    "supportedBy" | "challengedBy" | "groundedIn" | "formalizedBy" | "testedBy"
                )
            })
            .collect()
    }
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
                    statement: None,
                    url: None,
                    relations: vec![Relation {
                        predicate: "supportedBy".into(),
                        target: "source:b".into(),
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
                    url: None,
                    relations: vec![Relation {
                        predicate: "motivates".into(),
                        target: "theorem:c".into(),
                    }],
                    verification: vec![],
                },
                Node {
                    id: "theorem:c".into(),
                    types: vec!["Theorem".into()],
                    name: "C".into(),
                    summary: "gamma".into(),
                    status: None,
                    statement: None,
                    url: None,
                    relations: vec![],
                    verification: vec![],
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
            graph.path("claim:a", "theorem:c"),
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
        let neighbors = graph.neighbors("source:b", None);
        assert!(neighbors.iter().any(|n| {
            n.direction == Direction::Incoming
                && n.predicate == "supportedBy"
                && n.node_id == "claim:a"
        }));
        assert!(neighbors.iter().any(|n| {
            n.direction == Direction::Outgoing
                && n.predicate == "motivates"
                && n.node_id == "theorem:c"
        }));
    }
}
