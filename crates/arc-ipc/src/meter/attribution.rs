//! Native process attribution (lane A: MTR-007, AT-MTR-03, MET-027, NFR-012,
//! R-005 item 2).
//!
//! The daemon scans the current user's processes on the Meter's adaptive
//! cadence and assigns each live process one SRS 10.6 confidence:
//!
//! - [`Confidence::Proven`] (A): a member of a RUNNING managed job's tree.
//!   Evidence: ARC's managed spawn recorded the root child identity (pid,
//!   start token, boot id) with verified ownership; the job still holds its
//!   lifecycle lease (state RUNNING); and in the same scan the member's own
//!   identity is revalidated and its ppid chain reaches that root. It is
//!   grouped under the job's recorded vendor, or `none` for a direct command.
//! - [`Confidence::Estimated`] (C, Probable): not Proven, and its
//!   identity-revalidated ppid ancestry reaches a process whose OS process
//!   name is exactly one entry of [`AGENT_EXECUTABLES`].
//! - [`Confidence::Unattributed`] (D, Unknown): everything else. It is never
//!   counted under a vendor or under `none`.
//!
//! A scan contributes, per process identity, the CPU-time delta since the
//! previous scan of that same identity; a first sighting, a changed
//! identity or an unreadable process contributes nothing. Only aggregates
//! are persisted (`process_attribution`, `process_attribution_scans`); no
//! PID, argv, path or cwd of any foreign process is stored, logged or sent.
//!
//! The section is carried by `meter.usage_summary.v2` replies only while the
//! daemon holds [`ADVERTISEMENT`]. Its window is the set of whole UTC minutes
//! that overlap the query's `[from_ms, to_ms)`; a scan interval belongs to
//! the minute of the scan that closed it. `unattributed_ratio` is
//! `100 * unattributed / (proven + estimated + unattributed)` over that same
//! window, and is unavailable (never 0) without a scan or with a zero total.
use super::quality::{Availability, Flagged};
use crate::jobs::usage::Vendor;
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "meter.attribution.v1";
pub const ADVERTISEMENT: &str = ".meter-attribution-v1";
/// The reply key inside a v2 usage summary.
pub const SECTION_KEY: &str = "attribution";
/// The group of a Proven managed job without a vendor.
pub const PROVEN_WITHOUT_VENDOR: &str = "none";
/// Minute buckets of the persisted aggregates.
pub const BUCKET_MS: u64 = 60_000;
pub const BASIS_SCAN_DELTAS: &str = "scan_deltas";

/// The finite executable-to-vendor list, from the two vendor hook adapters
/// (`arcctl hooks install claude|cursor`): Claude Code's CLI executable is
/// `claude`; Cursor's agent CLI is `cursor-agent` and its application
/// process is `Cursor`. The match is exact and case-sensitive on the OS
/// process name; any other name, including a vendor's runtime host such as
/// `node`, is not an agent executable.
pub const AGENT_EXECUTABLES: [(&str, Vendor); 3] = [
    ("claude", Vendor::ClaudeCode),
    ("cursor-agent", Vendor::Cursor),
    ("Cursor", Vendor::Cursor),
];

pub fn vendor_for_executable(name: &str) -> Option<Vendor> {
    AGENT_EXECUTABLES
        .iter()
        .find(|(executable, _)| *executable == name)
        .map(|(_, vendor)| *vendor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Proven,
    Estimated,
    Unattributed,
}
impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::Estimated => "estimated",
            Self::Unattributed => "unattributed",
        }
    }
    /// The SRS 10.6 letter.
    pub fn letter(self) -> char {
        match self {
            Self::Proven => 'A',
            Self::Estimated => 'C',
            Self::Unattributed => 'D',
        }
    }
}

