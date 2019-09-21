# libquickjs_sys - Changelog

## v0.4.0 - ??

Upgraded to version `2019-09-18`.

* Added `JS_ValueToAtom`
* ...

## v0.3.0 - 2019-08-13

* Added `patched` feature for applying patches
* Added patch stack-overflow-signed to fix stackoverflow due invalid cast

* c_int changed to usize in JS_NewAtomLen/JS_NewStringLen
* JS_ToCStringLen2 replaces JS_ToCStringLen 
* Added JS_GetOwnProperty(Names) functions

## v0.2.0 - 2019-07-31

* Updated embedded bindings to version 2019-07-28
    - `JS_EVAL_FLAG_SHEBANG` constant was removed
    - `JS_NewPromiseCallback` was added
