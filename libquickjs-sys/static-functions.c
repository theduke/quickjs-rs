#include "embed/quickjs/quickjs.h"

// these are static inline functions in quickjs.h so bindgen does not pick them up
// we impl them here as real functions so they can be added to lib.rs

void JS_FreeValue_real(JSContext *ctx, JSValue v) {
    JS_FreeValue(ctx, v);
}

void JS_DupValue_real(JSContext *ctx, JSValue v) {
    JS_DupValue(ctx, v);
}

JSValue JS_NewFloat64_real(JSContext *ctx, double d) {
    return JS_NewFloat64(ctx, d);
}

JSValue JS_NewInt32_real(JSContext *ctx, int32_t val) {
    return JS_NewInt32(ctx, val);
}

JSValue JS_NewBool_real(JSContext *ctx, JS_BOOL val) {
    return JS_NewBool(ctx, val) ;
}