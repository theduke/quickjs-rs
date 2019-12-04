use std::{convert::TryFrom, marker::PhantomData, panic::RefUnwindSafe};
use std::os::raw::c_int;
use std::ffi::c_void;

use crate::bindings::{ContextWrapper, TAG_EXCEPTION, TAG_NULL, TAG_OBJECT};
use crate::value::{JsValue, ValueError};
use crate::ExecutionError;
use crate::marshal::{deserialize_value, serialize_value};

use libquickjs_sys as q;

pub trait IntoCallbackResult {
    fn into_callback_res(self) -> Result<JsValue, String>;
}

impl<T: Into<JsValue>> IntoCallbackResult for T {
    fn into_callback_res(self) -> Result<JsValue, String> {
        Ok(self.into())
    }
}

impl<T: Into<JsValue>, E: std::fmt::Display> IntoCallbackResult for Result<T, E> {
    fn into_callback_res(self) -> Result<JsValue, String> {
        match self {
            Ok(v) => Ok(v.into()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The Callback trait is implemented for functions/closures that can be
/// used as callbacks in the JS runtime.
pub trait Callback<F>: RefUnwindSafe {
    /// The number of JS arguments required.
    fn argument_count(&self) -> usize;
    /// Execute the callback.
    ///
    /// Should return:
    ///   - Err(_) if the JS values could not be converted
    ///   - Ok(Err(_)) if an error occurred while processing.
    ///       The given error will be raised as a JS exception.
    ///   - Ok(Ok(result)) when execution succeeded.
    fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError>;
}

macro_rules! impl_callback {
    (@call $len:literal $self:ident $args:ident ) => {
        $self()
    };

    (@call $len:literal $self:ident $args:ident $( $arg:ident ),* ) => {
        {
            let mut iter = $args.into_iter();
            $self(
                $(
                    $arg::try_from(iter.next().unwrap())?,
                )*
            )
        }
    };

    [ $(  $len:literal : ( $( $arg:ident, )* ), )* ] => {
        $(

            impl<
                $( $arg, )*
                R,
                F,
            > Callback<PhantomData<(
                $( &$arg, )*
                &R,
                &F,
            )>> for F
            where
                $( $arg: TryFrom<JsValue, Error = ValueError>, )*
                R: IntoCallbackResult,
                F: Fn( $( $arg, )*  ) -> R + Sized + RefUnwindSafe,
            {
                fn argument_count(&self) -> usize {
                    $len
                }

                fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
                    if args.len() != $len {
                        return Ok(Err(format!(
                            "Invalid argument count: Expected {}, got {}",
                            self.argument_count(),
                            args.len()
                        )));
                    }

                    let res = impl_callback!(@call $len self args $($arg),* );
                    Ok(res.into_callback_res())
                }
            }
        )*
    };
}

impl_callback![
    0: (),
    1: (A1,),
    2: (A1, A2,),
    3: (A1, A2, A3,),
    4: (A1, A2, A3, A4,),
    5: (A1, A2, A3, A4, A5,),
];

/// A wrapper around Vec<JsValue>, used for vararg callbacks.
///
/// To create a callback with a variable number of arguments, a callback closure
/// must take a single `Arguments` argument.
pub struct Arguments(Vec<JsValue>);

impl Arguments {
    /// Unpack the arguments into a Vec.
    pub fn into_vec(self) -> Vec<JsValue> {
        self.0
    }
}

impl<F> Callback<PhantomData<(&Arguments, &F)>> for F
where
    F: Fn(Arguments) + Sized + RefUnwindSafe,
{
    fn argument_count(&self) -> usize {
        0
    }

    fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
        (self)(Arguments(args));
        Ok(Ok(JsValue::Null))
    }
}

impl<F, R> Callback<PhantomData<(&Arguments, &F, &R)>> for F
where
    R: IntoCallbackResult,
    F: Fn(Arguments) -> R + Sized + RefUnwindSafe,
{
    fn argument_count(&self) -> usize {
        0
    }

    fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
        let res = (self)(Arguments(args));
        Ok(res.into_callback_res())
    }
}

impl ContextWrapper {
    /// Helper for executing a callback closure.
    fn exec_callback<F>(
        context: *mut q::JSContext,
        argc: c_int,
        argv: *mut q::JSValue,
        callback: &impl Callback<F>,
    ) -> Result<q::JSValue, ExecutionError> {
        let result = std::panic::catch_unwind(|| {
            let arg_slice = unsafe { std::slice::from_raw_parts(argv, argc as usize) };

            let args = arg_slice
                .iter()
                .map(|raw| deserialize_value(context, raw))
                .collect::<Result<Vec<_>, _>>()?;

            match callback.call(args) {
                Ok(Ok(result)) => {
                    let serialized = serialize_value(context, result)?;
                    Ok(serialized)
                }
                // TODO: better error reporting.
                Ok(Err(e)) => Err(ExecutionError::Exception(JsValue::String(e))),
                Err(e) => Err(e.into()),
            }
        });

        match result {
            Ok(r) => r,
            Err(_e) => Err(ExecutionError::Internal("Callback panicked!".to_string())),
        }
    }

