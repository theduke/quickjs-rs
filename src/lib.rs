//! quick-js is a a Rust wrapper for [QuickJS](https://bellard.org/quickjs/), a new Javascript
//! engine by Fabrice Bellard.
//!
//! It enables easy and straight-forward execution of modern Javascript from Rust.
//!
//! ## Limitations
//!
//! * Building on Windows requires the `x86_64-pc-windows-gnu` toolchain
//!
//! ## Quickstart:
//!
//! ```rust
//! use quick_js::{Context, JsValue};
//!
//! let context = Context::new().unwrap();
//!
//! // Eval.
//!
//! let value = context.eval("1 + 2").unwrap();
//! assert_eq!(value, JsValue::Int(3));
//!
//! let value = context.eval_as::<String>(" var x = 100 + 250; x.toString() ").unwrap();
//! assert_eq!(&value, "350");
//!
//! // Callbacks.
//!
//! context.add_callback("myCallback", |a: i32, b: i32| a + b).unwrap();
//!
//! context.eval(r#"
//!     // x will equal 30
//!     var x = myCallback(10, 20);
//! "#).unwrap();
//! ```

#![deny(missing_docs)]

mod bindings;
mod callback;
pub mod console;
mod value;

#[cfg(test)]
mod tests;

use std::{convert::TryFrom, error, fmt};

pub use self::{
    callback::{Arguments, Callback},
    value::*,
};

/// Error on Javascript execution.
#[derive(PartialEq, Debug)]
pub enum ExecutionError {
    /// Code to be executed contained zero-bytes.
    InputWithZeroBytes,
    /// Value conversion failed. (either input arguments or result value).
    Conversion(ValueError),
    /// Internal error.
    Internal(String),
    /// JS Exception was thrown.
    Exception(JsValue),
    /// JS Runtime exceeded the memory limit.
    OutOfMemory,
    #[doc(hidden)]
    __NonExhaustive,
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ExecutionError::*;
        match self {
            InputWithZeroBytes => write!(f, "Invalid script input: code contains zero byte (\\0)"),
            Conversion(e) => e.fmt(f),
            Internal(e) => write!(f, "Internal error: {}", e),
            Exception(e) => write!(f, "{:?}", e),
            OutOfMemory => write!(f, "Out of memory: runtime memory limit exceeded"),
            __NonExhaustive => unreachable!(),
        }
    }
}

impl error::Error for ExecutionError {}

impl From<ValueError> for ExecutionError {
    fn from(v: ValueError) -> Self {
        ExecutionError::Conversion(v)
    }
}

/// Error on context creation.
#[derive(Debug)]
pub enum ContextError {
    /// Runtime could not be created.
    RuntimeCreationFailed,
    /// Context could not be created.
    ContextCreationFailed,
    /// Execution error while building.
    Execution(ExecutionError),
    #[doc(hidden)]
    __NonExhaustive,
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ContextError::*;
        match self {
            RuntimeCreationFailed => write!(f, "Could not create runtime"),
            ContextCreationFailed => write!(f, "Could not create context"),
            Execution(e) => e.fmt(f),
            __NonExhaustive => unreachable!(),
        }
    }
}

impl error::Error for ContextError {}

/// A builder for [Context](Context).
///
/// Create with [Context::builder](Context::builder).
pub struct ContextBuilder {
    memory_limit: Option<usize>,
    console_backend: Option<Box<dyn console::ConsoleBackend>>,
}

impl ContextBuilder {
    fn new() -> Self {
        Self {
            memory_limit: None,
            console_backend: None,
        }
    }

    /// Sets the memory limit of the Javascript runtime (in bytes).
    ///
    /// If the limit is exceeded, methods like `eval` will return
    /// a `Err(ExecutionError::Exception(JsValue::Null))`
    // TODO: investigate why we don't get a proper exception message here.
    pub fn memory_limit(self, max_bytes: usize) -> Self {
        let mut s = self;
        s.memory_limit = Some(max_bytes);
        s
    }

    /// Set a console handler that will proxy `console.{log,trace,debug,...}`
    /// calls.
    ///
    /// The given argument must implement the [console::ConsoleBackend] trait.
    ///
    /// A very simple logger could look like this:
    pub fn console<B>(mut self, backend: B) -> Self
    where
        B: console::ConsoleBackend,
    {
        self.console_backend = Some(Box::new(backend));
        self
    }

    /// Finalize the builder and build a JS Context.
    pub fn build(self) -> Result<Context, ContextError> {
        let wrapper = bindings::ContextWrapper::new(self.memory_limit)?;
        if let Some(be) = self.console_backend {
            wrapper.set_console(be).map_err(ContextError::Execution)?;
        }
        Ok(Context::from_wrapper(wrapper))
    }
}

/// Context is a wrapper around a QuickJS Javascript context.
/// It is the primary way to interact with the runtime.
///
/// For each `Context` instance a new instance of QuickJS
/// runtime is created. It means that it is safe to use
/// different contexts in different threads, but each
/// `Context` instance must be used only from a single thread.
pub struct Context {
    wrapper: bindings::ContextWrapper,
}

impl Context {
    fn from_wrapper(wrapper: bindings::ContextWrapper) -> Self {
        Self { wrapper }
    }

