//! Print the generator's hidden truth per (class, seed) as a TSV, for offline analysis.
//!
//! ```text
//! cargo run --locked -p gordian-eval --example truth_table -- --seed-start 1000 --seed-count 500
//! cargo run --locked -p gordian-eval --example truth_table -- --seed-start 1000 --seed-count 500 \
//!     --classes Ambiguous,NoFault
//! ```
//!
//! Options (all optional): `--seed-start N` (default 1000), `--seed-count N` (default 500),
//! `--classes A,B,...` (default every class, in `EpisodeClass::ALL` order). Rows are ordered by
//! class, then by seed. The output is a pure function of the arguments, so two runs have the same
//! sha256.
//!
//! This reads hidden simulator state through the privileged accessor, which is why it lives in
//! `gordian-eval` and not anywhere a policy can reach. The output is for analysis only (B4's
//! effective-ambiguity table); it must never be an input to a policy, and it is not committed.
//!
//! Columns, matching what `experiments/exploration/scripts/b4.py` reads:
//!
//! - `class`, `seed`: the episode (default `episode_params`, so noise rate 3);
//! - `truth_kind`: the kind of the first true fault (`None` when there is none);
//! - `critical`: whether that fault is critical (`false` when there is none);
//! - `n_hyp`: how many hypotheses the full public stream leaves consistent;
//! - `kinds_in_set`: the distinct fault kinds among them, sorted by name and joined with `+`
//!   (empty when only "no fault" is consistent);
//! - `contains_nofault`: whether "no fault" is among the consistent hypotheses.

use gordian_world::oracle::reveal;
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{EpisodeClass, EpisodeSpec, generate};
use std::collections::BTreeSet;
use std::io::{BufWriter, Write};
use std::process::ExitCode;

const HEADER: &str = "class\tseed\ttruth_kind\tcritical\tn_hyp\tkinds_in_set\tcontains_nofault";

fn usage() -> ExitCode {
    eprintln!("usage: truth_table [--seed-start N] [--seed-count N] [--classes A,B,...]");
    let names: Vec<String> = EpisodeClass::ALL.iter().map(|c| format!("{c:?}")).collect();
    eprintln!("classes: {}", names.join(" "));
    ExitCode::from(2)
}

fn parse_classes(list: &str) -> Option<Vec<EpisodeClass>> {
    list.split(',')
        .map(|name| {
            EpisodeClass::ALL
                .iter()
                .copied()
                .find(|c| format!("{c:?}") == name)
        })
        .collect()
}

fn main() -> ExitCode {
    let mut seed_start = 1000u64;
    let mut seed_count = 500u64;
    let mut classes: Vec<EpisodeClass> = EpisodeClass::ALL.to_vec();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--seed-start" => match value.parse() {
                Ok(v) => seed_start = v,
                Err(_) => return usage(),
            },
            "--seed-count" => match value.parse() {
                Ok(v) => seed_count = v,
                Err(_) => return usage(),
            },
            "--classes" => match parse_classes(&value) {
                Some(v) => classes = v,
                None => return usage(),
            },
            _ => return usage(),
        }
    }
    let Some(seed_end) = seed_start.checked_add(seed_count) else {
        return usage();
    };

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut rows = || -> std::io::Result<()> {
        writeln!(out, "{HEADER}")?;
        for class in &classes {
            for seed in seed_start..seed_end {
                let episode = generate(&EpisodeSpec::new(seed, *class));
                let hidden = reveal(&episode);
                let first = hidden.faults.first();
                let truth = first.map_or("None".to_string(), |f| format!("{:?}", f.kind));
                let critical = first.is_some_and(|f| f.critical);
                let set = consistent_hypotheses(&episode.public_info(), episode.stream());
                let mut kinds: BTreeSet<String> = BTreeSet::new();
                let mut contains_nofault = false;
                for hypothesis in &set {
                    match hypothesis {
                        Some((kind, _site)) => {
                            kinds.insert(format!("{kind:?}"));
                        }
                        None => contains_nofault = true,
                    }
                }
                let kinds: Vec<&str> = kinds.iter().map(String::as_str).collect();
                writeln!(
                    out,
                    "{class:?}\t{seed}\t{truth}\t{critical}\t{}\t{}\t{contains_nofault}",
                    set.len(),
                    kinds.join("+"),
                )?;
            }
        }
        out.flush()
    };
    match rows() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("truth_table: {err}");
            ExitCode::FAILURE
        }
    }
}
