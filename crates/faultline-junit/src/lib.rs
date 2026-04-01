//! JUnit XML export adapter for faultline `AnalysisReport`.

use faultline_types::{AnalysisReport, LocalizationOutcome};
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesEnd, BytesStart, Event};
use std::io::Cursor;

/// Converts an `AnalysisReport` into a JUnit XML string.
pub fn to_junit_xml(report: &AnalysisReport) -> String {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // <?xml version="1.0" encoding="UTF-8"?>
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            None,
        )))
        .expect("write xml decl");

    // <testsuites>
    writer
        .write_event(Event::Start(BytesStart::new("testsuites")))
        .expect("write testsuites start");

    let failure_count = match &report.outcome {
        LocalizationOutcome::FirstBad { .. }
        | LocalizationOutcome::SuspectWindow { .. }
        | LocalizationOutcome::Inconclusive { .. } => "1",
    };

    // <testsuite name="faultline" tests="1" failures="...">
    let mut testsuite = BytesStart::new("testsuite");
    testsuite.push_attribute(("name", "faultline"));
    testsuite.push_attribute(("tests", "1"));
    testsuite.push_attribute(("failures", failure_count));
    writer
        .write_event(Event::Start(testsuite))
        .expect("write testsuite start");

    // <testcase name="regression-localization" classname="faultline.{run_id}">
    let classname = format!("faultline.{}", report.run_id);
    let mut testcase = BytesStart::new("testcase");
    testcase.push_attribute(("name", "regression-localization"));
    testcase.push_attribute(("classname", classname.as_str()));
    writer
        .write_event(Event::Start(testcase))
        .expect("write testcase start");

    // <failure> element based on outcome
    match &report.outcome {
        LocalizationOutcome::FirstBad {
            last_good,
            first_bad,
            ..
        } => {
            let msg = format!("FirstBad: {last_good} \u{2192} {first_bad}");
            let mut failure = BytesStart::new("failure");
            failure.push_attribute(("message", msg.as_str()));
            writer
                .write_event(Event::Empty(failure))
                .expect("write failure");
        }
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive,
            upper_bound_inclusive,
            ..
        } => {
            let msg =
                format!("SuspectWindow: {lower_bound_exclusive} \u{2192} {upper_bound_inclusive}");
            let mut failure = BytesStart::new("failure");
            failure.push_attribute(("message", msg.as_str()));
            writer
                .write_event(Event::Empty(failure))
                .expect("write failure");
        }
        LocalizationOutcome::Inconclusive { reasons } => {
            let reason_strs: Vec<String> = reasons.iter().map(|r| r.to_string()).collect();
            let msg = format!("Inconclusive: {}", reason_strs.join(", "));
            let mut failure = BytesStart::new("failure");
            failure.push_attribute(("message", msg.as_str()));
            writer
                .write_event(Event::Empty(failure))
                .expect("write failure");
        }
    }

    // <system-out> with observations summary
    let observations_summary = build_observations_summary(report);
    writer
        .write_event(Event::Start(BytesStart::new("system-out")))
        .expect("write system-out start");
    writer
        .write_event(Event::CData(BytesCData::new(&observations_summary)))
        .expect("write system-out cdata");
    writer
        .write_event(Event::End(BytesEnd::new("system-out")))
        .expect("write system-out end");

    // </testcase>
    writer
        .write_event(Event::End(BytesEnd::new("testcase")))
        .expect("write testcase end");

    // </testsuite>
    writer
        .write_event(Event::End(BytesEnd::new("testsuite")))
        .expect("write testsuite end");

    // </testsuites>
    writer
        .write_event(Event::End(BytesEnd::new("testsuites")))
        .expect("write testsuites end");

    String::from_utf8(writer.into_inner().into_inner()).expect("valid utf-8")
}

