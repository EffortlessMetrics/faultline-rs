use faultline_types::{CommitId, RevisionSequence};

#[derive(Debug, Default, Clone)]
pub struct RevisionSequenceBuilder {
    revisions: Vec<CommitId>,
}

impl RevisionSequenceBuilder {
    pub fn push(mut self, revision: impl Into<String>) -> Self {
        self.revisions.push(CommitId(revision.into()));
        self
    }

    pub fn build(self) -> RevisionSequence {
        RevisionSequence {
            revisions: self.revisions,
        }
    }
}
