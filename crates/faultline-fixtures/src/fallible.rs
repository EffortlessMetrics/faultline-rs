//! Fallible test-helper macros that prefer `?` propagation over panics.
//!
//! These exist so that tests returning `Result<(), anyhow::Error>` can
//! assert without resorting to `unwrap()`/`expect()`/`panic!()`. The
//! Effortless Metrics no-panic policy treats panic-family debt as
//! receipted exceptions; by using these macros, new tests can assert
//! without adding to that ledger.
//!
//! ## Usage
//!
//! ```ignore
//! use faultline_fixtures::{ensure, ensure_eq, require_some, require_ok};
//!
//! #[test]
//! fn parses_valid_input() -> Result<(), anyhow::Error> {
//!     let parsed = require_ok!("42".parse::<i32>(), "expected to parse 42");
//!     ensure_eq!(parsed, 42);
//!     ensure!(parsed > 0, "value must be positive (got {parsed})");
//!     let head = require_some!([1u8, 2, 3].first().copied(), "vec was empty");
//!     ensure_eq!(head, 1);
//!     Ok(())
//! }
//! ```
//!
//! Each macro short-circuits on failure by constructing an
//! `anyhow::Error` with a caller-supplied message and propagating via
//! `?`. There is no panic; the test runner sees a `Result::Err` and
//! reports the failure with the supplied message.

/// Fail the test with the supplied message if the condition is false.
#[macro_export]
macro_rules! ensure {
    ($cond:expr $(,)?) => {
        if !$cond {
            return Err(::anyhow::anyhow!(
                concat!("ensure failed: ", stringify!($cond))
            ));
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(::anyhow::anyhow!($($arg)+));
        }
    };
}

/// Fail the test if `left != right`. Mirrors `assert_eq!` but propagates
/// via `Result` instead of panicking.
#[macro_export]
macro_rules! ensure_eq {
    ($left:expr, $right:expr $(,)?) => {{
        let l = &$left;
        let r = &$right;
        if l != r {
            return Err(::anyhow::anyhow!(
                "ensure_eq failed: {} = {:?}, {} = {:?}",
                stringify!($left), l, stringify!($right), r
            ));
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let l = &$left;
        let r = &$right;
        if l != r {
            return Err(::anyhow::anyhow!($($arg)+));
        }
    }};
}

/// Unwrap a `Result`, propagating the error with caller context.
/// Replaces `result.expect("...")` in fallible tests.
#[macro_export]
macro_rules! require_ok {
    ($expr:expr $(,)?) => {
        match $expr {
            ::core::result::Result::Ok(v) => v,
            ::core::result::Result::Err(e) => {
                return Err(::anyhow::anyhow!(
                    "{}: {}",
                    stringify!($expr),
                    e
                ));
            }
        }
    };
    ($expr:expr, $($arg:tt)+) => {
        match $expr {
            ::core::result::Result::Ok(v) => v,
            ::core::result::Result::Err(e) => {
                return Err(::anyhow::anyhow!($($arg)+).context(e.to_string()));
            }
        }
    };
}

/// Unwrap an `Option`, returning an error with caller context.
/// Replaces `option.expect("...")` in fallible tests.
#[macro_export]
macro_rules! require_some {
    ($expr:expr $(,)?) => {
        match $expr {
            ::core::option::Option::Some(v) => v,
            ::core::option::Option::None => {
                return Err(::anyhow::anyhow!(
                    "{}: expected Some, got None",
                    stringify!($expr)
                ));
            }
        }
    };
    ($expr:expr, $($arg:tt)+) => {
        match $expr {
            ::core::option::Option::Some(v) => v,
            ::core::option::Option::None => {
                return Err(::anyhow::anyhow!($($arg)+));
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{ensure, ensure_eq, require_ok, require_some};

    #[test]
    fn ensure_passes_on_true() -> Result<(), ::anyhow::Error> {
        ensure!(2 + 2 == 4);
        Ok(())
    }

    #[test]
    fn ensure_fails_on_false() {
        fn inner() -> Result<(), ::anyhow::Error> {
            ensure!(false, "boom");
            Ok(())
        }
        let err = inner().err().expect_or_anyhow();
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn ensure_eq_propagates_message() {
        fn inner() -> Result<(), ::anyhow::Error> {
            ensure_eq!(1, 2);
            Ok(())
        }
        let err = inner().err().expect_or_anyhow();
        assert!(err.to_string().contains("ensure_eq"));
    }

    #[test]
    fn require_ok_unwraps_ok() -> Result<(), ::anyhow::Error> {
        let v: i32 = require_ok!("42".parse());
        assert_eq!(v, 42);
        Ok(())
    }

    #[test]
    fn require_ok_propagates_err() {
        fn inner() -> Result<(), ::anyhow::Error> {
            let _: i32 = require_ok!("x".parse());
            Ok(())
        }
        let err = inner().err().expect_or_anyhow();
        assert!(err.to_string().contains("parse"));
    }

    #[test]
    fn require_some_unwraps_some() -> Result<(), ::anyhow::Error> {
        let v: i32 = require_some!(Some(7));
        assert_eq!(v, 7);
        Ok(())
    }

    #[test]
    fn require_some_propagates_none() {
        fn inner() -> Result<(), ::anyhow::Error> {
            let _: i32 = require_some!(None::<i32>, "absent");
            Ok(())
        }
        let err = inner().err().expect_or_anyhow();
        assert!(err.to_string().contains("absent"));
    }

    /// Tiny helper so the tests above don't need `.expect("...")` on
    /// `Option<anyhow::Error>` — the no-panic gate would receipt that
    /// otherwise. `assert!` is acceptable as a test oracle.
    trait OptionAnyhowExt {
        fn expect_or_anyhow(self) -> ::anyhow::Error;
    }
    impl OptionAnyhowExt for Option<::anyhow::Error> {
        fn expect_or_anyhow(self) -> ::anyhow::Error {
            match self {
                Some(e) => e,
                None => ::anyhow::anyhow!("inner() unexpectedly returned Ok"),
            }
        }
    }
}
