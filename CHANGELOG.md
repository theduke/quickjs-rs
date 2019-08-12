# quick-js - Changelog

## v0.2.1 - 2018-08-13

* Impelemented deserializiation of objects to `JsValue::Object`
* Added `chrono` integration via the `chrono` feature
  Adds a `JsValue::Date(DateTime<Utc>)` variant that allows (de)serializing
  a JS `Date`
* Implemented resolving promises in `eval`/`call_function`
* Added `patched` feature for applying quickjs fixes
* quickjs upgraded to `2019-08-10` release

## v0.2.0 - 2019-07-31

* Added `memory_limit` customization
* Added `Context::clear` method for resetting context
* Callbacks now support more function signatures
    ( up to 5 arguments, `Result<T, E>` return value)
* Updated embedded quickjs bindings to version 2019-07-28.
* Fixed a bug in callback logic

