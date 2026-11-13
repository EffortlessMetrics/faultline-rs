use faultline_queue::{DiagnosisInput, DiagnosisReport, DiagnosisStatus};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "faultline-queue-{}-{timestamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("fixture cleanup failed: {error}");
        }
    }
}
fn cli(args: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_faultline-queue"))
        .args(args)
        .output()
}
fn demo() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = cli(&["demo"])?;
    assert!(output.status.success());
    let packet: DiagnosisInput = serde_json::from_slice(&output.stdout)?;
    assert_eq!(packet.sequence.len(), 21);
    assert_eq!(packet.sequence.provider, "synthetic");
    Ok(output.stdout)
}

#[test]
fn installed_cli_demo_produces_a_non_green_bounded_plan() -> TestResult {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input.json");
    let output = scratch.0.join("report.json");
    fs::write(&input, demo()?)?;
    let result = cli(&[
        "diagnose",
        "--input",
        input.to_str().ok_or("input")?,
        "--output",
        output.to_str().ok_or("output")?,
    ])?;
    assert_eq!(result.status.code(), Some(2));
    let report: DiagnosisReport = serde_json::from_slice(&fs::read(output)?)?;
    assert_eq!(report.status, DiagnosisStatus::NeedsEvidence);
    assert_eq!(report.next_probe.ok_or("plan")?.state.ordinal, 10);
    assert_eq!(report.recorded_launches, 0);
    Ok(())
}

#[test]
fn installed_cli_never_overwrites_an_existing_output_or_input() -> TestResult {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input.json");
    let original = demo()?;
    fs::write(&input, &original)?;
    let result = cli(&[
        "diagnose",
        "--input",
        input.to_str().ok_or("input")?,
        "--output",
        input.to_str().ok_or("input")?,
    ])?;
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(fs::read(input)?, original);
    Ok(())
}

#[test]
fn installed_cli_rejects_malformed_and_oversized_packets() -> TestResult {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input.json");
    fs::write(&input, b"{ invalid")?;
    assert_eq!(
        cli(&["diagnose", "--input", input.to_str().ok_or("input")?])?
            .status
            .code(),
        Some(3)
    );
    fs::File::create(&input)?.set_len(17 * 1024 * 1024)?;
    let result = cli(&["diagnose", "--input", input.to_str().ok_or("input")?])?;
    assert_eq!(result.status.code(), Some(3));
    assert!(String::from_utf8(result.stderr)?.contains("16 MiB"));
    Ok(())
}

#[test]
fn installed_cli_emits_both_versioned_schemas() -> TestResult {
    for kind in ["input", "report"] {
        let result = cli(&["schema", kind])?;
        assert!(result.status.success());
        let schema: serde_json::Value = serde_json::from_slice(&result.stdout)?;
        assert!(
            schema
                .get("properties")
                .and_then(|p| p.get("schema_version"))
                .is_some()
        );
    }
    Ok(())
}

#[test]
fn installed_cli_markdown_escapes_untrusted_labels() -> TestResult {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input.json");
    let mut packet: DiagnosisInput = serde_json::from_slice(&demo()?)?;
    packet.target.failure_signature = "<script>bad</script>".into();
    for receipt in &mut packet.receipts {
        if receipt.failure_signature.is_some() {
            receipt.failure_signature = Some(packet.target.failure_signature.clone());
        }
    }
    fs::write(&input, serde_json::to_vec(&packet)?)?;
    let result = cli(&[
        "diagnose",
        "--input",
        input.to_str().ok_or("input")?,
        "--format",
        "markdown",
    ])?;
    assert_eq!(result.status.code(), Some(2));
    let text = String::from_utf8(result.stdout)?;
    assert!(!text.contains("<script>"));
    assert!(text.contains("&lt;script&gt;"));
    assert!(text.contains("did not launch"));
    Ok(())
}
