//! Event statistics per tick length (W1; evaluator-side: the incident tables read hidden labels).
//!
//! ```text
//! cargo run -p gordian-stream --features reveal-hidden-state --example ticks -- \
//!     --seed-from 33000 --count 200 --out-dir experiments/exploration
//! ```
//!
//! For tick lengths 100 ms, 500 ms and 2 s, over `--count` streams at the default parameters
//! (regime schedule included), writes five CSV files into `--out-dir`:
//!
//! - `w1-ticks-hist.csv`: the histogram of events per tick (all, abnormal by the public rules)
//!   and per (node, tick) cell, empty ticks and cells included.
//! - `w1-ticks-summary.csv`: mean, variance, quantiles and the share of empty ticks and cells.
//! - `w1-ticks-lags.csv`: per hard family and mode, the ticks from the incident's first
//!   observation to its first abnormal observation and to the partner's first alarm (where the
//!   family has a partner), and from the first abnormal observation to the partner's alarm; and
//!   the same lags in milliseconds (`tick_ms` 0).
//! - `w1-ticks-decisive.csv`: per hard family and mode, the share of the incident's decisive
//!   evidence that falls in the same tick as its first observation.
//! - `w1-ticks-burst.csv`: per hard family and mode (the leak has no burst), the share of the
//!   incident's first-moments burst that falls in the first observation's tick, the share of
//!   incidents whose whole burst fits in one tick, and the number of ticks the burst spans.
//!
//! Streams here carry the hidden labels (tier, family, mode, partner, decisive evidence); nothing
//! in this example is a policy input. The output is a pure function of the arguments. The
//! example needs the hidden-state feature; without it the binary only says so.

#[cfg(feature = "reveal-hidden-state")]
mod stats;

#[cfg(not(feature = "reveal-hidden-state"))]
fn main() -> std::process::ExitCode {
    eprintln!("ticks needs --features reveal-hidden-state");
    std::process::ExitCode::from(2)
}

#[cfg(feature = "reveal-hidden-state")]
fn main() -> std::process::ExitCode {
    real::main()
}

#[cfg(feature = "reveal-hidden-state")]
mod real {
    use super::stats::{Group, Hist, MS, TICK_NS, Totals};
    use gordian_stream::{StreamParams, generate};
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    fn usage() -> ExitCode {
        eprintln!("usage: ticks [--seed-from N] [--count K] [--out-dir DIR]");
        ExitCode::from(2)
    }

    fn q(h: &Hist, p: f64) -> u64 {
        h.quantile(p)
    }

