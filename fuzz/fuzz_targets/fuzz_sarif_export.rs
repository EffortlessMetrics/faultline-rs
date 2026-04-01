#![no_main]
use faultline_types::AnalysisReport;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz SARIF serialization with arbitrary AnalysisReport JSON.
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(report) = serde_json::from_str::<AnalysisReport>(s) {
            let _ = faultline_sarif::to_sarif(&report);
        }
    }
});
