#![no_main]
use libfuzzer_sys::fuzz_target;

/// Minimal HTML escape function matching faultline-render's logic.
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fuzz_target!(|data: &[u8]| {
    // Fuzz HTML escaping with adversarial strings.
    if let Ok(s) = std::str::from_utf8(data) {
        let escaped = escape_html(s);
        // The escaped output must not contain raw HTML-special characters.
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(!escaped.contains('\''));
    }
});
