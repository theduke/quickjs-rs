use std::{convert::TryFrom, marker::PhantomData, panic::RefUnwindSafe};

use crate::value::{JsValue, ValueError};

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
    ///   - Ok(Err(_)) if an error ocurred while processing.
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
        Ok(Ok(JsValue::Undefined))
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

// Implement Callback for Fn() -> R functions.
//impl<R, F> Callback<PhantomData<(&R, &F)>> for F
//where
//R: Into<JsValue>,
//F: Fn() -> R + Sized + RefUnwindSafe,
//{
//fn argument_count(&self) -> usize {
//0
//}

//fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
//if !args.is_empty() {
//return Ok(Err(format!(
//"Invalid argument count: Expected 0, got {}",
//args.len()
//)));
//}

//let res = self().into();
//Ok(Ok(res))
//}
//}

// Implement Callback for Fn(A) -> R functions.
//impl<A1, R, F> Callback<PhantomData<(&A1, &R, &F)>> for F
//where
//A1: TryFrom<JsValue, Error = ValueError>,
//R: Into<JsValue>,
//F: Fn(A1) -> R + Sized + RefUnwindSafe,
//{
//fn argument_count(&self) -> usize {
//1
//}
//fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
//if args.len() != 1 {
//return Ok(Err(format!(
//"Invalid argument count: Expected 1, got {}",
//args.len()
//)));
//}

//let arg_raw = args.into_iter().next().expect("Invalid argument count");
//let arg = A1::try_from(arg_raw)?;
//let res = self(arg).into();
//Ok(Ok(res))
//}
//}

//// Implement Callback for Fn(A1, A2) -> R functions.
//impl<A1, A2, R, F> Callback<PhantomData<(&A1, &A2, &R, &F)>> for F
//where
//A1: TryFrom<JsValue, Error = ValueError>,
//A2: TryFrom<JsValue, Error = ValueError>,
//R: Into<JsValue>,
//F: Fn(A1, A2) -> R + Sized + RefUnwindSafe,
//{
//fn argument_count(&self) -> usize {
//2
//}

//fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
//if args.len() != 2 {
//return Ok(Err(format!(
//"Invalid argument count: Expected 2, got {}",
//args.len()
//)));
//}

//let mut iter = args.into_iter();
//let arg1_raw = iter.next().expect("Invalid argument count");
//let arg1 = A1::try_from(arg1_raw)?;

//let arg2_raw = iter.next().expect("Invalid argument count");
//let arg2 = A2::try_from(arg2_raw)?;

//let res = self(arg1, arg2).into();
//Ok(Ok(res))
//}
//}

// Implement Callback for Fn(A1, A2, A3) -> R functions.
//impl<A1, A2, A3, R, F> Callback<PhantomData<(&A1, &A2, &A3, &R, &F)>> for F
//where
//A1: TryFrom<JsValue, Error = ValueError>,
//A2: TryFrom<JsValue, Error = ValueError>,
//A3: TryFrom<JsValue, Error = ValueError>,
//R: Into<JsValue>,
//F: Fn(A1, A2, A3) -> R + Sized + RefUnwindSafe,
//{
//fn argument_count(&self) -> usize {
//3
//}

//fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
//if args.len() != self.argument_count() {
//return Ok(Err(format!(
//"Invalid argument count: Expected 3, got {}",
//args.len()
//)));
//}

//let mut iter = args.into_iter();
//let arg1_raw = iter.next().expect("Invalid argument count");
//let arg1 = A1::try_from(arg1_raw)?;

//let arg2_raw = iter.next().expect("Invalid argument count");
//let arg2 = A2::try_from(arg2_raw)?;

//let arg3_raw = iter.next().expect("Invalid argument count");
//let arg3 = A3::try_from(arg3_raw)?;

//let res = self(arg1, arg2, arg3).into();
//Ok(Ok(res))
//}
//}

//// Implement Callback for Fn(A1, A2, A3, A4) -> R functions.
//impl<A1, A2, A3, A4, R, F> Callback<PhantomData<(&A1, &A2, &A3, &A4, &R, &F)>> for F
//where
//A1: TryFrom<JsValue, Error = ValueError>,
//A2: TryFrom<JsValue, Error = ValueError>,
//A3: TryFrom<JsValue, Error = ValueError>,
//A4: TryFrom<JsValue, Error = ValueError>,
//R: Into<JsValue>,
//F: Fn(A1, A2, A3) -> R + Sized + RefUnwindSafe,
//{
//fn argument_count(&self) -> usize {
//4
//}

//fn call(&self, args: Vec<JsValue>) -> Result<Result<JsValue, String>, ValueError> {
//if args.len() != self.argument_count() {
//return Ok(Err(format!(
//"Invalid argument count: Expected 3, got {}",
//args.len()
//)));
//}

//let mut iter = args.into_iter();
//let arg1_raw = iter.next().expect("Invalid argument count");
//let arg1 = A1::try_from(arg1_raw)?;

//let arg2_raw = iter.next().expect("Invalid argument count");
//let arg2 = A2::try_from(arg2_raw)?;

//let arg3_raw = iter.next().expect("Invalid argument count");
//let arg3 = A3::try_from(arg3_raw)?;

//let res = self(arg1, arg2, arg3).into();
//Ok(Ok(res))
//}
//}

// RESULT variants.