fn build_observations_summary(report: &AnalysisReport) -> String {
    let mut lines = Vec::new();

    if report.observations.is_empty() {
        lines.push("No observations recorded.".to_string());
    } else {
        lines.push("Observations:".to_string());
        for obs in &report.observations {
            lines.push(format!(
                "  {} [{}] {:?} exit={} duration={}ms",
                obs.commit,
                obs.kind,
                obs.class,
                obs.exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "N/A".into()),
                obs.duration_ms,
            ));
        }
    }

    if !report.suspect_surface.is_empty() {
        lines.push(String::new());
        lines.push("Suspect Surface:".to_string());
        for entry in &report.suspect_surface {
            let owner = entry.owner_hint.as_deref().unwrap_or("none");
            lines.push(format!(
                "  [{}] {} ({}, {:?}) owner={}",
                entry.priority_score, entry.path, entry.surface_kind, entry.change_status, owner,
            ));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::*;
    use faultline_types::*;
    use std::path::PathBuf;

    fn sample_report(outcome: LocalizationOutcome) -> AnalysisReport {
        AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "run-42".into(),
            created_at_epoch_seconds: 1700000000,
            request: AnalysisRequest {
                repo_root: PathBuf::from("/tmp/repo"),
                good: RevisionSpec("abc123".into()),
                bad: RevisionSpec("def456".into()),
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
                revisions: vec![CommitId("abc123".into()), CommitId("def456".into())],
            },
            observations: vec![ProbeObservation {
                commit: CommitId("abc123".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 150,
                stdout: "ok".into(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
                flake_signal: None,
            }],
            outcome,
            changed_paths: vec![PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            }],
            surface: SurfaceSummary {
                total_changes: 1,
                buckets: vec![],
                execution_surfaces: vec![],
            },
            suspect_surface: vec![],
            reproduction_capsules: vec![],
        }
    }

    #[test]
    fn junit_first_bad_has_failure_element() {
        let report = sample_report(LocalizationOutcome::FirstBad {
            last_good: CommitId("abc123".into()),
            first_bad: CommitId("def456".into()),
            confidence: Confidence::high(),
        });
        let xml = to_junit_xml(&report);
        assert!(xml.contains("<testsuites>"));
        assert!(xml.contains(r#"name="faultline""#));
        assert!(xml.contains(r#"tests="1""#));
        assert!(xml.contains(r#"failures="1""#));
        assert!(xml.contains(r#"classname="faultline.run-42""#));
        assert!(xml.contains("FirstBad:"));
        assert!(xml.contains("abc123"));
        assert!(xml.contains("def456"));
        assert!(xml.contains("<system-out>"));
    }

    #[test]
    fn junit_suspect_window_has_failure_element() {
        let report = sample_report(LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("aaa".into()),
            upper_bound_inclusive: CommitId("bbb".into()),
            confidence: Confidence::medium(),
            reasons: vec![AmbiguityReason::SkippedRevision],
        });
        let xml = to_junit_xml(&report);
        assert!(xml.contains("SuspectWindow:"));
        assert!(xml.contains("aaa"));
        assert!(xml.contains("bbb"));
    }

    #[test]
    fn junit_inconclusive_has_failure_element() {
        let report = sample_report(LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        });
        let xml = to_junit_xml(&report);
        assert!(xml.contains("Inconclusive:"));
        assert!(xml.contains("missing pass boundary"));
    }

    #[test]
    fn junit_observations_in_system_out() {
        let report = sample_report(LocalizationOutcome::FirstBad {
            last_good: CommitId("abc123".into()),
            first_bad: CommitId("def456".into()),
            confidence: Confidence::high(),
        });
        let xml = to_junit_xml(&report);
        assert!(xml.contains("Observations:"));
        assert!(xml.contains("abc123"));
        assert!(xml.contains("duration=150ms"));
    }

    #[test]
    fn junit_empty_observations() {
        let mut report = sample_report(LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingFailBoundary],
        });
        report.observations = vec![];
        let xml = to_junit_xml(&report);
        assert!(xml.contains("No observations recorded."));
    }

    // Feature: repo-operating-system, Property 42: JUnit XML Export Structural Validity
    // **Validates: Requirements 3.7**
    mod prop_tests {
        use super::super::*;
        use faultline_fixtures::arb::arb_analysis_report;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_junit_xml_export_structural_validity(report in arb_analysis_report()) {
                let xml = to_junit_xml(&report);

                // (a) Well-formed XML (contains <?xml declaration)
                prop_assert!(
                    xml.contains("<?xml"),
                    "JUnit output must contain XML declaration"
                );

                // (b) <testsuites> root
                prop_assert!(
                    xml.contains("<testsuites>"),
                    "JUnit output must contain <testsuites> root element"
                );

                // (c) <testsuite name="faultline">
                prop_assert!(
                    xml.contains(r#"name="faultline""#),
                    "JUnit output must contain testsuite with name=\"faultline\""
                );

                // (d) <testcase> present
                prop_assert!(
                    xml.contains("<testcase"),
                    "JUnit output must contain a <testcase> element"
                );

                // (e) <failure> element present with non-empty message attribute
                prop_assert!(
                    xml.contains("<failure"),
                    "JUnit output must contain a <failure> element"
                );
                // Verify the message attribute is non-empty
                let failure_idx = xml.find("<failure").unwrap();
                let after_failure = &xml[failure_idx..];
                let msg_start = after_failure.find(r#"message=""#).expect("failure must have message attr");
                let msg_value_start = msg_start + r#"message=""#.len();
                let msg_end = after_failure[msg_value_start..].find('"').expect("message attr must be closed");
                prop_assert!(
                    msg_end > 0,
                    "failure message attribute must be non-empty"
                );
            }
        }
    }
}
