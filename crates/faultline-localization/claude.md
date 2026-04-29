# faultline-localization

Domain crate for monotonic narrowing and outcome determination.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve pass/fail boundary selection and candidate selection behavior.
- Keep `LocalizationOutcome` transitions deterministic and explainable.
- Maintain property-test stability and confidence score rules.

Validation:
- `cargo test -p faultline-localization`
- Ensure at least the domain property suite runs when editing decision logic.

