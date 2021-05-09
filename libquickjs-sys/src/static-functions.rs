extern "C" {
    fn JS_DupValue_real(ctx: *mut JSContext, v: JSValue);
    fn JS_DupValueRT_real(rt: *mut JSRuntime, v: JSValue);
    fn JS_FreeValue_real(ctx: *mut JSContext, v: JSValue);
    fn JS_FreeValueRT_real(rt: *mut JSRuntime, v: JSValue);
    fn JS_NewBool_real(ctx: *mut JSContext, v: bool) -> JSValue;
    fn JS_NewInt32_real(ctx: *mut JSContext, v: i32) -> JSValue;
    fn JS_NewFloat64_real(ctx: *mut JSContext, v: f64) -> JSValue;
    fn JS_NULL_real() -> JSValue;
    fn JS_UNDEFINED_real() -> JSValue;
    fn JS_VALUE_IS_NAN_real(v: JSValue) -> bool;
    fn JS_VALUE_GET_INT_real(v: JSValue) -> ::std::os::raw::c_int;
    fn JS_VALUE_GET_PTR_real(v: JSValue) -> *mut ::std::os::raw::c_void;
    fn JS_VALUE_GET_FLOAT64_real(v: JSValue) -> f64;
    fn JS_VALUE_GET_TAG_real(v: JSValue) -> ::std::os::raw::c_int;
    fn JS_VALUE_GET_NORM_TAG_real(v: JSValue) -> ::std::os::raw::c_int;
    fn JS_IsNumber_real(v: JSValue) -> bool;
    fn JS_IsBigInt_real(ctx: *mut JSContext, v: JSValue) -> bool;
    fn JS_IsBigFloat_real(v: JSValue) -> bool;
    fn JS_IsBigDecimal_real(v: JSValue) -> bool;
    fn JS_IsBool_real(v: JSValue) -> bool;
    fn JS_IsNull_real(v: JSValue) -> bool;
    fn JS_IsUndefined_real(v: JSValue) -> bool;
    fn JS_IsException_real(v: JSValue) -> bool;
    fn JS_IsUninitialized_real(v: JSValue) -> bool;
    fn JS_IsString_real(v: JSValue) -> bool;
    fn JS_IsSymbol_real(v: JSValue) -> bool;
    fn JS_IsObject_real(v: JSValue) -> bool;
    fn JS_ToUint32_real(ctx: *mut JSContext, pres: u32, val: JSValue) -> u32;
    fn JS_SetProperty_real(
        ctx: *mut JSContext,
        this_obj: JSValue,
        prop: JSAtom,
        val: JSValue,
    ) -> ::std::os::raw::c_int;
    fn JS_NewCFunction_real(
        ctx: *mut JSContext,
        func: *mut JSCFunction,
        name: *const ::std::os::raw::c_char,
        length: ::std::os::raw::c_int,
    ) -> JSValue;
    fn JS_NewCFunctionMagic_real(
        ctx: *mut JSContext,
        func: *mut JSCFunctionMagic,
        name: *const ::std::os::raw::c_char,
        length: ::std::os::raw::c_int,
        cproto: JSCFunctionEnum,
        magic: ::std::os::raw::c_int,
    ) -> JSValue;
}

/// Increment the refcount of this value
/// # Safety
/// be safe
pub unsafe fn JS_DupValue(ctx: *mut JSContext, v: JSValue) {
    JS_DupValue_real(ctx, v);
}

/// Increment the refcount of this value
/// # Safety
/// be safe
pub unsafe fn JS_DupValueRT(rt: *mut JSRuntime, v: JSValue) {
    JS_DupValueRT_real(rt, v);
}

/// Decrement the refcount of this value
/// # Safety
/// be safe
pub unsafe fn JS_FreeValue(ctx: *mut JSContext, v: JSValue) {
    JS_FreeValue_real(ctx, v);
}

/// Decrement the refcount of this value
/// # Safety
/// be safe
pub unsafe fn JS_FreeValueRT(rt: *mut JSRuntime, v: JSValue) {
    JS_FreeValueRT_real(rt, v);
}

/// create a new boolean value
/// # Safety
/// be safe
pub unsafe fn JS_NewBool(ctx: *mut JSContext, v: bool) -> JSValue {
    JS_NewBool_real(ctx, v)
}

/// create a new int32 value
/// # Safety
/// be safe
pub unsafe fn JS_NewInt32(ctx: *mut JSContext, v: i32) -> JSValue {
    JS_NewInt32_real(ctx, v)
}

/// create a new f64 value, please note that if the passed f64 fits in a i32 this will return a value with flag 0 (i32)
/// # Safety
/// be safe
pub unsafe fn JS_NewFloat64(ctx: *mut JSContext, v: f64) -> JSValue {
    JS_NewFloat64_real(ctx, v)
}

/// Get `null` JSValue
/// # Safety
/// be safe
pub unsafe fn JS_NULL() -> JSValue {
    JS_NULL_real()
}

