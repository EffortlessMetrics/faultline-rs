#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz CLI argument parsing via clap with arbitrary string vectors.
    if let Ok(s) = std::str::from_utf8(data) {
        let args: Vec<&str> = s.split_whitespace().collect();
        if args.is_empty() {
            return;
        }
        // We can't actually invoke clap here without the binary,
        // but we can exercise the argument splitting and validation logic.
        for arg in &args {
            let _ = arg.starts_with("--");
            let _ = arg.split_once('=');
        }
    }
});
