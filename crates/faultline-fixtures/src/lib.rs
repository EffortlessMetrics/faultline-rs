use faultline_types::{CommitId, RevisionSequence};

#[derive(Debug, Default, Clone)]
pub struct RevisionSequenceBuilder {
    revisions: Vec<CommitId>,
}

impl RevisionSequenceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, revision: impl Into<String>) -> Self {
        self.revisions.push(CommitId(revision.into()));
        self
    }

    pub fn build(self) -> RevisionSequence {
        RevisionSequence {
            revisions: self.revisions,
        }
    }

    /// Creates a sequence of `n` commits labeled "commit-0" through "commit-{n-1}".
    pub fn exact_boundary(n: usize) -> RevisionSequence {
        let revisions = (0..n).map(|i| CommitId(format!("commit-{}", i))).collect();
        RevisionSequence { revisions }
    }

    /// Creates a sequence from a list of labels.
    pub fn with_labels(labels: &[&str]) -> RevisionSequence {
        let revisions = labels.iter().map(|l| CommitId(l.to_string())).collect();
        RevisionSequence { revisions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn push_and_build_produces_correct_sequence() {
        let seq = RevisionSequenceBuilder::new()
            .push("aaa")
            .push("bbb")
            .push("ccc")
            .build();

        assert_eq!(seq.revisions.len(), 3);
        assert_eq!(seq.revisions[0], CommitId("aaa".into()));
        assert_eq!(seq.revisions[1], CommitId("bbb".into()));
        assert_eq!(seq.revisions[2], CommitId("ccc".into()));
    }

    #[test]
    fn exact_boundary_produces_n_commits() {
        let seq = RevisionSequenceBuilder::exact_boundary(5);
        assert_eq!(seq.revisions.len(), 5);
        for i in 0..5 {
            assert_eq!(seq.revisions[i], CommitId(format!("commit-{}", i)));
        }
    }

    #[test]
    fn exact_boundary_zero_produces_empty_sequence() {
        let seq = RevisionSequenceBuilder::exact_boundary(0);
        assert!(seq.revisions.is_empty());
    }

    #[test]
    fn with_labels_produces_correct_sequence() {
        let seq = RevisionSequenceBuilder::with_labels(&["good", "mid", "bad"]);
        assert_eq!(seq.revisions.len(), 3);
        assert_eq!(seq.revisions[0], CommitId("good".into()));
        assert_eq!(seq.revisions[1], CommitId("mid".into()));
        assert_eq!(seq.revisions[2], CommitId("bad".into()));
    }

    #[test]
    fn with_labels_empty_produces_empty_sequence() {
        let seq = RevisionSequenceBuilder::with_labels(&[]);
        assert!(seq.revisions.is_empty());
    }

    #[test]
    fn build_with_fewer_than_two_commits_still_works() {
        let empty = RevisionSequenceBuilder::new().build();
        assert!(empty.revisions.is_empty());

        let single = RevisionSequenceBuilder::new().push("only-one").build();
        assert_eq!(single.revisions.len(), 1);
        assert_eq!(single.revisions[0], CommitId("only-one".into()));
    }

    // Feature: v01-release-train, Property 3: Revision Sequence Boundary Invariant
    // **Validates: Requirements 1.4, 1.5**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_revision_sequence_boundary_invariant(
            good in "[a-f0-9]{8,40}",
            bad in "[a-f0-9]{8,40}",
            intermediates in prop::collection::vec("[a-f0-9]{8,40}", 0..20),
        ) {
            let mut builder = RevisionSequenceBuilder::new().push(good.clone());
            for mid in &intermediates {
                builder = builder.push(mid.clone());
            }
            builder = builder.push(bad.clone());
            let seq = builder.build();

            // First element is the good commit
            prop_assert_eq!(&seq.revisions.first().unwrap().0, &good,
                "first element must be the good commit");
            // Last element is the bad commit
            prop_assert_eq!(&seq.revisions.last().unwrap().0, &bad,
                "last element must be the bad commit");
            // Length >= 2 (at minimum good + bad)
            prop_assert!(seq.revisions.len() >= 2,
                "sequence must contain at least 2 elements, got {}", seq.revisions.len());
        }
    }
}
