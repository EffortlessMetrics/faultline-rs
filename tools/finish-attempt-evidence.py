from pathlib import Path
import re

p = Path('crates/faultline-app/src/lib.rs')
s = p.read_text()
old = 'report.schema_version, "0.1.0",'
assert s.count(old) == 1
s = s.replace(old, 'report.schema_version, "0.4.0",')
s = s.replace('schema_version should be 0.1.0', 'schema_version should be 0.4.0')
p.write_text(s)

old = 'Minimum proportion of consistent results to classify as stable (0.0–1.0)'
new = 'Legacy compatibility value (0.0–1.0); decisive retry results require unanimity'
for name in ['crates/faultline-cli/src/main.rs', 'crates/faultline-cli/src/snapshots/faultline_rs__tests__cli_help.snap']:
    p = Path(name)
    s = p.read_text()
    assert s.count(old) == 1, name
    p.write_text(s.replace(old, new))

p = Path('crates/faultline-render/src/lib.rs')
s = p.read_text()
old = '.filter(|obs| {\n                obs.stdout.ends_with("[truncated]") || obs.stderr.ends_with("[truncated]")\n            })'
new = '.filter(|obs| {\n                report.probe_attempts.is_empty()\n                    && (obs.stdout.ends_with("[truncated]") || obs.stderr.ends_with("[truncated]"))\n            })'
assert s.count(old) == 1
s = s.replace(old, new)
s = s.replace('md.push_str("## Retained attempts\\n\\n");', 'md.push_str("## Retained attempts\\n\\nCaptured output is retained in analysis.json. A truncation marker does not establish that a complete log exists.\\n\\n");')
s = s.replace('<h2>Retained attempts</h2><pre>{}</pre>', '<h2>Retained attempts</h2><p>Captured output is retained in analysis.json. A truncation marker does not establish that a complete log exists.</p><pre>{}</pre>')
p.write_text(s)

p = Path('crates/faultline-app/tests/attempt_evidence.rs')
s = p.read_text()
s += r'''

#[test]
fn invalid_aggregate_inputs_are_rejected_without_erasing_originals() -> TestResult {
    assert!(aggregate_attempts(&[]).is_err());
    let first = raw("c1", ObservationClass::Pass, 1);
    assert!(aggregate_attempts(&[first.clone(), first.clone()]).is_err());
    assert!(aggregate_attempts(&[first.clone(), raw("c2", ObservationClass::Fail, 2)]).is_err());
    for class in [ObservationClass::Skip, ObservationClass::Indeterminate] {
        let attempts = vec![first.clone(), raw("c1", class, 2)];
        let aggregate = aggregate_attempts(&attempts)?;
        assert_eq!(aggregate.class, ObservationClass::Indeterminate);
        assert!(!aggregate.counts.is_stable);
        assert_eq!(attempts.first(), Some(&first));
    }
    Ok(())
}

#[test]
fn tampered_cached_decision_cannot_override_the_journal() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let req = request(temp.path());
    let first = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, None), &store).localize(req.clone())?;
    let mut cached = store.load_observations(&first.run)?;
    cached.first_mut().ok_or("cache")?.class = ObservationClass::Indeterminate;
    std::fs::write(first.run.root.join("observations.json"), serde_json::to_vec(&cached)?)?;
    let probe = Probe::new(0, None);
    assert!(FaultlineApp::new(&History, &Checkout::new(false), &probe, &store).localize(req).is_err());
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(store.load_probe_attempts(&first.run)?, first.report.probe_attempts);
    Ok(())
}

#[test]
fn retained_preview_does_not_advertise_an_unavailable_full_log() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let mut req = request(temp.path());
    req.policy.flake_policy.retries = 0;
    let mut report = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, None), &store).localize(req)?.report;
    report.observations.first_mut().ok_or("observation")?.stdout = "captured preview[truncated]".into();
    report.probe_attempts.first_mut().ok_or("attempt")?.stdout = "captured preview[truncated]".into();
    let export = temp.path().join("export");
    faultline_render::ReportRenderer::new(&export).render_with_markdown(&report)?;
    let html = std::fs::read_to_string(export.join("index.html"))?;
    assert!(!html.contains("href=\"logs/"), "a preview cannot establish full-log custody");
    assert!(html.contains("A truncation marker does not establish"));
    let json = std::fs::read_to_string(export.join("analysis.json"))?;
    assert!(json.contains("captured preview[truncated]"));
    Ok(())
}
'''
p.write_text(s)
p = Path('docs/scenarios/scenario_index.md')
s = p.read_text()
for name in re.findall(r'#\[test\]\s*fn\s+(\w+)', Path('crates/faultline-app/tests/attempt_evidence.rs').read_text()):
    if f'`{name}`' not in s:
        s += f'\n| `{name}` | Preserve attempt provenance and export limits | Native producer and filesystem | faultline-app | Attempt evidence | Original execution is authoritative | #62 #63 | app | retry-evidence | v0.4 | faultline-app | P0 | faultline-app | no |'
p.write_text(s + '\n')
p = Path('docs/probe-attempt-evidence.md')
p.write_text(p.read_text() + '\nThe legacy stability-threshold option remains accepted for request compatibility,\nbut no threshold turns mixed executions into a decisive boundary. CLI help states\nthat rule. New evidence-bearing HTML/Markdown reports do not advertise inferred\nfull-log links merely because captured output ends in a truncation marker. The\ncaptured output is retained in the JSON projection; complete log custody is #60.\n')