/// Get `undefined` JSValue
/// # Safety
/// be safe
pub unsafe fn JS_UNDEFINED() -> JSValue {
    JS_UNDEFINED_real()
}

/// check if a JSValue is a NaN value
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_IS_NAN(v: JSValue) -> bool {
    JS_VALUE_IS_NAN_real(v)
}

/// get a int value from a JSValue
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_GET_INT(v: JSValue) -> ::std::os::raw::c_int {
    JS_VALUE_GET_INT_real(v)
}

/// get a f64 value from a JSValue
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_GET_FLOAT64(v: JSValue) -> f64 {
    JS_VALUE_GET_FLOAT64_real(v)
}

/// get a ptr from a JSValue
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_GET_PTR(v: JSValue) -> *mut ::std::os::raw::c_void {
    JS_VALUE_GET_PTR_real(v)
}

/// get the tag of a JSValue
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_GET_TAG(v: JSValue) -> ::std::os::raw::c_int {
    JS_VALUE_GET_TAG_real(v)
}

/// same as JS_VALUE_GET_TAG, but return JS_TAG_FLOAT64 with NaN boxing
/// # Safety
/// be safe
pub unsafe fn JS_VALUE_GET_NORM_TAG(v: JSValue) -> ::std::os::raw::c_int {
    JS_VALUE_GET_NORM_TAG_real(v)
}

/// check if a JSValue is a Number
/// # Safety
/// be safe
pub unsafe fn JS_IsNumber(v: JSValue) -> bool {
    JS_IsNumber_real(v)
}

/// check if a JSValue is a BigInt
/// # Safety
/// be safe
pub unsafe fn JS_IsBigInt(ctx: *mut JSContext, v: JSValue) -> bool {
    JS_IsBigInt_real(ctx, v)
}

/// check if a JSValue is a BigFloat
/// # Safety
/// be safe
pub unsafe fn JS_IsBigFloat(v: JSValue) -> bool {
    JS_IsBigFloat_real(v)
}

/// check if a JSValue is a BigDecimal
/// # Safety
/// be safe
pub unsafe fn JS_IsBigDecimal(v: JSValue) -> bool {
    JS_IsBigDecimal_real(v)
}

/// check if a JSValue is a Boolean
/// # Safety
/// be safe
pub unsafe fn JS_IsBool(v: JSValue) -> bool {
    JS_IsBool_real(v)
}

/// check if a JSValue is null
/// # Safety
/// be safe
pub unsafe fn JS_IsNull(v: JSValue) -> bool {
    JS_IsNull_real(v)
}

/// check if a JSValue is Undefined
/// # Safety
/// be safe
pub unsafe fn JS_IsUndefined(v: JSValue) -> bool {
    JS_IsUndefined_real(v)
}

/// check if a JSValue is an Exception
/// # Safety
/// be safe
pub unsafe fn JS_IsException(v: JSValue) -> bool {
    JS_IsException_real(v)
}

/// check if a JSValue is initialized
/// # Safety
/// be safe
pub unsafe fn JS_IsUninitialized(v: JSValue) -> bool {
    JS_IsUninitialized_real(v)
}

/// check if a JSValue is a String
/// # Safety
/// be safe
pub unsafe fn JS_IsString(v: JSValue) -> bool {
    JS_IsString_real(v)
}

/// check if a JSValue is a Symbol
/// # Safety
/// be safe
pub unsafe fn JS_IsSymbol(v: JSValue) -> bool {
    JS_IsSymbol_real(v)
}

/// check if a JSValue is an Object
/// # Safety
/// be safe
pub unsafe fn JS_IsObject(v: JSValue) -> bool {
    JS_IsObject_real(v)
}

/// get a u32 value from a JSValue
/// # Safety
/// be safe
pub unsafe fn JS_ToUint32(ctx: *mut JSContext, pres: u32, val: JSValue) -> u32 {
    JS_ToUint32_real(ctx, pres, val)
}

/// set a property of an object identified by a JSAtom
/// # Safety
/// be safe
pub unsafe fn JS_SetProperty(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: JSAtom,
    val: JSValue,
) -> ::std::os::raw::c_int {
    JS_SetProperty_real(ctx, this_obj, prop, val)
}

/// create a new Function based on a JSCFunction
/// # Safety
/// be safe
pub unsafe fn JS_NewCFunction(
    ctx: *mut JSContext,
    func: *mut JSCFunction,
    name: *const ::std::os::raw::c_char,
    length: ::std::os::raw::c_int,
) -> JSValue {
    JS_NewCFunction_real(ctx, func, name, length)
}

/// create a new Function based on a JSCFunction
/// # Safety
/// be safe
pub unsafe fn JS_NewCFunctionMagic(
    ctx: *mut JSContext,
    func: *mut JSCFunctionMagic,
    name: *const ::std::os::raw::c_char,
    length: ::std::os::raw::c_int,
    cproto: JSCFunctionEnum,
    magic: ::std::os::raw::c_int,
) -> JSValue {
    JS_NewCFunctionMagic_real(ctx, func, name, length, cproto, magic)
}
