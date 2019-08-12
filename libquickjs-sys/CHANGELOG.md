# libquickjs_sys - Changelog

## v0.3.0 - 2019-08-12

* c_int changed to usize in JS_NewAtomLen/JS_NewStringLen
* JS_ToCStringLen2 replaces JS_ToCStringLen 
* Added JS_GetOwnProperty(Names) functions

## v0.2.0 - 2019-07-31

* Updated embedded bindings to version 2019-07-28
    - `JS_EVAL_FLAG_SHEBANG` constant was removed
    - `JS_NewPromiseCallback` was added
