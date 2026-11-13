use faultline_types::{FaultlineError, ProbeObservation, Result, RunHandle};
use std::fs::{self, OpenOptions};
use std::io::Write;

fn directory(run: &RunHandle) -> Result<std::path::PathBuf> {
    let path = run.root.join("attempts");
    match fs::symlink_metadata(&path) {
        Ok(meta) if !meta.file_type().is_dir() => {
            return Err(FaultlineError::Store(
                "attempt journal is not an owned directory".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}

pub(super) fn load(run: &RunHandle) -> Result<Vec<ProbeObservation>> {
    let root = directory(run)?;
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut attempts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(FaultlineError::Store(
                "non-regular attempt journal entry".into(),
            ));
        }
        let attempt: ProbeObservation = serde_json::from_slice(&fs::read(entry.path())?)?;
        let expected = format!("{:020}.json", attempt.sequence_index);
        if entry.file_name() != std::ffi::OsStr::new(&expected) || attempt.flake_signal.is_some() {
            return Err(FaultlineError::Store(
                "attempt journal identity or raw-evidence mismatch".into(),
            ));
        }
        attempts.push(attempt);
    }
    attempts.sort_by_key(|a| a.sequence_index);
    Ok(attempts)
}

pub(super) fn save(run: &RunHandle, attempt: &ProbeObservation) -> Result<()> {
    if attempt.flake_signal.is_some() {
        return Err(FaultlineError::Store(
            "cannot journal an aggregate as a raw attempt".into(),
        ));
    }
    let root = directory(run)?;
    fs::create_dir_all(&root)?;
    directory(run)?;
    let path = root.join(format!("{:020}.json", attempt.sequence_index));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec_pretty(attempt)?)?;
            file.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&path)?.file_type().is_file() {
                return Err(FaultlineError::Store("non-regular attempt record".into()));
            }
            let existing: ProbeObservation = serde_json::from_slice(&fs::read(&path)?)?;
            if existing == *attempt {
                Ok(())
            } else {
                Err(FaultlineError::Store(
                    "immutable attempt identity already has different evidence".into(),
                ))
            }
        }
        Err(error) => Err(error.into()),
    }
}
