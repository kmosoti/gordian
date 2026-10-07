//! The engram layer's counters, through the medium's trace port (work item A1d, Lab 1).
//!
//! Written and committed before any run of an A1d arm (`DESIGN.md` of `gordian-medium`, "The
//! engram under a non-privileged selector (A1d)", "Counters, through the trace port", committed
//! before this code).
//!
//! The layer holds a [`MarkLog`] as its engram medium's trace port: it samples no tick, so the
//! medium does what it did under `NoTrace`, and it keeps the marks the layer writes on it, each a
//! [`Mark`] whose kind is a [`TraceKind`]. With the layer's `trace` switch on and the environment
//! variable [`TRACE_DIR_ENV`] naming a directory, the marks are appended, when the layer is
//! dropped at the end of its segment, to `<dir>/engram-trace-<state_key>.csv` ([`FILE_HEADER`]).
//! This is output only: nothing is read back and nothing a decision depends on is written. A
//! failure to write is counted ([`write_errors`]) and never stops the arm.
//!
//! The marks' meaning, by kind (subject: the noticer's anomaly id; event: an observation id, the
//! answer's focus or the recall's anchor; tag: the outcome tag, 0 "not an incident", 1 to 5 the
//! known kinds, 6 to 9 the hard kinds, in declaration order; value: the engram for a recall, what
//! the bind did for an answer, a count otherwise) is in the table of the design section. The arm
//! does not know tiers: an outcome tag is the kind the reasoner (or the memory) named, which is
//! all the arm sees.

use gordian_medium::{Mark, MarkLog};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The environment variable naming the directory the trace files are written to.
pub const TRACE_DIR_ENV: &str = "GORDIAN_ENGRAM_TRACE_DIR";

/// The header of a trace file.
pub const FILE_HEADER: &str = "segment,at_ns,event,anomaly,anchor,outcome,value,detail";

/// "No anomaly", "no observation" in a mark's subject or event.
pub const NONE: u32 = u32::MAX;

/// What a mark records (its `kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum TraceKind {
    /// An answer reached the layer; the value is a [`BindNote`].
    Answer = 1,
    /// The bind weakened engrams; the value is how many.
    Contradicted = 2,
    /// The engram medium recalled (not yet resolved to an anomaly).
    Recall = 3,
    /// A recall was resolved to an anomaly: to the gate, or acted on when ungated.
    Offered = 4,
    /// The gate (or no gate) let it through.
    Admitted = 5,
    /// Dropped after the gate's wait; the last reading was a consistent verdict.
    GatedConsistent = 6,
    /// Dropped after the gate's wait; the last reading was a standing declaration (A1d).
    GatedStanding = 7,
    /// Dropped while it waited: the anomaly was asked about, answered or retired.
    Overtaken = 8,
    /// Dropped: no anomaly owned it within the re-offer time, or its outcome named no diagnosis.
    Unmatched = 9,
    /// Dropped: the anomaly was recalled already, or a stronger recall of it won.
    Redundant = 10,
    /// The confirmation policy asked the reasoner instead.
    Confirmed = 11,
    /// The arm declared the recall.
    Declared = 12,
    /// The arm did not declare it (the anomaly was asked about or answered, or the same
    /// declaration was made already).
    NotDeclared = 13,
    /// The layer's segment ended; the value is the number of engrams held.
    SegmentEnd = 14,
}

impl TraceKind {
    /// Every kind, in code order.
    pub const ALL: [TraceKind; 14] = [
        TraceKind::Answer,
        TraceKind::Contradicted,
        TraceKind::Recall,
        TraceKind::Offered,
        TraceKind::Admitted,
        TraceKind::GatedConsistent,
        TraceKind::GatedStanding,
        TraceKind::Overtaken,
        TraceKind::Unmatched,
        TraceKind::Redundant,
        TraceKind::Confirmed,
        TraceKind::Declared,
        TraceKind::NotDeclared,
        TraceKind::SegmentEnd,
    ];

    /// The code a mark carries.
    pub fn code(self) -> u16 {
        self as u16
    }

    /// The kind of a code.
    pub fn of(code: u16) -> Option<TraceKind> {
        TraceKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// The name written to the file.
    pub fn name(self) -> &'static str {
        match self {
            TraceKind::Answer => "answer",
            TraceKind::Contradicted => "contradicted",
            TraceKind::Recall => "recall",
            TraceKind::Offered => "offered",
            TraceKind::Admitted => "admitted",
            TraceKind::GatedConsistent => "gated_consistent",
            TraceKind::GatedStanding => "gated_standing",
            TraceKind::Overtaken => "overtaken",
            TraceKind::Unmatched => "unmatched",
            TraceKind::Redundant => "redundant",
            TraceKind::Confirmed => "confirmed",
            TraceKind::Declared => "declared",
            TraceKind::NotDeclared => "not_declared",
            TraceKind::SegmentEnd => "segment_end",
        }
    }
}

