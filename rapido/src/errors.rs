//! Generic Error used across Rapido
use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

/// Rapido Error. Example use:
/// ```rust
///  use rapido::RapidoError;
///  RapidoError::from("this is an error");
/// ```
#[derive(Debug)]
pub struct RapidoError {
    inner: Context<String>,
}

impl Fail for RapidoError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for RapidoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<&'static str> for RapidoError {
    fn from(msg: &'static str) -> RapidoError {
        RapidoError {
            inner: Context::new(msg.into()),
        }
    }
}

impl From<String> for RapidoError {
    fn from(msg: String) -> RapidoError {
        RapidoError {
            inner: Context::new(msg),
        }
    }
}

// Allows adding more context via a String
impl From<Context<String>> for RapidoError {
    fn from(inner: Context<String>) -> RapidoError {
        RapidoError { inner }
    }
}

// Allows adding more context via a &str
impl From<Context<&'static str>> for RapidoError {
    fn from(inner: Context<&'static str>) -> RapidoError {
        RapidoError {
            inner: inner.map(|s| s.to_string()),
        }
    }
}

/// Translate from sio::Error
impl From<std::io::Error> for RapidoError {
    fn from(error: std::io::Error) -> Self {
        RapidoError::from(format!("std::io::error: {:?}", error))
    }
}

/// Translate from failure::Error
impl From<failure::Error> for RapidoError {
    fn from(error: failure::Error) -> Self {
        RapidoError::from(format!("Error: {:?}", error))
    }
}