/// The observed part of the requested window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionWindow {
    /// Start of the first and end of the last minute overlapping the query.
    pub from_ms: u64,
    pub to_ms: u64,
    /// Completed scans in the window; 0 means nothing was observed.
    pub scans: u64,
    pub first_scan_ms: Option<u64>,
    pub last_scan_ms: Option<u64>,
    /// Processes that contributed nothing (identity changed or unreadable).
    pub skipped_processes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorCpu {
    /// A [`Vendor`] id, or [`PROVEN_WITHOUT_VENDOR`] under `proven` only.
    pub vendor: String,
    pub cpu_core_seconds: Flagged<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionSection {
    pub window: AttributionWindow,
    pub proven: Vec<VendorCpu>,
    pub estimated: Vec<VendorCpu>,
    pub unattributed: Flagged<f64>,
    /// Percent, 0 to 100.
    pub unattributed_ratio: Flagged<f64>,
}

fn is_vendor(name: &str) -> bool {
    serde_json::from_value::<Vendor>(serde_json::Value::String(name.to_owned())).is_ok()
}

impl AttributionSection {
    /// Shape checks a reader applies before showing the section.
    pub fn is_valid(&self) -> bool {
        let unique = |groups: &[VendorCpu]| {
            groups
                .iter()
                .enumerate()
                .all(|(i, g)| groups[..i].iter().all(|h| h.vendor != g.vendor))
        };
        let window = &self.window;
        let ratio_ok = self
            .unattributed_ratio
            .value
            .is_none_or(|r| r.is_finite() && (0.0..=100.0).contains(&r));
        let values = self
            .proven
            .iter()
            .chain(&self.estimated)
            .map(|g| &g.cpu_core_seconds)
            .chain([&self.unattributed, &self.unattributed_ratio]);
        window.from_ms < window.to_ms
            && (window.scans == 0) == window.first_scan_ms.is_none()
            && window.first_scan_ms.is_none() == window.last_scan_ms.is_none()
            && unique(&self.proven)
            && unique(&self.estimated)
            && self
                .proven
                .iter()
                .all(|g| g.vendor == PROVEN_WITHOUT_VENDOR || is_vendor(&g.vendor))
            && self.estimated.iter().all(|g| is_vendor(&g.vendor))
            && ratio_ok
            && values
                .into_iter()
                .all(|v| v.is_consistent() && v.value.is_none_or(|x| x.is_finite() && x >= 0.0))
            && (window.scans != 0
                || self.unattributed_ratio.availability == Availability::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_executable_list_is_finite_exact_and_case_sensitive() {
        assert_eq!(vendor_for_executable("claude"), Some(Vendor::ClaudeCode));
        assert_eq!(vendor_for_executable("cursor-agent"), Some(Vendor::Cursor));
        assert_eq!(vendor_for_executable("Cursor"), Some(Vendor::Cursor));
        for other in ["Claude", "claude-code", "cursor", "node", "", "claude "] {
            assert_eq!(vendor_for_executable(other), None, "{other}");
        }
        assert_eq!(Confidence::Unattributed.letter(), 'D');
    }

    #[test]
    fn unattributed_never_names_a_vendor_and_an_unobserved_ratio_is_unavailable() {
        let section = AttributionSection {
            window: AttributionWindow {
                from_ms: 0,
                to_ms: BUCKET_MS,
                scans: 0,
                first_scan_ms: None,
                last_scan_ms: None,
                skipped_processes: 0,
            },
            proven: vec![],
            estimated: vec![],
            unattributed: Flagged::unavailable(BASIS_SCAN_DELTAS, "no_scan"),
            unattributed_ratio: Flagged::unavailable(BASIS_SCAN_DELTAS, "no_scan"),
        };
        assert!(section.is_valid());
        let mut wrong = section.clone();
        wrong.estimated.push(VendorCpu {
            vendor: PROVEN_WITHOUT_VENDOR.into(),
            cpu_core_seconds: Flagged::unavailable(BASIS_SCAN_DELTAS, "no_scan"),
        });
        assert!(!wrong.is_valid());
        let mut zero = section;
        zero.unattributed_ratio = Flagged {
            value: Some(0.0),
            availability: Availability::Complete,
            basis: BASIS_SCAN_DELTAS.into(),
            reason: None,
        };
        assert!(!zero.is_valid());
    }
}