    /// Add a global JS function that is backed by a Rust function or closure.
    pub fn create_callback<F>(
        &self,
        callback: impl Callback<F> + 'static,
    ) -> Result<q::JSValue, ExecutionError> {
        let argcount = callback.argument_count() as i32;

        let context = self.context;
        let wrapper = move |argc: c_int, argv: *mut q::JSValue| -> q::JSValue {
            match Self::exec_callback(context, argc, argv, &callback) {
                Ok(value) => value,
                // TODO: better error reporting.
                Err(e) => {
                    let js_exception_value = match e {
                        ExecutionError::Exception(e) => e,
                        other => other.to_string().into(),
                    };
                    let js_exception = serialize_value(context, js_exception_value).unwrap();
                    unsafe {
                        q::JS_Throw(context, js_exception);
                    }

                    q::JSValue {
                        u: q::JSValueUnion { int32: 0 },
                        tag: TAG_EXCEPTION,
                    }
                }
            }
        };

        let (pair, trampoline) = unsafe { build_closure_trampoline(wrapper) };
        let data = (&*pair.1) as *const q::JSValue as *mut q::JSValue;
        self.callbacks.lock().unwrap().push(pair);

        let cfunc =
            unsafe { q::JS_NewCFunctionData(self.context, trampoline, argcount, 0, 1, data) };
        if cfunc.tag != TAG_OBJECT {
            return Err(ExecutionError::Internal("Could not create callback".into()));
        }

        Ok(cfunc)
    }

    pub fn add_callback<'a, F>(
        &'a self,
        name: &str,
        callback: impl Callback<F> + 'static,
    ) -> Result<(), ExecutionError> {
        let cfunc = self.create_callback(callback)?;
        let global = self.global()?;
        unsafe {
            global.set_property_raw(name, cfunc)?;
        }
        Ok(())
    }
}

pub type WrappedCallback = dyn Fn(c_int, *mut q::JSValue) -> q::JSValue;

/// Taken from: https://s3.amazonaws.com/temp.michaelfbryan.com/callbacks/index.html
///
/// Create a C wrapper function for a Rust closure to enable using it as a
/// callback function in the Quickjs runtime.
///
/// Both the boxed closure and the boxed data are returned and must be stored
/// by the caller to guarantee they stay alive.
unsafe fn build_closure_trampoline<F>(
    closure: F,
) -> ((Box<WrappedCallback>, Box<q::JSValue>), q::JSCFunctionData)
where
    F: Fn(c_int, *mut q::JSValue) -> q::JSValue + 'static,
{
    unsafe extern "C" fn trampoline<F>(
        _ctx: *mut q::JSContext,
        _this: q::JSValue,
        argc: c_int,
        argv: *mut q::JSValue,
        _magic: c_int,
        data: *mut q::JSValue,
    ) -> q::JSValue
    where
        F: Fn(c_int, *mut q::JSValue) -> q::JSValue,
    {
        let closure_ptr = (*data).u.ptr;
        let closure: &mut F = &mut *(closure_ptr as *mut F);
        (*closure)(argc, argv)
    }

    let boxed_f = Box::new(closure);

    let data = Box::new(q::JSValue {
        u: q::JSValueUnion {
            ptr: (&*boxed_f) as *const F as *mut c_void,
        },
        tag: TAG_NULL,
    });

    ((boxed_f, data), Some(trampoline::<F>))
}
