//! Print R8's question records as JSON lines (evaluator-side: truth is included).
//!
//! ```text
//! cargo run -p gordian-stream --features reveal-hidden-state --example questions -- \
//!     --seed-from 30000 --count 100 --tier hard
//! ```
//!
//! One line per question (see `gordian_stream::questions::QuestionRecord`): the focus
//! observation, the incident's decisive evidence, its truth and the pool of candidate
//! distractors within 40 s of the focus. Streams are generated with the default parameters
//! except that the regime schedule is empty unless `--regimes on` is given (R8 reading: an
//! unannounced change of the public physics would make some questions unanswerable from the
//! stated rules, which is a different property from distractor sensitivity). The output is a
//! pure function of the arguments, so two runs have the same sha256.
//!
//! Flags: `--seed-from N` (default 30000), `--count K` streams (default 1), `--tier
//! plain|hard|both` (default hard), `--regimes on|off` (default off), `--pool others|own`
//! (default others).
//!
//! `--pool others` (R8's pool, the default) holds the background and other incidents.
//! `--pool own` (R9's pool) also holds the focus incident's own non-decisive observations, the
//! ones the simulated reasoner counts in `m`, and adds a `pool_roles` field to each record.

use gordian_stream::questions::{POOL_WINDOW_NS, Pool, questions_with_pool};
use gordian_stream::{StreamParams, generate};
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: questions [--seed-from N] [--count K] [--tier plain|hard|both] [--regimes on|off] \
         [--pool others|own]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut seed_from = 30_000u64;
    let mut count = 1u64;
    let (mut plain, mut hard) = (false, true);
    let mut regimes = false;
    let mut pool = Pool::Others;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match (flag.as_str(), value.as_str()) {
            ("--seed-from", v) => match v.parse() {
                Ok(v) => seed_from = v,
                Err(_) => return usage(),
            },
            ("--count", v) => match v.parse() {
                Ok(v) => count = v,
                Err(_) => return usage(),
            },
            ("--tier", "plain") => (plain, hard) = (true, false),
            ("--tier", "hard") => (plain, hard) = (false, true),
            ("--tier", "both") => (plain, hard) = (true, true),
            ("--regimes", "on") => regimes = true,
            ("--regimes", "off") => regimes = false,
            ("--pool", "others") => pool = Pool::Others,
            ("--pool", "own") => pool = Pool::IncludingOwn,
            _ => return usage(),
        }
    }
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for seed in seed_from..seed_from.saturating_add(count) {
        let mut params = StreamParams::new(seed);
        if !regimes {
            params.regimes.clear();
        }
        let stream = generate(&params);
        for q in questions_with_pool(&stream, POOL_WINDOW_NS, plain, hard, pool) {
            let line = serde_json::to_string(&q).expect("a question record serializes");
            if writeln!(out, "{line}").is_err() {
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
