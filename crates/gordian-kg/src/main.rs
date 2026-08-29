use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use gordian_kg::{Direction, FindingSeverity, KnowledgeGraph, Node};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "gordian-kg")]
#[command(about = "Traverse, validate, and audit the Gordian research knowledge graph")]
struct Cli {
    #[arg(long, default_value = "knowledge/graph.jsonld")]
    graph: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate graph structure: identities, types, and relation targets.
    Validate,
    /// Audit epistemic completeness such as untested hypotheses or unlocated sources.
    Audit {
        /// Return an error when the audit contains any warning as well as errors.
        #[arg(long)]
        strict: bool,
    },
    /// Show node/edge/type/predicate counts.
    Stats,
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
    /// Find the shortest directed relation path by edge count.
    Path { from: String, to: String },
    /// Show evidence-oriented relations attached to a claim/hypothesis/concept.
    Evidence { id: String },
    /// List formal theorem nodes and their checker targets.
    Theorems,
    /// List hypotheses and their experiment/check targets.
    Hypotheses,
    /// Export the complete graph as Graphviz DOT.
    ExportDot {
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn print_node(node: &Node) {
    let kinds = if node.types.is_empty() {
        "-".to_owned()
    } else {
        node.types.join(",")
    };
    println!("{:<40} {:<28} {}", node.id, kinds, node.name);
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let graph = KnowledgeGraph::load(&cli.graph)?;

    match cli.command {
        Command::Validate => {
            graph.validate()?;
            let stats = graph.stats();
            println!("valid: {} nodes, {} edges", stats.nodes, stats.edges);
        }
        Command::Audit { strict } => {
            graph.validate()?;
            let findings = graph.audit();
            let mut errors = 0_usize;
            let mut warnings = 0_usize;

            for finding in &findings {
                match finding.severity {
                    FindingSeverity::Error => {
                        errors += 1;
                        println!("ERROR   {:<40} {}", finding.node_id, finding.message);
                    }
                    FindingSeverity::Warning => {
                        warnings += 1;
                        println!("WARNING {:<40} {}", finding.node_id, finding.message);
                    }
                }
            }

            println!("audit: {errors} errors, {warnings} warnings");
            if errors > 0 || (strict && warnings > 0) {
                bail!("knowledge graph audit failed");
            }
        }
        Command::Stats => {
            graph.validate()?;
            let stats = graph.stats();
            println!("nodes: {}", stats.nodes);
            println!("edges: {}", stats.edges);
            println!("\nnode types:");
            for (kind, count) in stats.node_types {
                println!("  {kind:<32} {count}");
            }
            println!("\npredicates:");
            for (predicate, count) in stats.predicates {
                println!("  {predicate:<32} {count}");
            }
        }
        Command::List { kind } => {
            graph.validate()?;
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
            graph.validate()?;
            let Some(node) = graph.node(&id) else {
                bail!("unknown node: {id}");
            };
            println!("{}", serde_json::to_string_pretty(node)?);
        }
        Command::Search { query } => {
            graph.validate()?;
            for node in graph.search(&query) {
                print_node(node);
            }
        }
        Command::Neighbors { id, predicate } => {
            graph.validate()?;
            for neighbor in graph.neighbors(&id, predicate.as_deref())? {
                let arrow = match neighbor.direction {
                    Direction::Incoming => "<-",
                    Direction::Outgoing => "->",
                };
                println!("{arrow} {:<24} {}", neighbor.predicate, neighbor.node_id);
            }
        }
        Command::Path { from, to } => {
            graph.validate()?;
            let Some(path) = graph.path(&from, &to)? else {
                bail!("no directed path from {from} to {to}");
            };
            println!("{}", path.join(" -> "));
        }
        Command::Evidence { id } => {
            graph.validate()?;
            if graph.node(&id).is_none() {
                bail!("unknown node: {id}");
            }
            let evidence = graph.evidence_for(&id)?;
            if evidence.is_empty() {
                println!("no evidence-oriented relations recorded for {id}");
            } else {
                for neighbor in evidence {
                    let arrow = match neighbor.direction {
                        Direction::Incoming => "<-",
                        Direction::Outgoing => "->",
                    };
                    println!("{arrow} {:<24} {}", neighbor.predicate, neighbor.node_id);
                }
            }
        }
        Command::Theorems => {
            graph.validate()?;
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
        Command::Hypotheses => {
            graph.validate()?;
            for hypothesis in graph.nodes_of_type("Hypothesis") {
                print_node(hypothesis);
                for verification in &hypothesis.verification {
                    println!(
                        "  [{}] {} -> {}",
                        verification.status, verification.method, verification.target
                    );
                }
            }
        }
        Command::ExportDot { out } => {
            graph.validate()?;
            let dot = graph.to_dot()?;
            if let Some(out) = out {
                fs::write(&out, dot)?;
                println!("wrote {}", out.display());
            } else {
                print!("{dot}");
            }
        }
    }

    Ok(())
}
