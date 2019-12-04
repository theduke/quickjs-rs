//! # JS Timer API support (setTimeout, clearTimeout)
use crate::bindings::ContextWrapper;
use crate::{Arguments, ExecutionError, JsValue};
use std::ops::DerefMut;
use crate::owned_value_ref::OwnedValueRef;

/// A js timer has an expire time, a callback function and a reference to the next timer (linked list).
pub struct JsTimer {
    /// Unix time in milliseconds
    expire: i64,
    /// A function will be taken out of this option as soon as it is about to be executed
    value: Option<OwnedValueRef>,
    /// Pointer to the next timer
    next: JsTimerRef,
}

pub type JsTimerRef = Option<Box<JsTimer>>;

impl JsTimer {
    pub(crate) fn new(func: OwnedValueRef, timeout_ms: i32) -> Self {
        JsTimer {
            expire: chrono::Utc::now().timestamp_millis() + timeout_ms as i64,
            value: Some(func),
            next: None,
        }
    }
    pub(crate) fn get_next(&mut self) -> &mut JsTimerRef {
        &mut self.next
    }
}

impl ContextWrapper {
    /// Enables the timer API setTimeout and clearTimeout.
    /// Execution of [`eval`] and [`run_bytecode`] will only finish after all timers have finished.
    /// You may forcefully quit all timers with [`cancel_all_timers`]
    pub fn enable_timer_api(&self) -> Result<(), ExecutionError> {
        let timers = self.timers.clone();
        self.add_callback("clearTimeout", move |args: Arguments| {
            let mut args = args.into_vec();
            if args.len() != 1 {
                return Err("Expect 1 arguments!".to_owned());
            }
            if let JsValue::Int(timer_id) = args.remove(0) {
                let mut timer_root = timers.lock().unwrap();
                timer_iter(
                    timer_root.deref_mut(),
                    // For each matching timer
                    |v| v.expire == timer_id as i64,
                    // Do nothing. The entry will be removed by "timer_iter"
                    |_| {},
                );
                return Ok(JsValue::Null);
            }
            return Err("First argument must be a number!".to_owned());
        })?;
        let timers = self.timers.clone();
        self.add_callback("setTimeout", move |args: Arguments| {
            let mut args = args.into_vec();
            if args.len() != 2 {
                return Err("Expect 2 arguments!".to_owned());
            }
            match (args.remove(1), args.remove(0)) {
                (JsValue::Int(timeout_ms), JsValue::OpaqueFunction(func)) => {
                    // Go to the very last item and add a new TimerRef
                    let mut timer_root = timers.lock().unwrap();
                    let mut timer = timer_root.deref_mut();
                    let timer_next = loop {
                        timer = match timer {
                            Some(v) => &mut v.next,
                            None => break timer,
                        };
                    };
                    let next_timer = JsTimer::new(func, timeout_ms);
                    // The expire time will be the id
                    let id = next_timer.expire;
                    timer_next.replace(Box::new(next_timer));

                    Ok(JsValue::Int(id as i32))
                }
                _ => Err("Arguments invalid".to_owned()),
            }
        })?;
        Ok(())
    }

    /// Stop all timers.
    pub fn cancel_all_timers(&self) {
        let mut timer = self.timers.lock().unwrap();
        // This will recursively drop() all timer values
        let _ = timer.take();
    }

    pub(crate) fn await_timers(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        let mut timer_root = self.timers.lock().unwrap();
        timer_iter(
            timer_root.deref_mut(),
            // For each expired timer
            |v| v.expire < now,
            // Call the respective callback function
            |value| {
                let result = self.call_function(value, Vec::new(), false);
                if let Err(e) = result {
                    eprintln!("{:?}", e);
                }
            },
        );

        timer_root.is_some()
    }
}

/// Timers are basically structured in a linked list.
/// Looping through a linked list in Rust requires some thoughts to satisfy the borrow checker.
/// If mutability (removing) is required, the next timer link, which is an Option<Box<JsTimer>> is taken
/// and then reassigned again for each loop iteration (next_timer_option.take() + *next_timer_option = v).
fn timer_iter<C, E>(mut next_timer_option: &mut JsTimerRef, condition: C, exec: E)
where
    C: Fn(&Box<JsTimer>) -> bool,
    E: Fn(OwnedValueRef) -> (),
{
    loop {
        match next_timer_option.take() {
            Some(mut v) if condition(&v) => {
                if let Some(value) = v.value.take() {
                    exec(value)
                }
                let v = v.next;
                *next_timer_option = v;
                next_timer_option = match next_timer_option.as_mut() {
                    Some(v) => v.get_next(),
                    None => break,
                };
            }
            Some(v) => {
                *next_timer_option = Some(v);
                // Unwrap is safe, see line above
                next_timer_option = next_timer_option.as_mut().unwrap().get_next();
            }
            None => break,
        };
    }
}
