#![no_main]
use faultline_types::AnalysisReport;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz JUnit serialization with arbitrary AnalysisReport JSON.
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(report) = serde_json::from_str::<AnalysisReport>(s) {
            let _ = faultline_junit::to_junit_xml(&report);
        }
    }
});
