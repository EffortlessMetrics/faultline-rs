#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz Git adapter diff output parsing with arbitrary byte strings.
    // Simulates `git diff --name-status` output.
    if let Ok(s) = std::str::from_utf8(data) {
        // Parse each line as a tab-separated status + path pair.
        for line in s.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split('\t').collect();
            let _status = parts.first().copied().unwrap_or("");
            let _path = parts.get(1).copied().unwrap_or("");
        }
    }
});
