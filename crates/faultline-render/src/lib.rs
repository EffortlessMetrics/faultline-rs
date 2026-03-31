use faultline_types::{AnalysisReport, LocalizationOutcome, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ReportRenderer {
    output_dir: PathBuf,
}

impl ReportRenderer {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn render(&self, report: &AnalysisReport) -> Result<()> {
        fs::create_dir_all(&self.output_dir)?;
        fs::write(
            self.output_dir.join("analysis.json"),
            serde_json::to_string_pretty(report)?,
        )?;
        fs::write(self.output_dir.join("index.html"), self.render_html(report))?;
        Ok(())
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    fn render_html(&self, report: &AnalysisReport) -> String {
        let outcome_html = match &report.outcome {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => format!(
                "<p><strong>Exact boundary:</strong> {} → {} (confidence: {} / {})</p>",
                escape_html(&last_good.0),
                escape_html(&first_bad.0),
                confidence.score,
                escape_html(&confidence.label),
            ),
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons,
            } => format!(
                "<p><strong>Suspect window:</strong> {} → {} (confidence: {} / {})</p><p>Reasons: {}</p>",
                escape_html(&lower_bound_exclusive.0),
                escape_html(&upper_bound_inclusive.0),
                confidence.score,
                escape_html(&confidence.label),
                reasons
                    .iter()
                    .map(|reason| escape_html(&reason.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            LocalizationOutcome::Inconclusive { reasons } => format!(
                "<p><strong>Inconclusive.</strong> Reasons: {}</p>",
                reasons
                    .iter()
                    .map(|reason| escape_html(&reason.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        };

        let observations = report
            .observations
            .iter()
            .map(|obs| {
                format!(
                    "<tr><td><code>{}</code></td><td>{:?}</td><td>{}</td><td>{:?}</td><td>{}</td></tr>",
                    escape_html(&obs.commit.0),
                    obs.class,
                    escape_html(&obs.kind.to_string()),
                    obs.exit_code,
                    obs.duration_ms,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let buckets = report
            .surface
            .buckets
            .iter()
            .map(|bucket| {
                format!(
                    "<li><strong>{}</strong> — {} changes — kinds: {}<br/><small>{}</small></li>",
                    escape_html(&bucket.name),
                    bucket.change_count,
                    escape_html(&bucket.surface_kinds.join(", ")),
                    escape_html(&bucket.paths.join(", ")),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let changed_paths = report
            .changed_paths
            .iter()
            .map(|change| {
                format!(
                    "<li><code>{:?}</code> {}</li>",
                    change.status,
                    escape_html(&change.path),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>faultline report</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 1100px; line-height: 1.5; }}
    code {{ background: #f3f4f6; padding: 0.15rem 0.35rem; border-radius: 4px; }}
    table {{ width: 100%; border-collapse: collapse; }}
    th, td {{ border: 1px solid #ddd; padding: 0.5rem; text-align: left; }}
    th {{ background: #f8f8f8; }}
  </style>
</head>
<body>
  <h1>faultline</h1>
  <p>Run ID: <code>{}</code></p>
  <h2>Summary</h2>
  {}
  <p><strong>Probe fingerprint:</strong> <code>{}</code></p>
  <p><strong>History mode:</strong> {:?}</p>
  <h2>Observation timeline</h2>
  <table>
    <thead><tr><th>Commit</th><th>Class</th><th>Kind</th><th>Exit</th><th>Duration ms</th></tr></thead>
    <tbody>{}</tbody>
  </table>
  <h2>Changed surface</h2>
  <ul>{}</ul>
  <h2>Changed paths</h2>
  <ul>{}</ul>
</body>
</html>"#,
            escape_html(&report.run_id),
            outcome_html,
            escape_html(&report.request.probe.fingerprint()),
            report.request.history_mode,
            observations,
            buckets,
            changed_paths,
        )
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_types::*;
    use std::path::PathBuf;

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            run_id: "test-run-001".into(),
            created_at_epoch_seconds: 1700000000,
            request: AnalysisRequest {
                repo_root: PathBuf::from("/tmp/repo"),
                good: RevisionSpec("aaa111".into()),
                bad: RevisionSpec("bbb222".into()),
                history_mode: HistoryMode::AncestryPath,
                probe: ProbeSpec::Exec {
                    kind: ProbeKind::Test,
                    program: "cargo".into(),
                    args: vec!["test".into()],
                    env: vec![],
                    timeout_seconds: 300,
                },
                policy: SearchPolicy::default(),
            },
            sequence: RevisionSequence {
                revisions: vec![CommitId("aaa111".into()), CommitId("bbb222".into())],
            },
            observations: vec![
                ProbeObservation {
                    commit: CommitId("aaa111".into()),
                    class: ObservationClass::Pass,
                    kind: ProbeKind::Test,
                    exit_code: Some(0),
                    timed_out: false,
                    duration_ms: 120,
                    stdout: "ok".into(),
                    stderr: String::new(),
                },
                ProbeObservation {
                    commit: CommitId("bbb222".into()),
                    class: ObservationClass::Fail,
                    kind: ProbeKind::Test,
                    exit_code: Some(1),
                    timed_out: false,
                    duration_ms: 95,
                    stdout: String::new(),
                    stderr: "test failed".into(),
                },
            ],
            outcome: LocalizationOutcome::FirstBad {
                last_good: CommitId("aaa111".into()),
                first_bad: CommitId("bbb222".into()),
                confidence: Confidence::high(),
            },
            changed_paths: vec![PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            }],
            surface: SurfaceSummary {
                total_changes: 1,
                buckets: vec![SubsystemBucket {
                    name: "src".into(),
                    change_count: 1,
                    paths: vec!["src/main.rs".into()],
                    surface_kinds: vec!["source".into()],
                }],
                execution_surfaces: vec![],
            },
        }
    }

    fn temp_output_dir(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("faultline-render-tests")
            .join(name)
            .join(format!("{}", std::process::id()))
    }

    // Req 6.1: writes analysis.json to output directory
    #[test]
    fn render_writes_analysis_json() {
        let dir = temp_output_dir("writes-json");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).expect("render should succeed");

        let json_path = dir.join("analysis.json");
        assert!(json_path.exists(), "analysis.json must be created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Req 6.2: analysis.json contains all required fields
    #[test]
    fn analysis_json_contains_all_fields() {
        let dir = temp_output_dir("all-fields");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).expect("render should succeed");

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed.get("run_id").is_some());
        assert!(parsed.get("created_at_epoch_seconds").is_some());
        assert!(parsed.get("request").is_some());
        assert!(parsed.get("sequence").is_some());
        assert!(parsed.get("observations").is_some());
        assert!(parsed.get("outcome").is_some());
        assert!(parsed.get("changed_paths").is_some());
        assert!(parsed.get("surface").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Req 6.3: deterministic output — same report produces identical JSON
    #[test]
    fn analysis_json_is_deterministic() {
        let dir1 = temp_output_dir("deterministic-1");
        let dir2 = temp_output_dir("deterministic-2");
        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);

        let report = sample_report();
        ReportRenderer::new(&dir1).render(&report).unwrap();
        ReportRenderer::new(&dir2).render(&report).unwrap();

        let json1 = std::fs::read_to_string(dir1.join("analysis.json")).unwrap();
        let json2 = std::fs::read_to_string(dir2.join("analysis.json")).unwrap();
        assert_eq!(
            json1, json2,
            "identical reports must produce identical JSON"
        );

        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    // Req 6.4: analysis.json is valid JSON
    #[test]
    fn analysis_json_is_valid_json() {
        let dir = temp_output_dir("valid-json");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).unwrap();

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let result: std::result::Result<serde_json::Value, _> = serde_json::from_str(&content);
        assert!(result.is_ok(), "analysis.json must be valid JSON");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Creates output directory if it doesn't exist
    #[test]
    fn render_creates_output_directory() {
        let dir = temp_output_dir("creates-dir");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!dir.exists());

        let renderer = ReportRenderer::new(&dir);
        renderer.render(&sample_report()).unwrap();

        assert!(dir.exists(), "output directory must be created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // output_dir accessor returns the configured path
    #[test]
    fn output_dir_returns_configured_path() {
        let dir = PathBuf::from("/some/output/path");
        let renderer = ReportRenderer::new(&dir);
        assert_eq!(renderer.output_dir(), dir.as_path());
    }

    // Round-trip: deserializing the written JSON produces the original report
    #[test]
    fn analysis_json_round_trips() {
        let dir = temp_output_dir("round-trip");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).unwrap();

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let deserialized: AnalysisReport = serde_json::from_str(&content).unwrap();
        assert_eq!(report, deserialized);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- HTML report tests (Req 7.1, 7.2, 7.3, 7.5) ---

    // Req 7.1: writes index.html to output directory
    #[test]
    fn render_writes_index_html() {
        let dir = temp_output_dir("writes-html");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);

        renderer.render(&sample_report()).unwrap();

        assert!(
            dir.join("index.html").exists(),
            "index.html must be created"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Req 7.2: HTML contains run ID
    #[test]
    fn html_contains_run_id() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains("test-run-001"),
            "HTML must contain the run ID"
        );
    }

    // Req 7.2: HTML contains outcome summary for FirstBad
    #[test]
    fn html_contains_first_bad_outcome() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains("Exact boundary"),
            "HTML must show FirstBad outcome"
        );
        assert!(html.contains("aaa111"), "HTML must contain last_good SHA");
        assert!(html.contains("bbb222"), "HTML must contain first_bad SHA");
    }

    // Req 7.2: HTML contains outcome summary for SuspectWindow
    #[test]
    fn html_contains_suspect_window_outcome() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let mut report = sample_report();
        report.outcome = LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("aaa111".into()),
            upper_bound_inclusive: CommitId("bbb222".into()),
            confidence: Confidence::medium(),
            reasons: vec![faultline_codes::AmbiguityReason::SkippedRevision],
        };
        let html = renderer.render_html(&report);

        assert!(
            html.contains("Suspect window"),
            "HTML must show SuspectWindow outcome"
        );
        assert!(html.contains("aaa111"), "HTML must contain lower bound SHA");
        assert!(html.contains("bbb222"), "HTML must contain upper bound SHA");
    }

    // Req 7.2: HTML contains outcome summary for Inconclusive
    #[test]
    fn html_contains_inconclusive_outcome() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let mut report = sample_report();
        report.outcome = LocalizationOutcome::Inconclusive {
            reasons: vec![faultline_codes::AmbiguityReason::MissingPassBoundary],
        };
        let html = renderer.render_html(&report);

        assert!(
            html.contains("Inconclusive"),
            "HTML must show Inconclusive outcome"
        );
    }

    // Req 7.2: HTML contains probe fingerprint and history mode
    #[test]
    fn html_contains_probe_fingerprint_and_history_mode() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains(&report.request.probe.fingerprint()),
            "HTML must contain probe fingerprint"
        );
        assert!(
            html.contains("AncestryPath"),
            "HTML must contain history mode"
        );
    }

    // Req 7.2: HTML contains observation timeline with one row per observation
    #[test]
    fn html_contains_observation_timeline_rows() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        let tr_count = html.matches("<tr>").count();
        // 1 header row + 2 observation rows = 3
        assert_eq!(tr_count, 3, "HTML must have 1 header + 2 observation rows");
        assert!(
            html.contains("aaa111"),
            "observation row must contain commit SHA"
        );
        assert!(
            html.contains("120"),
            "observation row must contain duration"
        );
    }

    // Req 7.2: HTML contains changed-surface buckets
    #[test]
    fn html_contains_surface_buckets() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains("Changed surface"),
            "HTML must have changed surface section"
        );
        assert!(html.contains("src"), "HTML must contain bucket name");
        assert!(html.contains("source"), "HTML must contain surface kind");
    }

    // Req 7.2: HTML contains changed paths
    #[test]
    fn html_contains_changed_paths() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains("Changed paths"),
            "HTML must have changed paths section"
        );
        assert!(
            html.contains("src/main.rs"),
            "HTML must contain changed path"
        );
        assert!(html.contains("Modified"), "HTML must contain change status");
    }

    // Req 7.3: HTML is self-contained — no external resource dependencies
    #[test]
    fn html_has_no_external_dependencies() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        // No <link> tags with external URLs
        assert!(!html.contains("<link"), "HTML must not contain <link> tags");
        // No <script> tags
        assert!(
            !html.contains("<script"),
            "HTML must not contain <script> tags"
        );
        // No external URLs in img tags
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "HTML must not reference external URLs"
        );
    }

    // Req 7.3: HTML has inline CSS
    #[test]
    fn html_has_inline_css() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(
            html.contains("<style>"),
            "HTML must contain inline <style> block"
        );
    }

    // Req 7.5: escape_html replaces all special characters
    #[test]
    fn escape_html_replaces_special_chars() {
        let input = r#"<script>alert("xss")</script> & it's 'bad'"#;
        let escaped = escape_html(input);

        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&#39;"));
    }

    // Req 7.5: HTML-escapes dynamic content (run_id with special chars)
    #[test]
    fn html_escapes_dynamic_content() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let mut report = sample_report();
        report.run_id = "run-<script>alert(1)</script>".into();
        let html = renderer.render_html(&report);

        assert!(
            !html.contains("<script>alert(1)</script>"),
            "dynamic content must be HTML-escaped"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "special chars must be escaped to entities"
        );
    }

    // Req 7.1: HTML is valid (starts with doctype, has html/head/body)
    #[test]
    fn html_has_valid_structure() {
        let renderer = ReportRenderer::new("/tmp/unused");
        let report = sample_report();
        let html = renderer.render_html(&report);

        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<html"));
        assert!(html.contains("<head>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("</html>"));
    }

    // --- Proptest strategies for Property 16 ---

    use proptest::prelude::*;

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
    }

    fn arb_revision_spec() -> impl Strategy<Value = RevisionSpec> {
        "[a-f0-9]{8,40}".prop_map(RevisionSpec)
    }

    fn arb_history_mode() -> impl Strategy<Value = HistoryMode> {
        prop_oneof![
            Just(HistoryMode::AncestryPath),
            Just(HistoryMode::FirstParent),
        ]
    }

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    fn arb_shell_kind() -> impl Strategy<Value = ShellKind> {
        prop_oneof![
            Just(ShellKind::Default),
            Just(ShellKind::PosixSh),
            Just(ShellKind::Cmd),
            Just(ShellKind::PowerShell),
        ]
    }

    fn arb_probe_spec() -> impl Strategy<Value = ProbeSpec> {
        prop_oneof![
            (
                arb_probe_kind(),
                "[a-z]{1,10}",
                prop::collection::vec("[a-z0-9]{1,8}", 0..3),
                prop::collection::vec(("[A-Z]{1,4}", "[a-z0-9]{1,6}"), 0..2),
                1u64..600,
            )
                .prop_map(|(kind, program, args, env, timeout_seconds)| {
                    ProbeSpec::Exec {
                        kind,
                        program,
                        args,
                        env,
                        timeout_seconds,
                    }
                }),
            (
                arb_probe_kind(),
                arb_shell_kind(),
                "[a-z ]{1,20}",
                1u64..600
            )
                .prop_map(|(kind, shell, script, timeout_seconds)| {
                    ProbeSpec::Shell {
                        kind,
                        shell,
                        script,
                        timeout_seconds,
                    }
                }),
        ]
    }

    fn arb_search_policy() -> impl Strategy<Value = SearchPolicy> {
        (1usize..128, 1usize..16).prop_map(|(max_probes, edge_refine_threshold)| SearchPolicy {
            max_probes,
            edge_refine_threshold,
        })
    }

    fn arb_analysis_request() -> impl Strategy<Value = AnalysisRequest> {
        (
            "[a-z/]{1,20}",
            arb_revision_spec(),
            arb_revision_spec(),
            arb_history_mode(),
            arb_probe_spec(),
            arb_search_policy(),
        )
            .prop_map(|(repo_root, good, bad, history_mode, probe, policy)| {
                AnalysisRequest {
                    repo_root: PathBuf::from(repo_root),
                    good,
                    bad,
                    history_mode,
                    probe,
                    policy,
                }
            })
    }

    fn arb_revision_sequence() -> impl Strategy<Value = RevisionSequence> {
        prop::collection::vec(arb_commit_id(), 2..10)
            .prop_map(|revisions| RevisionSequence { revisions })
    }

    fn arb_observation_class() -> impl Strategy<Value = ObservationClass> {
        prop_oneof![
            Just(ObservationClass::Pass),
            Just(ObservationClass::Fail),
            Just(ObservationClass::Skip),
            Just(ObservationClass::Indeterminate),
        ]
    }

    fn arb_probe_observation() -> impl Strategy<Value = ProbeObservation> {
        (
            arb_commit_id(),
            arb_observation_class(),
            arb_probe_kind(),
            prop::option::of(any::<i32>()),
            any::<bool>(),
            any::<u64>(),
            "[a-z ]{0,20}",
            "[a-z ]{0,20}",
        )
            .prop_map(
                |(commit, class, kind, exit_code, timed_out, duration_ms, stdout, stderr)| {
                    ProbeObservation {
                        commit,
                        class,
                        kind,
                        exit_code,
                        timed_out,
                        duration_ms,
                        stdout,
                        stderr,
                    }
                },
            )
    }

    fn arb_confidence() -> impl Strategy<Value = Confidence> {
        (any::<u8>(), "[a-z]{1,10}").prop_map(|(score, label)| Confidence { score, label })
    }

    fn arb_ambiguity_reason() -> impl Strategy<Value = faultline_codes::AmbiguityReason> {
        use faultline_codes::AmbiguityReason;
        prop_oneof![
            Just(AmbiguityReason::MissingPassBoundary),
            Just(AmbiguityReason::MissingFailBoundary),
            Just(AmbiguityReason::NonMonotonicEvidence),
            Just(AmbiguityReason::SkippedRevision),
            Just(AmbiguityReason::IndeterminateRevision),
            Just(AmbiguityReason::UntestableWindow),
            Just(AmbiguityReason::BoundaryValidationFailed),
            Just(AmbiguityReason::NeedsMoreProbes),
        ]
    }

    fn arb_localization_outcome() -> impl Strategy<Value = LocalizationOutcome> {
        prop_oneof![
            (arb_commit_id(), arb_commit_id(), arb_confidence()).prop_map(
                |(last_good, first_bad, confidence)| {
                    LocalizationOutcome::FirstBad {
                        last_good,
                        first_bad,
                        confidence,
                    }
                }
            ),
            (
                arb_commit_id(),
                arb_commit_id(),
                arb_confidence(),
                prop::collection::vec(arb_ambiguity_reason(), 1..4),
            )
                .prop_map(
                    |(lower_bound_exclusive, upper_bound_inclusive, confidence, reasons)| {
                        LocalizationOutcome::SuspectWindow {
                            lower_bound_exclusive,
                            upper_bound_inclusive,
                            confidence,
                            reasons,
                        }
                    }
                ),
            prop::collection::vec(arb_ambiguity_reason(), 1..4)
                .prop_map(|reasons| LocalizationOutcome::Inconclusive { reasons }),
        ]
    }

    fn arb_change_status() -> impl Strategy<Value = ChangeStatus> {
        prop_oneof![
            Just(ChangeStatus::Added),
            Just(ChangeStatus::Modified),
            Just(ChangeStatus::Deleted),
            Just(ChangeStatus::Renamed),
            Just(ChangeStatus::TypeChanged),
            Just(ChangeStatus::Unknown),
        ]
    }

    fn arb_path_change() -> impl Strategy<Value = PathChange> {
        (arb_change_status(), "[a-z/]{1,30}").prop_map(|(status, path)| PathChange { status, path })
    }

    fn arb_subsystem_bucket() -> impl Strategy<Value = SubsystemBucket> {
        (
            "[a-z]{1,10}",
            0usize..20,
            prop::collection::vec("[a-z/]{1,20}", 0..5),
            prop::collection::vec("[a-z]{1,10}", 0..3),
        )
            .prop_map(
                |(name, change_count, paths, surface_kinds)| SubsystemBucket {
                    name,
                    change_count,
                    paths,
                    surface_kinds,
                },
            )
    }

    fn arb_surface_summary() -> impl Strategy<Value = SurfaceSummary> {
        (
            0usize..50,
            prop::collection::vec(arb_subsystem_bucket(), 0..5),
            prop::collection::vec("[a-z/]{1,20}", 0..3),
        )
            .prop_map(
                |(total_changes, buckets, execution_surfaces)| SurfaceSummary {
                    total_changes,
                    buckets,
                    execution_surfaces,
                },
            )
    }

    fn arb_analysis_report() -> impl Strategy<Value = AnalysisReport> {
        (
            "[a-z0-9-]{1,20}",
            any::<u64>(),
            arb_analysis_request(),
            arb_revision_sequence(),
            prop::collection::vec(arb_probe_observation(), 0..5),
            arb_localization_outcome(),
            prop::collection::vec(arb_path_change(), 0..5),
            arb_surface_summary(),
        )
            .prop_map(
                |(
                    run_id,
                    created_at_epoch_seconds,
                    request,
                    sequence,
                    observations,
                    outcome,
                    changed_paths,
                    surface,
                )| {
                    AnalysisReport {
                        run_id,
                        created_at_epoch_seconds,
                        request,
                        sequence,
                        observations,
                        outcome,
                        changed_paths,
                        surface,
                    }
                },
            )
    }

    // Feature: v01-release-train, Property 16: HTML Contains Required Data Consistent with JSON
    // **Validates: Requirements 7.2, 7.4, 11.5**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_html_contains_required_data(report in arb_analysis_report()) {
            let renderer = ReportRenderer::new("/tmp/unused");
            let html = renderer.render_html(&report);

            // HTML must contain the run_id (HTML-escaped)
            let escaped_run_id = escape_html(&report.run_id);
            prop_assert!(
                html.contains(&escaped_run_id),
                "HTML must contain run_id '{}' (escaped: '{}')",
                report.run_id,
                escaped_run_id,
            );

            // HTML must contain the outcome type label
            match &report.outcome {
                LocalizationOutcome::FirstBad { last_good, first_bad, .. } => {
                    prop_assert!(
                        html.contains("Exact boundary"),
                        "HTML must contain 'Exact boundary' for FirstBad outcome"
                    );
                    // Boundary SHAs must be present (HTML-escaped)
                    let escaped_good = escape_html(&last_good.0);
                    let escaped_bad = escape_html(&first_bad.0);
                    prop_assert!(
                        html.contains(&escaped_good),
                        "HTML must contain last_good SHA '{}'", last_good.0
                    );
                    prop_assert!(
                        html.contains(&escaped_bad),
                        "HTML must contain first_bad SHA '{}'", first_bad.0
                    );
                }
                LocalizationOutcome::SuspectWindow { lower_bound_exclusive, upper_bound_inclusive, .. } => {
                    prop_assert!(
                        html.contains("Suspect window"),
                        "HTML must contain 'Suspect window' for SuspectWindow outcome"
                    );
                    // Boundary SHAs must be present (HTML-escaped)
                    let escaped_lower = escape_html(&lower_bound_exclusive.0);
                    let escaped_upper = escape_html(&upper_bound_inclusive.0);
                    prop_assert!(
                        html.contains(&escaped_lower),
                        "HTML must contain lower_bound SHA '{}'", lower_bound_exclusive.0
                    );
                    prop_assert!(
                        html.contains(&escaped_upper),
                        "HTML must contain upper_bound SHA '{}'", upper_bound_inclusive.0
                    );
                }
                LocalizationOutcome::Inconclusive { .. } => {
                    prop_assert!(
                        html.contains("Inconclusive"),
                        "HTML must contain 'Inconclusive' for Inconclusive outcome"
                    );
                }
            }

            // HTML must contain one <tr> per observation (plus 1 header row)
            let tr_count = html.matches("<tr>").count();
            let expected_tr = 1 + report.observations.len(); // 1 header + N observation rows
            prop_assert_eq!(
                tr_count,
                expected_tr,
                "HTML must have exactly 1 header <tr> + {} observation <tr> rows, got {}",
                report.observations.len(),
                tr_count,
            );

            // Each observation's commit SHA must appear in the HTML
            for obs in &report.observations {
                let escaped_commit = escape_html(&obs.commit.0);
                prop_assert!(
                    html.contains(&escaped_commit),
                    "HTML must contain observation commit SHA '{}'", obs.commit.0
                );
            }
        }

        // Feature: v01-release-train, Property 17: HTML Escaping Correctness
        // **Validates: Requirement 7.5**
        #[test]
        fn prop_html_escaping_correctness(input in ".*[<>&\"'].*") {
            let escaped = escape_html(&input);

            // The output must not contain any raw HTML special characters
            prop_assert!(
                !escaped.contains('<'),
                "escaped output must not contain raw '<', got: {}", escaped
            );
            prop_assert!(
                !escaped.contains('>'),
                "escaped output must not contain raw '>', got: {}", escaped
            );
            prop_assert!(
                !escaped.contains('"'),
                "escaped output must not contain raw '\"', got: {}", escaped
            );

            // For '&', every occurrence must be part of an entity (not a raw '&')
            // We check that no '&' exists that isn't followed by amp;, lt;, gt;, quot;, or #39;
            for (i, _) in escaped.match_indices('&') {
                let rest = &escaped[i..];
                prop_assert!(
                    rest.starts_with("&amp;")
                        || rest.starts_with("&lt;")
                        || rest.starts_with("&gt;")
                        || rest.starts_with("&quot;")
                        || rest.starts_with("&#39;"),
                    "every '&' in output must be part of an HTML entity, found raw '&' at index {}: ...{}...",
                    i,
                    &escaped[i..std::cmp::min(i + 10, escaped.len())]
                );
            }

            // Single quotes must be replaced with &#39;
            prop_assert!(
                !escaped.contains('\''),
                "escaped output must not contain raw single quote, got: {}", escaped
            );

            // Each special char in the input must have a corresponding entity in the output
            let amp_count = input.chars().filter(|&c| c == '&').count();
            let lt_count = input.chars().filter(|&c| c == '<').count();
            let gt_count = input.chars().filter(|&c| c == '>').count();
            let quot_count = input.chars().filter(|&c| c == '"').count();
            let apos_count = input.chars().filter(|&c| c == '\'').count();

            prop_assert_eq!(
                escaped.matches("&lt;").count(), lt_count,
                "number of &lt; entities must match number of '<' in input"
            );
            prop_assert_eq!(
                escaped.matches("&gt;").count(), gt_count,
                "number of &gt; entities must match number of '>' in input"
            );
            prop_assert_eq!(
                escaped.matches("&quot;").count(), quot_count,
                "number of &quot; entities must match number of '\"' in input"
            );
            prop_assert_eq!(
                escaped.matches("&#39;").count(), apos_count,
                "number of &#39; entities must match number of single quotes in input"
            );
            // &amp; count = original '&' count (each '&' becomes exactly one &amp;)
            prop_assert_eq!(
                escaped.matches("&amp;").count(), amp_count,
                "number of &amp; entities must match number of '&' in input"
            );
        }

        // Feature: v01-release-train, Property 18: HTML Is Self-Contained
        // **Validates: Requirement 7.3**
        #[test]
        fn prop_html_is_self_contained(report in arb_analysis_report()) {
            let renderer = ReportRenderer::new("/tmp/unused");
            let html = renderer.render_html(&report);

            // Scan for <link> tags referencing external URLs
            for (i, _) in html.match_indices("<link") {
                let tag_end = html[i..].find('>').unwrap_or(html.len() - i);
                let tag = &html[i..i + tag_end + 1];
                prop_assert!(
                    !tag.contains("http://") && !tag.contains("https://"),
                    "HTML must not contain <link> tags with external URLs, found: {}", tag
                );
            }

            // Scan for <script> tags referencing external URLs
            for (i, _) in html.match_indices("<script") {
                let tag_end = html[i..].find('>').unwrap_or(html.len() - i);
                let tag = &html[i..i + tag_end + 1];
                prop_assert!(
                    !tag.contains("http://") && !tag.contains("https://"),
                    "HTML must not contain <script> tags with external URLs, found: {}", tag
                );
            }

            // Scan for <img> tags referencing external URLs
            for (i, _) in html.match_indices("<img") {
                let tag_end = html[i..].find('>').unwrap_or(html.len() - i);
                let tag = &html[i..i + tag_end + 1];
                prop_assert!(
                    !tag.contains("http://") && !tag.contains("https://"),
                    "HTML must not contain <img> tags with external URLs, found: {}", tag
                );
            }
        }
    }
}
