mod common;
use common::{endpoints, packet};
use faultline_integration::{IntegrationSnapshotId, StateProvenance};
use faultline_queue::diagnose;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn planned_state_identifiers_must_be_receiptable() -> TestResult {
    for field in ["state", "commit", "tree", "snapshot"] {
        for bad in ["bad\n".to_string(), "bad\0".to_string(), "x".repeat(1025)] {
            let mut input = packet(2)?;
            endpoints(&mut input)?;
            if field == "snapshot" {
                input.sequence.snapshot_id = IntegrationSnapshotId(bad.clone());
                for state in &mut input.sequence.states {
                    state.provenance = StateProvenance::Provider {
                        provider: "synthetic".into(),
                        snapshot: IntegrationSnapshotId(bad.clone()),
                    };
                }
            } else {
                let state = input.sequence.states.get_mut(1).ok_or("midpoint")?;
                match field {
                    "state" => state.id.0 = bad,
                    "commit" => state.commit.0 = bad,
                    _ => state.tree.0 = bad,
                }
            }
            assert!(
                diagnose(&input).is_err(),
                "unreceiptable {field} must fail before planning"
            );
        }
    }
    Ok(())
}

#[test]
fn bounded_unicode_state_identity_remains_usable() -> TestResult {
    let mut input = packet(2)?;
    input.sequence.states.get_mut(1).ok_or("midpoint")?.id.0 = "état-一".into();
    endpoints(&mut input)?;
    assert_eq!(
        diagnose(&input)?.next_probe.ok_or("probe")?.state.ordinal,
        1
    );
    Ok(())
}
