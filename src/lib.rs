//! quick-js is a a Rust wrapper for [QuickJS](https://bellard.org/quickjs/), a new Javascript
//! engine by Fabrice Bellard.
//!
//! It enables easy and straight-forward execution of modern Javascript from Rust.
//!
//! ## Limitations
//!
//!* JS objects can not be deserialized into Rust (JsValue::Object) due to a missing property enumeration API
//!    (will be fixed soon)
//! * Windows is not supported yet
//!
//! ## Quickstart:
//!
//! ```rust
//! use quick_js::{Context, JsValue};
//!
//! let mut context = Context::new().unwrap();
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

#![deny(warnings)]
#![deny(missing_docs)]

mod bindings;
mod callback;
mod value;

use std::{convert::TryFrom, error, fmt};

pub use callback::Callback;
pub use value::*;

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
    #[doc(hidden)]
    __NonExhaustive,
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ContextError::*;
        match self {
            RuntimeCreationFailed => write!(f, "Could not create runtime"),
            ContextCreationFailed => write!(f, "Could not create context"),
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
}

impl ContextBuilder {
    fn new() -> Self {
        Self { memory_limit: None }
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

    /// Finalize the builder and build a JS Context.
    pub fn build(self) -> Result<Context, ContextError> {
        let wrapper = bindings::ContextWrapper::new(self.memory_limit)?;
        Ok(Context::from_wrapper(wrapper))
    }
}

/// Context is a wrapper around a QuickJS Javascript context.
/// It is the primary way to interact with the runtime.
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
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let mut context = Context::new().unwrap();
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
    pub fn eval(&mut self, code: &str) -> Result<JsValue, ExecutionError> {
        let value_raw = self.wrapper.eval(code)?;
        let value = value_raw.to_value()?;
        Ok(value)
    }

    /// Evaluates Javascript code and returns the value of the final expression
    /// as a Rust type.
    ///
    /// ```rust
    /// use quick_js::{Context};
    /// let mut context = Context::new().unwrap();
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
    pub fn eval_as<R>(&mut self, code: &str) -> Result<R, ExecutionError>
    where
        R: TryFrom<JsValue>,
        R::Error: Into<ValueError>,
    {
        let value_raw = self.wrapper.eval(code)?;
        let value = value_raw.to_value()?;
        let ret = R::try_from(value).map_err(|e| e.into())?;
        Ok(ret)
    }

