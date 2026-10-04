//! Print an episode's public observation stream as JSON lines.
//!
//! ```text
//! cargo run -p gordian-world --example dump -- --seed 7 --class Ambiguous
//! ```
//!
//! One line per observation: `{"at": <nanoseconds>, "obs": {...}}`. Only the public stream is
//! printed; nothing here touches hidden state. The output is a pure function of the arguments, so
//! two runs have the same sha256.

use gordian_world::{EpisodeClass, EpisodeSpec, generate};
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: dump [--seed N] [--class CLASS]");
    eprintln!("classes: {}", class_names().join(" "));
    ExitCode::from(2)
}

fn class_names() -> Vec<String> {
    EpisodeClass::ALL.iter().map(|c| format!("{c:?}")).collect()
}

fn main() -> ExitCode {
    let mut seed = 0u64;
    let mut class = EpisodeClass::Ambiguous;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--seed" => match value.parse() {
                Ok(v) => seed = v,
                Err(_) => return usage(),
            },
            "--class" => match serde_json::from_str::<EpisodeClass>(&format!("\"{value}\"")) {
                Ok(c) => class = c,
                Err(_) => return usage(),
            },
            _ => return usage(),
        }
    }
    let episode = generate(&EpisodeSpec::new(seed, class));
    let mut out = std::io::stdout().lock();
    for (at, obs) in episode.stream() {
        let line = serde_json::json!({ "at": at.0, "obs": obs });
        if writeln!(out, "{line}").is_err() {
            break; // closed pipe, e.g. `| head`
        }
    }
    ExitCode::SUCCESS
}