    /// Create a `ContextBuilder` that allows customization of JS Runtime settings.
    ///
    /// For details, see the methods on `ContextBuilder`.
    ///
    /// ```rust
    /// let _context = quick_js::Context::builder()
    ///     .memory_limit(100_000)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> ContextBuilder {
        ContextBuilder::new()
    }

    /// Create a new Javascript context with default settings.
    pub fn new() -> Result<Self, ContextError> {
        let wrapper = bindings::ContextWrapper::new(None)?;
        Ok(Self::from_wrapper(wrapper))
    }

    /// Reset the Javascript engine.
    ///
    /// All state and callbacks will be removed.
    pub fn reset(self) -> Result<Self, ContextError> {
        let wrapper = self.wrapper.reset()?;
        Ok(Self { wrapper })
    }

    /// Evaluates Javascript code and returns the value of the final expression.
    ///
    /// **Promises**:
    /// If the evaluated code returns a Promise, the event loop
    /// will be executed until the promise is finished. The final value of
    /// the promise will be returned, or a `ExecutionError::Exception` if the
    /// promise failed.
    ///
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let context = Context::new().unwrap();
    ///
    /// let value = context.eval(" 1 + 2 + 3 ");
    /// assert_eq!(
    ///     value,
    ///     Ok(JsValue::Int(6)),
    /// );
    ///
    /// let value = context.eval(r#"
    ///     function f() { return 55 * 3; }
    ///     let y = f();
    ///     var x = y.toString() + "!"
    ///     x
    /// "#);
    /// assert_eq!(
    ///     value,
    ///     Ok(JsValue::String("165!".to_string())),
    /// );
    /// ```
    pub fn eval(&self, code: &str) -> Result<JsValue, ExecutionError> {
        let value_raw = self.wrapper.eval(code)?;
        let value = value_raw.to_value()?;
        Ok(value)
    }

    /// Evaluates Javascript code and returns the value of the final expression
    /// as a Rust type.
    ///
    /// **Promises**:
    /// If the evaluated code returns a Promise, the event loop
    /// will be executed until the promise is finished. The final value of
    /// the promise will be returned, or a `ExecutionError::Exception` if the
    /// promise failed.
    ///
    /// ```rust
    /// use quick_js::{Context};
    /// let context = Context::new().unwrap();
    ///
    /// let res = context.eval_as::<bool>(" 100 > 10 ");
    /// assert_eq!(
    ///     res,
    ///     Ok(true),
    /// );
    ///
    /// let value: i32 = context.eval_as(" 10 + 10 ").unwrap();
    /// assert_eq!(
    ///     value,
    ///     20,
    /// );
    /// ```
    pub fn eval_as<R>(&self, code: &str) -> Result<R, ExecutionError>
    where
        R: TryFrom<JsValue>,
        R::Error: Into<ValueError>,
    {
        let value_raw = self.wrapper.eval(code)?;
        let value = value_raw.to_value()?;
        let ret = R::try_from(value).map_err(|e| e.into())?;
        Ok(ret)
    }

    /// Set a global variable.
    ///
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let context = Context::new().unwrap();
    ///
    /// context.set_global("someGlobalVariable", 42).unwrap();
    /// let value = context.eval_as::<i32>("someGlobalVariable").unwrap();
    /// assert_eq!(
    ///     value,
    ///     42,
    /// );
    /// ```
    pub fn set_global<V>(&self, name: &str, value: V) -> Result<(), ExecutionError>
    where
        V: Into<JsValue>,
    {
        let global = self.wrapper.global()?;
        let v = self.wrapper.serialize_value(value.into())?;
        global.set_property(name, v)?;
        Ok(())
    }

    /// Call a global function in the Javascript namespace.
    ///
    /// **Promises**:
    /// If the evaluated code returns a Promise, the event loop
    /// will be executed until the promise is finished. The final value of
    /// the promise will be returned, or a `ExecutionError::Exception` if the
    /// promise failed.
    ///
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let context = Context::new().unwrap();
    ///
    /// let res = context.call_function("encodeURIComponent", vec!["a=b"]);
    /// assert_eq!(
    ///     res,
    ///     Ok(JsValue::String("a%3Db".to_string())),
    /// );
    /// ```
    pub fn call_function(
        &self,
        function_name: &str,
        args: impl IntoIterator<Item = impl Into<JsValue>>,
    ) -> Result<JsValue, ExecutionError> {
        let qargs = args
            .into_iter()
            .map(|arg| self.wrapper.serialize_value(arg.into()))
            .collect::<Result<Vec<_>, _>>()?;

        let global = self.wrapper.global()?;
        let func = global
            .property_require(function_name)?
            .try_into_function()?;
        let v = self.wrapper.call_function(func, qargs)?.to_value()?;
        Ok(v)
    }

    /// Add a global JS function that is backed by a Rust function or closure.
    ///
    /// The callback must satisfy several requirements:
    /// * accepts 0 - 5 arguments
    /// * each argument must be convertible from a JsValue
    /// * must return a value
    /// * the return value must either:
    ///   - be convertible to JsValue
    ///   - be a Result<T, E> where T is convertible to JsValue
    ///     if Err(e) is returned, a Javascript exception will be raised
    ///
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let context = Context::new().unwrap();
    ///
    /// // Register a closue as a callback under the "add" name.
    /// // The 'add' function can now be called from Javascript code.
    /// context.add_callback("add", |a: i32, b: i32| { a + b }).unwrap();
    ///
    /// // Now we try out the 'add' function via eval.
    /// let output = context.eval_as::<i32>(" add( 3 , 4 ) ").unwrap();
    /// assert_eq!(
    ///     output,
    ///     7,
    /// );
    /// ```
    pub fn add_callback<F>(
        &self,
        name: &str,
        callback: impl Callback<F> + 'static,
    ) -> Result<(), ExecutionError> {
        self.wrapper.add_callback(name, callback)
    }
}
