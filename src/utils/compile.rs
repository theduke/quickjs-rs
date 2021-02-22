//! Utils to compile script to bytecode and run script from bytecode

use crate::bindings::{ContextWrapper, OwnedValueRef};
use crate::ExecutionError;
use libquickjs_sys as q;
use std::ffi::CString;

/// compile a script, will result in a JSValueRef with tag JS_TAG_FUNCTION_BYTECODE or JS_TAG_MODULE.
///  It can be executed with run_compiled_function().
#[allow(dead_code)]
pub fn compile<'a>(
    context: &'a ContextWrapper,
    script: &str,
    file_name: &str,
) -> Result<OwnedValueRef<'a>, ExecutionError> {
    let filename_c = CString::new(file_name)
        .map_err(|_e| ExecutionError::Internal("cstring creation failed".to_string()))?;
    let code_c = CString::new(script)
        .map_err(|_e| ExecutionError::Internal("cstring creation failed".to_string()))?;

    let value_raw = unsafe {
        q::JS_Eval(
            context.context,
            code_c.as_ptr(),
            script.len() as _,
            filename_c.as_ptr(),
            q::JS_EVAL_FLAG_COMPILE_ONLY as i32,
        )
    };

    // check for error
    let ret = OwnedValueRef::new(context, value_raw);
    if ret.is_exception() {
        let ex_opt = context.get_exception();
        if let Some(ex) = ex_opt {
            Err(ex)
        } else {
            Err(ExecutionError::Internal("Unknown error".to_string()))
        }
    } else {
        Ok(ret)
    }
}

/// run a compiled function, see compile for an example
#[allow(dead_code)]
pub fn run_compiled_function<'a>(
    context: &'a ContextWrapper,
    compiled_func: &OwnedValueRef,
) -> Result<OwnedValueRef<'a>, ExecutionError> {
    assert!(compiled_func.is_compiled_function());
    let val = unsafe { q::JS_EvalFunction(context.context, *compiled_func.into_inner()) };
    let val_ref = OwnedValueRef::new(context, val);
    if val_ref.is_exception() {
        let ex_opt = context.get_exception();
        if let Some(ex) = ex_opt {
            Err(ex)
        } else {
            Err(ExecutionError::Internal(
                "run_compiled_function failed and could not get exception".to_string(),
            ))
        }
    } else {
        Ok(val_ref)
    }
}

/// write a function to bytecode
#[allow(dead_code)]
pub fn to_bytecode(context: &ContextWrapper, compiled_func: &OwnedValueRef) -> Vec<u8> {
    assert!(compiled_func.is_compiled_function());

    #[cfg(target_pointer_width = "64")]
    let mut len: u64 = 0;
    #[cfg(target_pointer_width = "32")]
    let mut len: u32 = 0;

    let slice_u8 = unsafe {
        q::JS_WriteObject(
            context.context,
            &mut len,
            *compiled_func.into_inner(),
            q::JS_WRITE_OBJ_BYTECODE as i32,
        )
    };

    let slice = unsafe { std::slice::from_raw_parts(slice_u8, len as usize) };

    slice.to_vec()
}

/// read a compiled function from bytecode, see to_bytecode for an example
#[allow(dead_code)]
pub fn from_bytecode(
    context: &ContextWrapper,
    bytecode: Vec<u8>,
) -> Result<OwnedValueRef, ExecutionError> {
    assert!(!bytecode.is_empty());
    {
        #[cfg(target_pointer_width = "64")]
        let len = bytecode.len() as u64;
        #[cfg(target_pointer_width = "32")]
        let len = bytecode.len() as u32;

        let buf = bytecode.as_ptr();
        let raw =
            unsafe { q::JS_ReadObject(context.context, buf, len, q::JS_READ_OBJ_BYTECODE as i32) };

        let func_ref = OwnedValueRef::new_dup(context, raw);
        if func_ref.is_exception() {
            let ex_opt = context.get_exception();
            if let Some(ex) = ex_opt {
                Err(ex)
            } else {
                Err(ExecutionError::Internal(
                    "from_bytecode failed and could not get exception".to_string(),
                ))
            }
        } else {
            Ok(func_ref)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::bindings::ContextWrapper;
    use crate::utils::compile::{compile, from_bytecode, run_compiled_function, to_bytecode};
    use crate::JsValue;

    #[test]
    fn test_compile() {
        let ctx = ContextWrapper::new(None).unwrap();

        let func_res = compile(
            &ctx,
            "{let a_tb3 = 7; let b_tb3 = 5; a_tb3 * b_tb3;}",
            "test_func.es",
        );
        let func = func_res.ok().expect("func compile failed");
        let bytecode: Vec<u8> = to_bytecode(&ctx, &func);
        drop(func);
        assert!(!bytecode.is_empty());
        let func2_res = from_bytecode(&ctx, bytecode);
        let func2 = func2_res.ok().expect("could not read bytecode");
        let run_res = run_compiled_function(&ctx, &func2);
        match run_res {
            Ok(res) => {
                assert_eq!(res.to_value().unwrap(), JsValue::Int(7 * 5));
            }
            Err(e) => {
                panic!("run failed1: {}", e);
            }
        }
    }

    #[test]
    fn test_bytecode() {
        let ctx = ContextWrapper::new(None).unwrap();

        let func_res = compile(
            &ctx,
            "let a_tb4 = 7; let b_tb4 = 5; a_tb4 * b_tb4;",
            "test_func.es",
        );
        let func = func_res.ok().expect("func compile failed");
        let bytecode: Vec<u8> = to_bytecode(&ctx, &func);
        drop(func);
        assert!(!bytecode.is_empty());
        let func2_res = from_bytecode(&ctx, bytecode);
        let func2 = func2_res.ok().expect("could not read bytecode");
        let run_res = run_compiled_function(&ctx, &func2);

        match run_res {
            Ok(res) => {
                assert_eq!(res.to_value().unwrap(), JsValue::Int(7 * 5));
            }
            Err(e) => {
                panic!("run failed: {}", e);
            }
        }
    }
}
