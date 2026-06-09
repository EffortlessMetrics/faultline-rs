//! BDD-style tests for the ReproductionCapsule workflow.
//!
//! Each test follows the Given/When/Then pattern to verify that
//! `ReproductionCapsule::to_shell_script()` produces correct POSIX
//! shell scripts for various predicate types and edge cases.

use faultline_codes::ProbeKind;
use faultline_types::{CommitId, ProbeSpec, ReproductionCapsule, ShellKind};

// ---------------------------------------------------------------------------
// Scenario 1: Capsule from Shell predicate produces valid shell script
// ---------------------------------------------------------------------------

#[test]
fn shell_predicate_produces_valid_shell_script() {
    // Given: a ReproductionCapsule with a ProbeSpec::Shell predicate
    let capsule = ReproductionCapsule {
        commit: CommitId("a1b2c3d".into()),
        predicate: ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::PosixSh,
            script: "make test && make lint".into(),
            env: vec![],
            timeout_seconds: 120,
        },
        env: vec![],
        working_dir: "/home/dev/project".into(),
        timeout_seconds: 120,
    };

    // When: to_shell_script() is called
    let script = capsule.to_shell_script();

    // Then: output starts with a POSIX shebang
    assert!(
        script.starts_with("#!/bin/sh\n"),
        "script must begin with #!/bin/sh shebang"
    );

    // Then: output contains `set -e` for fail-fast behavior
    assert!(
        script.contains("set -e"),
        "script must contain set -e for fail-fast"
    );

    // Then: output contains the original shell script text
    assert!(
        script.contains("make test && make lint"),
        "script must contain the probe's shell command"
    );

    // Then: output contains `git checkout` with the capsule's commit
    assert!(
        script.contains("git checkout 'a1b2c3d'"),
        "script must checkout the capsule's commit"
    );

    // Then: output contains the timeout wrapper
    assert!(
        script.contains("timeout 120"),
        "script must contain the timeout value"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: Capsule from Exec predicate produces valid shell script
// ---------------------------------------------------------------------------

#[test]
fn exec_predicate_produces_valid_shell_script() {
    // Given: a ReproductionCapsule with ProbeSpec::Exec for "cargo test"
    let capsule = ReproductionCapsule {
        commit: CommitId("deadbeef".into()),
        predicate: ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![],
            timeout_seconds: 300,
        },
        env: vec![],
        working_dir: "/tmp/workspace".into(),
        timeout_seconds: 300,
    };

    // When: to_shell_script() is called
    let script = capsule.to_shell_script();

    // Then: output contains the program name "cargo"
    assert!(
        script.contains("'cargo'"),
        "script must contain the program name"
    );

    // Then: output contains the argument "test"
    assert!(
        script.contains("'test'"),
        "script must contain the program argument"
    );

    // Then: output contains `git checkout` with the commit
    assert!(
        script.contains("git checkout 'deadbeef'"),
        "script must checkout the correct commit"
    );

    // Then: output contains the timeout wrapper
    assert!(
        script.contains("timeout 300"),
        "script must wrap the command with timeout"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Environment variables are exported in shell script
// ---------------------------------------------------------------------------

#[test]
fn environment_variables_are_exported() {
    // Given: a capsule with env = [("RUST_LOG", "debug"), ("CI", "true")]
    let capsule = ReproductionCapsule {
        commit: CommitId("face0ff".into()),
        predicate: ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::Default,
            script: "true".into(),
            env: vec![],
            timeout_seconds: 10,
        },
        env: vec![
            ("RUST_LOG".into(), "debug".into()),
            ("CI".into(), "true".into()),
        ],
        working_dir: "/repo".into(),
        timeout_seconds: 10,
    };

    // When: to_shell_script() is called
    let script = capsule.to_shell_script();

    // Then: output contains export for RUST_LOG
    assert!(
        script.contains("export RUST_LOG='debug'"),
        "script must export RUST_LOG with correct value"
    );

    // Then: output contains export for CI
    assert!(
        script.contains("export CI='true'"),
        "script must export CI with correct value"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: Shell-special characters are escaped
// ---------------------------------------------------------------------------

#[test]
fn shell_special_characters_are_escaped() {
    // Given: a capsule with a script containing single quotes: echo 'hello world'
    let capsule = ReproductionCapsule {
        commit: CommitId("abc".into()),
        predicate: ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::Default,
            script: "echo 'hello world'".into(),
            env: vec![],
            timeout_seconds: 30,
        },
        env: vec![],
        working_dir: "/tmp/repo".into(),
        timeout_seconds: 30,
    };

    // When: to_shell_script() is called
    let script = capsule.to_shell_script();

    // Then: the single quotes in the script are properly escaped as '\''
    //       The full escaped form is: sh -c 'echo '\''hello world'\'''
    assert!(
        script.contains("'\\''"),
        "script must escape single quotes using the '\\'' idiom"
    );

    // Then: the original unescaped text should NOT appear as a raw
    //       single-quoted string (it must be broken up by escaping)
    assert!(
        !script.contains("'echo 'hello world''"),
        "raw single quotes must not appear unescaped inside single-quoted context"
    );
}

// ---------------------------------------------------------------------------
// Scenario 5: Empty env produces no export lines
// ---------------------------------------------------------------------------

#[test]
fn empty_env_produces_no_export_lines() {
    // Given: a capsule with env = [] and probe env = []
    let capsule = ReproductionCapsule {
        commit: CommitId("000aaa".into()),
        predicate: ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "echo".into(),
            args: vec!["ok".into()],
            env: vec![],
            timeout_seconds: 5,
        },
        env: vec![],
        working_dir: "/tmp".into(),
        timeout_seconds: 5,
    };

    // When: to_shell_script() is called
    let script = capsule.to_shell_script();

    // Then: output does not contain "export" since there are no env vars
    assert!(
        !script.contains("export"),
        "script must not contain export lines when env is empty"
    );
}