    /// Call a global function in the Javascript namespace.
    ///
    /// ```rust
    /// use quick_js::{Context, JsValue};
    /// let mut context = Context::new().unwrap();
    ///
    /// let res = context.call_function("encodeURIComponent", vec!["a=b"]);
    /// assert_eq!(
    ///     res,
    ///     Ok(JsValue::String("a%3Db".to_string())),
    /// );
    /// ```
    pub fn call_function(
        &mut self,
        function_name: &str,
        args: impl IntoIterator<Item = impl Into<JsValue>>,
    ) -> Result<JsValue, ExecutionError> {
        let qargs = args
            .into_iter()
            .map(|arg| self.wrapper.serialize_value(arg.into()))
            .collect::<Result<Vec<_>, _>>()?;

        let global = self.wrapper.global()?;
        let func_obj = global.property(function_name)?;

        if !func_obj.is_object() {
            return Err(ExecutionError::Internal(format!(
                "Could not find function '{}' in global scope: does not exist, or not an object",
                function_name
            )));
        }

        let value = self.wrapper.call_function(func_obj, qargs)?.to_value()?;
        Ok(value)
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
    /// let mut context = Context::new().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn test_global_properties() {
    //     let c = Context::new().unwrap();

    //     assert_eq!(
    //         c.global_property("lala"),
    //         Err(ExecutionError::Exception(
    //             "Global object does not have property 'lala'".into()
    //         ))
    //     );

    //     c.set_global_property("testprop", true).unwrap();
    //     assert_eq!(
    //         c.global_property("testprop").unwrap(),
    //         JsValue::Bool(true),
    //     );
    // }

    #[test]
    fn test_eval_pass() {
        let mut c = Context::new().unwrap();

        let cases = vec![
            ("null", Ok(JsValue::Null)),
            ("true", Ok(JsValue::Bool(true))),
            ("2 > 10", Ok(JsValue::Bool(false))),
            ("1", Ok(JsValue::Int(1))),
            ("1 + 1", Ok(JsValue::Int(2))),
            ("1.1", Ok(JsValue::Float(1.1))),
            ("2.2 * 2 + 5", Ok(JsValue::Float(9.4))),
            ("\"abc\"", Ok(JsValue::String("abc".into()))),
            (
                "[1,2]",
                Ok(JsValue::Array(vec![JsValue::Int(1), JsValue::Int(2)])),
            ),
        ];

        for (code, res) in cases.into_iter() {
            assert_eq!(c.eval(code), res,);
        }

        assert_eq!(c.eval_as::<bool>("true").unwrap(), true,);
        assert_eq!(c.eval_as::<i32>("1 + 2").unwrap(), 3,);

        let value: String = c.eval_as("var x = 44; x.toString()").unwrap();
        assert_eq!(&value, "44");
    }

    #[test]
    fn test_eval_syntax_error() {
        let mut c = Context::new().unwrap();
        assert_eq!(
            c.eval(
                r#"
                !!!!
            "#
            ),
            Err(ExecutionError::Exception(
                "SyntaxError: unexpected token in expression: \'\'".into()
            ))
        );
    }

    #[test]
    fn test_eval_exception() {
        let mut c = Context::new().unwrap();
        assert_eq!(
            c.eval(
                r#"
                function f() {
                    throw new Error("My Error");
                }
                f();
            "#
            ),
            Err(ExecutionError::Exception("Error: My Error".into(),))
        );
    }

    #[test]
    fn test_call() {
        let mut c = Context::new().unwrap();

        assert_eq!(
            c.call_function("parseInt", vec!["22"]).unwrap(),
            JsValue::Int(22),
        );

        c.eval(
            r#"
            function add(a, b) {
                return a + b;
            }
        "#,
        )
        .unwrap();
        assert_eq!(
            c.call_function("add", vec![5, 7]).unwrap(),
            JsValue::Int(12),
        );

        c.eval(
            r#"
            function sumArray(arr) {
                let sum = 0;
                for (const value of arr) {
                    sum += value;
                }
                return sum;
            }
        "#,
        )
        .unwrap();
        assert_eq!(
            c.call_function("sumArray", vec![vec![1, 2, 3]]).unwrap(),
            JsValue::Int(6),
        );

        c.eval(
            r#"
            function addObject(obj) {
                let sum = 0;
                for (const key of Object.keys(obj)) {
                    sum += obj[key];
                }
                return sum;
            }
        "#,
        )
        .unwrap();
        let mut obj = std::collections::HashMap::<String, i32>::new();
        obj.insert("a".into(), 10);
        obj.insert("b".into(), 20);
        obj.insert("c".into(), 30);
        assert_eq!(
            c.call_function("addObject", vec![obj]).unwrap(),
            JsValue::Int(60),
        );
    }

    #[test]
    fn test_call_large_string() {
        let mut c = Context::new().unwrap();
        c.eval(" function strLen(s) { return s.length; } ").unwrap();

        let s = " ".repeat(200_000);
        let v = c.call_function("strLen", vec![s]).unwrap();
        assert_eq!(v, JsValue::Int(200_000));
    }

    #[test]
    fn test_callback() {
        let mut c = Context::new().unwrap();

        c.add_callback("cb1", |flag: bool| !flag).unwrap();
        assert_eq!(c.eval("cb1(true)").unwrap(), JsValue::Bool(false),);

        c.add_callback("concat2", |a: String, b: String| format!("{}{}", a, b))
            .unwrap();
        assert_eq!(
            c.eval(r#"concat2("abc", "def")"#).unwrap(),
            JsValue::String("abcdef".into()),
        );

        c.add_callback("add2", |a: i32, b: i32| -> i32 { a + b })
            .unwrap();
        assert_eq!(c.eval("add2(5, 11)").unwrap(), JsValue::Int(16),);
    }

    #[test]
    fn test_callback_argn_variants() {
        macro_rules! callback_argn_tests {
            [
                $(
                    $len:literal : ( $( $argn:ident : $argv:literal ),* ),
                )*
            ] => {
                $(
                   {
                       // Test plain return type.
                        let name = format!("cb{}", $len);
                        let mut c = Context::new().unwrap();
                        c.add_callback(&name, | $( $argn : i32 ),*| -> i32 {
                            $( $argn + )* 0
                        }).unwrap();

                        let code = format!("{}( {} )", name, "1,".repeat($len));
                        let v = c.eval(&code).unwrap();
                        assert_eq!(v, JsValue::Int($len));

                        // Test Result<T, E> return type with OK(_) returns.
                        let name = format!("cbres{}", $len);
                        c.add_callback(&name, | $( $argn : i32 ),*| -> Result<i32, String> {
                            Ok($( $argn + )* 0)
                        }).unwrap();

                        let code = format!("{}( {} )", name, "1,".repeat($len));
                        let v = c.eval(&code).unwrap();
                        assert_eq!(v, JsValue::Int($len));

                        // Test Result<T, E> return type with Err(_) returns.
                        let name = format!("cbreserr{}", $len);
                        c.add_callback(&name, #[allow(unused_variables)] | $( $argn : i32 ),*| -> Result<i32, String> {
                            Err("error".into())
                        }).unwrap();

                        let code = format!("{}( {} )", name, "1,".repeat($len));
                        let res = c.eval(&code);
                        assert_eq!(res, Err(ExecutionError::Exception("error".into())));
                   }
                )*
            }
        }

        callback_argn_tests![
            1: (a : 1),
        ]
    }

    #[test]
    fn test_callback_invalid_argcount() {
        let mut c = Context::new().unwrap();

        c.add_callback("cb", |a: i32, b: i32| a + b).unwrap();

        assert_eq!(
            c.eval(" cb(5) "),
            Err(ExecutionError::Exception(
                "Invalid argument count: Expected 2, got 1".into()
            )),
        );
    }

    #[test]
    fn memory_limit_exceeded() {
        let mut c = Context::builder().memory_limit(100_000).build().unwrap();
        assert_eq!(
            c.eval("  'abc'.repeat(200_000) "),
            Err(ExecutionError::OutOfMemory),
        );
    }

    #[test]
    fn context_reset() {
        let mut c = Context::new().unwrap();
        c.eval(" var x = 123; ").unwrap();
        c.add_callback("myCallback", || true).unwrap();

        let mut c2 = c.reset().unwrap();

        // Check it still works.
        assert_eq!(
            c2.eval_as::<String>(" 'abc'.repeat(2) ").unwrap(),
            "abcabc".to_string(),
        );

        // Check old state is gone.
        let err_msg = c2.eval(" x ").unwrap_err().to_string();
        assert!(err_msg.contains("ReferenceError"));

        // Check callback is gone.
        let err_msg = c2.eval(" myCallback() ").unwrap_err().to_string();
        assert!(err_msg.contains("ReferenceError"));
    }

    #[inline(never)]
    fn build_context() -> Context {
        let mut ctx = Context::new().unwrap();
        let name = "cb".to_string();
        ctx.add_callback(&name, |a: String| a.repeat(2)).unwrap();

        let code = " function f(value) { return cb(value); } ".to_string();
        ctx.eval(&code).unwrap();

        ctx
    }

    #[test]
    fn moved_context() {
        let mut c = build_context();
        let v = c.call_function("f", vec!["test"]).unwrap();
        assert_eq!(v, "testtest".into());

        let v = c.eval(" f('la') ").unwrap();
        assert_eq!(v, "lala".into());
    }
}
