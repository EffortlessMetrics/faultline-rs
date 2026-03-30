use faultline_ports::RunStorePort;
use faultline_types::{AnalysisReport, AnalysisRequest, ProbeObservation, Result, RunHandle};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileRunStore {
    root: PathBuf,
}

impl FileRunStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn run_root(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id)
    }

    fn observations_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("observations.json")
    }

    fn request_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("request.json")
    }

    fn report_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("report.json")
    }

    fn read_json_or_default<T>(&self, path: &Path) -> Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if !path.exists() {
            return Ok(T::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }
}

impl RunStorePort for FileRunStore {
    fn prepare_run(&self, request: &AnalysisRequest) -> Result<RunHandle> {
        let run_id = request.fingerprint();
        let root = self.run_root(&run_id);
        let resumed = root.exists();
        fs::create_dir_all(&root)?;
        let handle = RunHandle {
            id: run_id,
            root,
            resumed,
        };
        fs::write(self.request_path(&handle), serde_json::to_string_pretty(request)?)?;
        Ok(handle)
    }

    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>> {
        self.read_json_or_default(&self.observations_path(run))
    }

    fn save_observation(&self, run: &RunHandle, observation: &ProbeObservation) -> Result<()> {
        let mut observations: Vec<ProbeObservation> = self.load_observations(run)?;
        if let Some(existing) = observations
            .iter_mut()
            .find(|item| item.commit == observation.commit)
        {
            *existing = observation.clone();
        } else {
            observations.push(observation.clone());
        }
        observations.sort_by(|a, b| a.commit.0.cmp(&b.commit.0));
        fs::write(
            self.observations_path(run),
            serde_json::to_string_pretty(&observations)?,
        )?;
        Ok(())
    }

    fn save_report(&self, run: &RunHandle, report: &AnalysisReport) -> Result<()> {
        fs::write(self.report_path(run), serde_json::to_string_pretty(report)?)?;
        Ok(())
    }
}
