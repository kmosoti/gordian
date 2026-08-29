use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use gordian_kg::{Direction, KnowledgeGraph, Node};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "gordian-kg")]
#[command(about = "Traverse and validate the Gordian research knowledge graph")]
struct Cli {
    #[arg(long, default_value = "knowledge/graph.jsonld")]
    graph: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate node identities and graph references.
    Validate,
    /// List nodes, optionally filtered by @type.
    List {
        #[arg(long)]
        kind: Option<String>,
    },
    /// Show one node as JSON.
    Show { id: String },
    /// Search IDs, names, summaries, and theorem/claim statements.
    Search { query: String },
    /// Show incoming and outgoing relations for one node.
    Neighbors {
        id: String,
        #[arg(long)]
        predicate: Option<String>,
    },
    /// Find a directed relation path between two nodes.
    Path { from: String, to: String },
    /// Show research/formal evidence attached to a claim.
    Evidence { claim: String },
    /// List formal theorem nodes and their verification targets.
    Theorems,
}

fn print_node(node: &Node) {
    let kinds = if node.types.is_empty() {
        "-".to_owned()
    } else {
        node.types.join(",")
    };
    println!("{:<34} {:<22} {}", node.id, kinds, node.name);
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let graph = KnowledgeGraph::load(&cli.graph)?;

    match cli.command {
        Command::Validate => {
            graph.validate()?;
            println!("valid: {} nodes", graph.nodes.len());
        }
        Command::List { kind } => {
            if let Some(kind) = kind {
                for node in graph.nodes_of_type(&kind) {
                    print_node(node);
                }
            } else {
                for node in &graph.nodes {
                    print_node(node);
                }
            }
        }
        Command::Show { id } => {
            let Some(node) = graph.node(&id) else {
                bail!("unknown node: {id}");
            };
            println!("{}", serde_json::to_string_pretty(node)?);
        }
        Command::Search { query } => {
            for node in graph.search(&query) {
                print_node(node);
            }
        }
        Command::Neighbors { id, predicate } => {
            if graph.node(&id).is_none() {
                bail!("unknown node: {id}");
            }
            for neighbor in graph.neighbors(&id, predicate.as_deref()) {
                let arrow = match neighbor.direction {
                    Direction::Incoming => "<-",
                    Direction::Outgoing => "->",
                };
                println!("{arrow} {:<20} {}", neighbor.predicate, neighbor.node_id);
            }
        }
        Command::Path { from, to } => {
            let Some(path) = graph.path(&from, &to) else {
                bail!("no directed path from {from} to {to}");
            };
            println!("{}", path.join(" -> "));
        }
        Command::Evidence { claim } => {
            if graph.node(&claim).is_none() {
                bail!("unknown claim: {claim}");
            }
            let evidence = graph.evidence_for(&claim);
            if evidence.is_empty() {
                println!("no evidence relations recorded for {claim}");
            } else {
                for neighbor in evidence {
                    let arrow = match neighbor.direction {
                        Direction::Incoming => "<-",
                        Direction::Outgoing => "->",
                    };
                    println!("{arrow} {:<20} {}", neighbor.predicate, neighbor.node_id);
                }
            }
        }
        Command::Theorems => {
            for theorem in graph.nodes_of_type("Theorem") {
                print_node(theorem);
                for verification in &theorem.verification {
                    println!(
                        "  [{}] {} -> {}",
                        verification.status, verification.method, verification.target
                    );
                }
            }
        }
    }

    Ok(())
}
