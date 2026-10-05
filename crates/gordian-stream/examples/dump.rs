//! Print a stream's public observations as JSON lines.
//!
//! ```text
//! cargo run -p gordian-stream --example dump -- --seed 7
//! ```
//!
//! One line per observation: `{"id": n, "at": <nanoseconds>, "obs": {...}}`. Only the public
//! stream is printed; nothing here touches hidden state. The output is a pure function of the
//! seed, so two runs have the same sha256.

use gordian_stream::{StreamParams, generate};
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: dump [--seed N]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut seed = 0u64;
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
            _ => return usage(),
        }
    }
    let stream = generate(&StreamParams::new(seed));
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (i, (at, obs)) in stream.events().iter().enumerate() {
        let line = serde_json::json!({ "id": i, "at": at.0, "obs": obs });
        if writeln!(out, "{line}").is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