    fn write(dir: &Path, name: &str, body: &str) -> bool {
        match std::fs::write(dir.join(name), body) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("ticks: cannot write {name}: {e}");
                false
            }
        }
    }

    pub fn main() -> ExitCode {
        let mut seed_from = 33_000u64;
        let mut count = 200u64;
        let mut out_dir = PathBuf::from(".");
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            let Some(value) = args.next() else {
                return usage();
            };
            match flag.as_str() {
                "--seed-from" => match value.parse() {
                    Ok(v) => seed_from = v,
                    Err(_) => return usage(),
                },
                "--count" => match value.parse() {
                    Ok(v) => count = v,
                    Err(_) => return usage(),
                },
                "--out-dir" => out_dir = PathBuf::from(value),
                _ => return usage(),
            }
        }

        let mut totals = Totals::new(&TICK_NS);
        for seed in seed_from..seed_from.saturating_add(count) {
            totals.add_stream(&generate(&StreamParams::new(seed)), &TICK_NS);
        }

        let mut hist = String::from("tick_ms,measure,events,frequency\n");
        let mut summary = String::from(
            "tick_ms,measure,cells,mean,variance,dispersion,share_empty,p50,p90,p99,max\n",
        );
        for (k, &tick_ns) in TICK_NS.iter().enumerate() {
            let ms = tick_ns / MS;
            let h = &totals.hists[k];
            for (name, hh) in [
                ("tick_all", &h.all),
                ("tick_abnormal", &h.abnormal),
                ("node_tick_all", &h.node_all),
                ("node_tick_abnormal", &h.node_abnormal),
            ] {
                for (v, n) in &hh.0 {
                    writeln!(hist, "{ms},{name},{v},{n}").unwrap();
                }
                writeln!(
                    summary,
                    "{ms},{name},{},{:.6},{:.6},{:.4},{:.6},{},{},{},{}",
                    hh.n(),
                    hh.mean(),
                    hh.variance(),
                    hh.variance() / hh.mean().max(f64::MIN_POSITIVE),
                    hh.share_at_most(0),
                    q(hh, 0.5),
                    q(hh, 0.9),
                    q(hh, 0.99),
                    hh.max()
                )
                .unwrap();
            }
        }

        let mut lags = String::from(
            "family,mode,tick_ms,unit,measure,incidents,n,missing,mean,p50,p90,max,share_zero,share_at_most_1\n",
        );
        let mut decisive = String::from(
            "family,mode,tick_ms,incidents,with_decisive,mean_share_in_first_tick,share_any_in_first_tick,share_all_in_first_tick\n",
        );
        let mut burst = String::from(
            "family,mode,tick_ms,incidents,mean_share_in_first_tick,share_in_one_tick,mean_span_ticks,max_span_ticks\n",
        );
        let groups: Vec<Group> = totals.lags[0].keys().copied().collect();
        for g in &groups {
            let fam = format!("{:?}", g.family);
            let mode = g.mode_name();
            let has_partner =
                totals.lags[0][g].to_partner.n() + totals.lags[0][g].no_partner_alarm > 0;
            let row = |out: &mut String,
                       tick_ms: u64,
                       unit: &str,
                       measure: &str,
                       inc: u64,
                       h: &Hist,
                       missing: u64| {
                writeln!(
                    out,
                    "{fam},{mode},{tick_ms},{unit},{measure},{inc},{},{missing},{:.4},{},{},{},{:.4},{:.4}",
                    h.n(),
                    h.mean(),
                    q(h, 0.5),
                    q(h, 0.9),
                    h.max(),
                    h.share_at_most(0),
                    h.share_at_most(1)
                )
                .unwrap();
            };
            for (k, &tick_ns) in TICK_NS.iter().enumerate() {
                let s = &totals.lags[k][g];
                let ms = tick_ns / MS;
                row(
                    &mut lags,
                    ms,
                    "ticks",
                    "first_obs_to_first_abnormal",
                    s.incidents,
                    &s.to_abnormal,
                    s.no_abnormal,
                );
                if has_partner {
                    row(
                        &mut lags,
                        ms,
                        "ticks",
                        "first_obs_to_partner_alarm",
                        s.incidents,
                        &s.to_partner,
                        s.no_partner_alarm,
                    );
                    row(
                        &mut lags,
                        ms,
                        "ticks",
                        "first_abnormal_to_partner_alarm",
                        s.incidents,
                        &s.abnormal_to_partner,
                        s.no_partner_alarm,
                    );
                }
                row(
                    &mut lags,
                    ms,
                    "ticks",
                    "first_obs_to_first_decisive",
                    s.incidents,
                    &s.to_decisive,
                    s.no_decisive,
                );
                if s.with_burst > 0 {
                    writeln!(
                        burst,
                        "{fam},{mode},{ms},{},{:.4},{:.4},{:.4},{}",
                        s.with_burst,
                        s.burst_share_sum / s.with_burst as f64,
                        s.burst_one_tick as f64 / s.with_burst as f64,
                        s.burst_span.mean(),
                        s.burst_span.max()
                    )
                    .unwrap();
                }
                writeln!(
                    decisive,
                    "{fam},{mode},{ms},{},{},{:.4},{:.4},{:.4}",
                    s.incidents,
                    s.with_decisive,
                    s.decisive_share_sum / s.with_decisive.max(1) as f64,
                    s.decisive_any as f64 / s.with_decisive.max(1) as f64,
                    s.decisive_all as f64 / s.with_decisive.max(1) as f64
                )
                .unwrap();
            }
            let s0 = &totals.lags[0][g];
            let m = &totals.ms[g];
            row(
                &mut lags,
                0,
                "ms",
                "first_obs_to_first_abnormal",
                s0.incidents,
                &m.to_abnormal,
                s0.no_abnormal,
            );
            row(
                &mut lags,
                0,
                "ms",
                "first_obs_to_first_decisive",
                s0.incidents,
                &m.to_decisive,
                s0.no_decisive,
            );
            if has_partner {
                row(
                    &mut lags,
                    0,
                    "ms",
                    "first_obs_to_partner_alarm",
                    s0.incidents,
                    &m.to_partner,
                    s0.no_partner_alarm,
                );
                row(
                    &mut lags,
                    0,
                    "ms",
                    "first_abnormal_to_partner_alarm",
                    s0.incidents,
                    &m.abnormal_to_partner,
                    s0.no_partner_alarm,
                );
            }
        }

        let ok = std::fs::create_dir_all(&out_dir).is_ok()
            && write(&out_dir, "w1-ticks-hist.csv", &hist)
            && write(&out_dir, "w1-ticks-summary.csv", &summary)
            && write(&out_dir, "w1-ticks-lags.csv", &lags)
            && write(&out_dir, "w1-ticks-decisive.csv", &decisive)
            && write(&out_dir, "w1-ticks-burst.csv", &burst);
        eprintln!(
            "ticks: {} streams from seed {seed_from}, {} events ({} abnormal), {} s of stream",
            totals.streams,
            totals.events,
            totals.abnormal_events,
            totals.duration_ns / 1_000_000_000
        );
        if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}