/// What a bind did with an answer (an `answer` mark's value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum BindNote {
    /// A new engram.
    Created = 0,
    /// An engram with exactly this key and outcome, strengthened.
    Strengthened = 1,
    /// An engram with this outcome, narrowed to the shared features and strengthened.
    Generalised = 2,
    /// Not bound: the key had fewer than `min_features` features.
    TooFew = 3,
    /// Not bound: the medium's limits refused it.
    Refused = 4,
    /// Not bound: the key had no late feature (the late switch on).
    NoLate = 5,
    /// Not bound: a family-keyed answer naming another service than the anomaly's.
    Elsewhere = 6,
    /// Not bound: binding is off, or the layer has stopped.
    Off = 7,
    /// Not bound: the noticer no longer held the anomaly.
    Unheld = 8,
}

impl BindNote {
    /// Every note, in code order.
    pub const ALL: [BindNote; 9] = [
        BindNote::Created,
        BindNote::Strengthened,
        BindNote::Generalised,
        BindNote::TooFew,
        BindNote::Refused,
        BindNote::NoLate,
        BindNote::Elsewhere,
        BindNote::Off,
        BindNote::Unheld,
    ];

    /// The value an `answer` mark carries.
    pub fn code(self) -> i64 {
        self as i64
    }

    /// The note of a value.
    pub fn of(code: i64) -> Option<BindNote> {
        BindNote::ALL.iter().copied().find(|n| n.code() == code)
    }

    /// The name written to the file.
    pub fn name(self) -> &'static str {
        match self {
            BindNote::Created => "created",
            BindNote::Strengthened => "strengthened",
            BindNote::Generalised => "generalised",
            BindNote::TooFew => "too_few",
            BindNote::Refused => "refused",
            BindNote::NoLate => "no_late",
            BindNote::Elsewhere => "elsewhere",
            BindNote::Off => "off",
            BindNote::Unheld => "unheld",
        }
    }

    /// Whether the answer was bound (an engram created or strengthened).
    pub fn bound(self) -> bool {
        matches!(
            self,
            BindNote::Created | BindNote::Strengthened | BindNote::Generalised
        )
    }
}

/// One row of a trace file for `mark` in segment `segment`.
pub fn row(segment: u64, mark: &Mark) -> String {
    let kind = TraceKind::of(mark.kind);
    let id = |x: u32| {
        if x == NONE {
            String::new()
        } else {
            x.to_string()
        }
    };
    let detail = match kind {
        Some(TraceKind::Answer) => BindNote::of(mark.value).map_or("", BindNote::name),
        _ => "",
    };
    let outcome = match kind {
        Some(TraceKind::Declared | TraceKind::NotDeclared | TraceKind::SegmentEnd) => String::new(),
        _ => id(mark.tag),
    };
    format!(
        "{segment},{},{},{},{},{outcome},{},{detail}",
        mark.at_ns,
        kind.map_or("unknown", TraceKind::name),
        id(mark.subject),
        id(mark.event),
        mark.value,
    )
}

/// Segments begun per state key in this process (the trace's segment ordinal).
static SEGMENTS: Mutex<BTreeMap<u64, u64>> = Mutex::new(BTreeMap::new());

/// Trace files that could not be written, in this process.
static WRITE_ERRORS: AtomicU64 = AtomicU64::new(0);

/// The ordinal of a new segment of the layer keyed `key`: 0 for the first in the process.
pub fn next_segment(key: u64) -> u64 {
    let mut map = SEGMENTS.lock().unwrap_or_else(PoisonError::into_inner);
    let n = map.entry(key).or_insert(0);
    let out = *n;
    *n += 1;
    out
}

/// Forget the segment count of `key` (tests).
pub fn reset_segments(key: u64) {
    SEGMENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
}

/// Trace files that could not be written so far in this process.
pub fn write_errors() -> u64 {
    WRITE_ERRORS.load(Ordering::Relaxed)
}

/// Append the marks of `log` for segment `segment` of the layer keyed `key` to its trace file in
/// the directory [`TRACE_DIR_ENV`] names ([`append_to`]). Nothing when the variable is unset or
/// empty.
pub fn append(key: u64, segment: u64, log: &MarkLog) {
    if let Some(dir) = std::env::var_os(TRACE_DIR_ENV).filter(|d| !d.is_empty()) {
        append_to(std::path::Path::new(&dir), key, segment, log);
    }
}

/// The path of the trace file of the layer keyed `key` in `dir`.
pub fn file_in(dir: &std::path::Path, key: u64) -> std::path::PathBuf {
    dir.join(format!("engram-trace-{key}.csv"))
}

/// Append the marks of `log` for segment `segment` of the layer keyed `key` to its trace file in
/// `dir` ([`file_in`]), writing the header when the file is new. A failure is counted
/// ([`write_errors`]).
pub fn append_to(dir: &std::path::Path, key: u64, segment: u64, log: &MarkLog) {
    let path = file_in(dir, key);
    let mut text = String::new();
    if std::fs::metadata(&path).map_or(true, |m| m.len() == 0) {
        text.push_str(FILE_HEADER);
        text.push('\n');
    }
    for m in &log.marks {
        text.push_str(&row(segment, m));
        text.push('\n');
    }
    let written = std::fs::create_dir_all(dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(text.as_bytes()))
    });
    if written.is_err() {
        WRITE_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}
