#![no_main]
use faultline_types::AnalysisReport;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Attempt to deserialize arbitrary strings as AnalysisReport.
        // We don't care about the result — we're looking for panics or
        // undefined behavior in the deserialization path.
        let _ = serde_json::from_str::<AnalysisReport>(s);
    }
});
