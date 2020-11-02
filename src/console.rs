//! Javascript console integration.
//! See the [ConsoleBackend] trait for more info.

use super::JsValue;

/// Log level of a log message sent via the console.
/// These levels represent the different functions defined in the spec:
/// https://s3.amazonaws.com/temp.michaelfbryan.com/callbacks/index.html
#[allow(missing_docs)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Level {
    Trace,
    Debug,
    Log,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Level::*;
        let v = match self {
            Trace => "trace",
            Debug => "debug",
            Log => "log",
            Info => "info",
            Warn => "warn",
            Error => "error",
        };
        write!(f, "{}", v)
    }
}

/// A console backend that handles console messages sent from JS via
/// console.{log,debug,trace,...} functions.
///
/// A backend has to be registered via the `ContextBuilder::console` method.
///
/// A backend that forwads to the `log` crate is available with the `log` feature.
///
/// Note that any closure of type `Fn(Level, Vec<JsValue>)` implements this trait.
///
/// A very simple logger that just prints to stderr could look like this:
///
/// ```rust
/// use quick_js::{Context, JsValue, console::Level};
///
/// Context::builder()
///     .console(|level: Level, args: Vec<JsValue>| {
///         eprintln!("{}: {:?}", level, args);
///     })
///     .build()
///     # .unwrap();
/// ```
///
pub trait ConsoleBackend: std::panic::RefUnwindSafe + 'static {
    /// Handle a log message.
    fn log(&self, level: Level, values: Vec<JsValue>);
}

impl<F> ConsoleBackend for F
where
    F: Fn(Level, Vec<JsValue>) + std::panic::RefUnwindSafe + 'static,
{
    fn log(&self, level: Level, values: Vec<JsValue>) {
        (self)(level, values);
    }
}

#[cfg(feature = "log")]
mod log {
    use super::{JsValue, Level};

    /// A console implementation that logs messages via the `log` crate.
    ///
    /// Only available with the `log` feature.
    pub struct LogConsole;

    fn print_value(value: JsValue) -> String {
        match value {
            JsValue::Undefined => "undefined".to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::Bool(v) => v.to_string(),
            JsValue::Int(v) => v.to_string(),
            JsValue::Float(v) => v.to_string(),
            JsValue::String(v) => v,
            JsValue::Array(values) => {
                let parts = values
                    .into_iter()
                    .map(print_value)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", parts)
            }
            JsValue::Object(map) => {
                let parts = map
                    .into_iter()
                    .map(|(key, value)| format!("{}: {}", key, print_value(value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", parts)
            }
            #[cfg(feature = "chrono")]
            JsValue::Date(v) => v.to_string(),
            #[cfg(feature = "bigint")]
            JsValue::BigInt(v) => v.to_string(),
            JsValue::__NonExhaustive => unreachable!(),
        }
    }

    impl super::ConsoleBackend for LogConsole {
        fn log(&self, level: Level, values: Vec<JsValue>) {
            if values.is_empty() {
                return;
            }
            let log_level = match level {
                Level::Trace => log::Level::Trace,
                Level::Debug => log::Level::Debug,
                Level::Log => log::Level::Info,
                Level::Info => log::Level::Info,
                Level::Warn => log::Level::Warn,
                Level::Error => log::Level::Error,
            };

            let msg = values
                .into_iter()
                .map(print_value)
                .collect::<Vec<_>>()
                .join(" ");

            log::log!(log_level, "{}", msg);
        }
    }
}

#[cfg(feature = "log")]
pub use self::log::LogConsole;
