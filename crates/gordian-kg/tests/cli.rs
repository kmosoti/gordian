use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gordian_kg::{
    AUDIT_RULES, KnowledgeGraph, RULE_CLAIM_UNSUPPORTED, RULE_EVIDENCE_CYCLE,
    RULE_EXPERIMENT_UNTARGETED, RULE_FOUNDATION_UNSOURCED, RULE_HYPOTHESIS_UNTESTED,
    RULE_MISSING_SUMMARY, RULE_SOURCE_LOCATOR, RULE_STATUS_VOCABULARY, RULE_THEOREM_INCOMPLETE,
    RULE_VERIFICATION_TARGET, Relation,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under repository/crates")
        .to_path_buf()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gordian-kg"))
        .args(args)
        .current_dir(repository_root())
        .output()
        .expect("run gordian-kg")
}

fn run_with_graph(graph: &Path, args: &[&str]) -> Output {
    let root = repository_root();
    Command::new(env!("CARGO_BIN_EXE_gordian-kg"))
        .arg("--graph")
        .arg(graph)
        .arg("--repo-root")
        .arg(&root)
        .args(args)
        .current_dir(root)
        .output()
        .expect("run gordian-kg against fixture graph")
}

fn has_type(node: &gordian_kg::Node, kind: &str) -> bool {
    node.types.iter().any(|candidate| candidate == kind)
}

fn make_rule_fail(graph: &mut KnowledgeGraph, rule: &str) {
    match rule {
        RULE_MISSING_SUMMARY => graph.nodes[0].summary.clear(),
        RULE_FOUNDATION_UNSOURCED => {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| {
                    node.types.iter().any(|kind| kind.ends_with("Foundation"))
                        && !has_type(node, "Source")
                })
                .expect("canonical graph has a non-Source foundation");
            node.relations.retain(|relation| {
                !matches!(
                    relation.predicate.as_str(),
                    "supportedBy" | "qualifiedBy" | "challengedBy" | "groundedIn"
                )
            });
        }
        RULE_SOURCE_LOCATOR => {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Source"))
                .expect("canonical graph has a Source");
            node.url = None;
            node.identifier = None;
        }
        RULE_EVIDENCE_CYCLE => {
            let (first, rest) = graph.nodes.split_at_mut(1);
            let left = &mut first[0];
            let right = &mut rest[0];
            let left_id = left.id.clone();
            let right_id = right.id.clone();
            left.relations.push(Relation {
                predicate: "supportedBy".into(),
                target: right_id,
                note: None,
            });
            right.relations.push(Relation {
                predicate: "supportedBy".into(),
                target: left_id,
                note: None,
            });
        }
        RULE_STATUS_VOCABULARY => {
            graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Theorem"))
                .expect("canonical graph has a Theorem")
                .status = Some("not-in-the-vocabulary".into());
        }
        RULE_VERIFICATION_TARGET => {
            let verification = graph
                .nodes
                .iter_mut()
                .flat_map(|node| &mut node.verification)
                .find(|verification| verification.status != "planned")
                .expect("canonical graph has a non-planned verification target");
            verification.target = "definitely/missing/verification-target".into();
        }
        RULE_HYPOTHESIS_UNTESTED => {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Hypothesis"))
                .expect("canonical graph has a Hypothesis");
            node.relations
                .retain(|relation| relation.predicate != "testedBy");
            node.verification.clear();
        }
        RULE_THEOREM_INCOMPLETE => {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Theorem"))
                .expect("canonical graph has a Theorem");
            node.statement = None;
            node.verification.clear();
        }
        RULE_EXPERIMENT_UNTARGETED => {
            graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Experiment"))
                .expect("canonical graph has an Experiment")
                .verification
                .clear();
        }
        RULE_CLAIM_UNSUPPORTED => {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| has_type(node, "Claim"))
                .expect("canonical graph has a Claim");
            node.relations.retain(|relation| {
                !matches!(
                    relation.predicate.as_str(),
                    "supportedBy"
                        | "qualifiedBy"
                        | "challengedBy"
                        | "groundedIn"
                        | "formalizedBy"
                        | "testedBy"
                )
            });
        }
        unknown => panic!("no CLI negative fixture for audit rule {unknown}"),
    }
}

fn assert_success(args: &[&str]) {
    let output = run(args);
    assert!(
        output.status.success(),
        "gordian-kg {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn every_subcommand_runs_against_the_canonical_graph() {
    let dot = std::env::temp_dir().join(format!("gordian-kg-cli-{}.dot", std::process::id()));
    let dot_text = dot.to_string_lossy().into_owned();
    let commands = [
        vec!["validate"],
        vec!["audit", "--strict"],
        vec!["stats"],
        vec!["list", "--kind", "Algorithm"],
        vec!["show", "concept:atom"],
        vec!["search", "critical path"],
        vec!["hypotheses"],
        vec!["theorems"],
        vec!["evidence", "claim:semantic-state-vs-code-state"],
        vec!["neighbors", "concept:atom"],
        vec![
            "path",
            "concept:atom",
            "theorem:dispatch-requires-dependencies",
        ],
        vec!["export-dot", "--out", dot_text.as_str()],
    ];

    for command in commands {
        assert_success(&command);
    }
    let rendered = fs::read_to_string(&dot).expect("export-dot wrote its output");
    assert!(rendered.starts_with("digraph gordian_knowledge"));
    let _ = fs::remove_file(dot);
}

#[test]
fn every_audit_rule_fails_through_the_strict_cli() {
    let root = repository_root();
    let canonical =
        KnowledgeGraph::load(root.join("knowledge/graph")).expect("load canonical graph");
    let fixture_root =
        std::env::temp_dir().join(format!("gordian-kg-cli-audit-rules-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fixture_root);
    fs::create_dir_all(&fixture_root).expect("create fixture directory");

    for rule in AUDIT_RULES {
        let mut graph = canonical.clone();
        make_rule_fail(&mut graph, rule);
        let path = fixture_root.join(format!("{rule}.jsonld"));
        fs::write(
            &path,
            serde_json::to_vec(&graph).expect("serialize failing graph"),
        )
        .expect("write failing graph");

        let output = run_with_graph(&path, &["audit", "--strict"]);
        assert!(
            !output.status.success(),
            "strict CLI unexpectedly accepted failing audit fixture {rule}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(rule),
            "strict CLI failure did not report {rule}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let _ = fs::remove_dir_all(fixture_root);
}

#[test]
fn readme_subcommand_block_and_clap_surface_are_equal() {
    let help = run(&["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("help is UTF-8");
    let clap_commands: BTreeSet<String> = help
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect();

    let readme = fs::read_to_string(repository_root().join("README.md")).expect("read README");
    let readme_commands: BTreeSet<String> = readme
        .lines()
        .filter_map(|line| line.strip_prefix("cargo run -p gordian-kg -- "))
        .filter_map(|args| args.split_whitespace().next())
        .map(str::to_owned)
        .collect();

    assert_eq!(readme_commands, clap_commands);
}
