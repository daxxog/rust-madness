//! The `Check.java` mirror: a flat, id-carrying counter whose report line
//! is byte-compatible with the Java gate's, so the two suites diff
//! mechanically — the Phase B parity bar (notes/PHASE-B.md).
//!
//! Design contract, carried from Check.java:
//! - one pass of the suite; an explicit runner lists every class in order;
//! - every assertion carries a stable id (group + id) the Java suite maps
//!   1:1 by name, so a green mirror is a direct faithfulness proof against
//!   the original;
//! - failures are RECORDED, not thrown — a run reports everything, like
//!   the Java gate.

use std::cell::RefCell;
use std::fmt::Display;

struct CheckState {
    group: String,
    passed: u32,
    failed: u32,
    failures: Vec<String>,
}

thread_local! {
    static STATE: RefCell<CheckState> = RefCell::new(CheckState {
        group: "?".to_string(),
        passed: 0,
        failed: 0,
        failures: Vec::new(),
    });
}

/// Sets the group name prefixed onto every assertion id (`Check.group`).
pub fn group(g: &str) {
    STATE.with_borrow_mut(|s| s.group = g.to_string());
}

/// The mechanical id audit line (`NFM_PIN_DUMP`), shared by the pass and
/// throw paths so a `throws_it` pin dumps like every other.
fn audit(id: &str) {
    if std::env::var_os("NFM_PIN_DUMP").is_some() {
        STATE.with_borrow(|s| eprintln!("PIN {}.{}", s.group, id));
    }
}

fn record(got: String, want: String, id: &str) {
    // Mechanical id audit (`NFM_PIN_DUMP=1`): every recorded pin in
    // execution order, the Java `PIN <id>` probe's twin.
    audit(id);
    STATE.with_borrow_mut(|s| {
        if got == want {
            s.passed += 1;
        } else {
            s.failed += 1;
            s.failures
                .push(format!("{}.{} got <{}> want <{}>", s.group, id, got, want));
        }
    });
}

/// `Check.eq(Object, Object, id)` for anything with the Java toString
/// shape (strings, booleans).
pub fn eq_str(got: impl Display, want: impl Display, id: &str) {
    record(format!("{got}"), format!("{want}"), id);
}

/// `Check.eq(int, int, id)` — also the `Float.floatToIntBits` /
/// `Double.doubleToLongBits` comparison shape (the caller passes bits).
pub fn eq_i32(got: i32, want: i32, id: &str) {
    record(got.to_string(), want.to_string(), id);
}

/// `Check.eq(long, long, id)`.
pub fn eq_i64(got: i64, want: i64, id: &str) {
    record(got.to_string(), want.to_string(), id);
}

/// `Check.eq(Boolean, Boolean, id)`.
pub fn eq_bool(got: bool, want: bool, id: &str) {
    record(got.to_string(), want.to_string(), id);
}

/// `Check.ok(cond, id)`.
pub fn ok(cond: bool, id: &str) {
    eq_bool(cond, true, id);
}

fn panic_message(e: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".to_string()
    }
}

/// Documents that the call panics with a message containing `want`
/// (`Check.throwsIt(task, type, id)` — Java asserts the thrown type or a
/// subclass; the Rust crash-contract mapping is message-shaped, the
/// per-pin class mapping is recorded in each batch's ledger entries).
pub fn throws_it<F: FnOnce() + std::panic::UnwindSafe>(f: F, want: &str, id: &str) {
    match std::panic::catch_unwind(f) {
        Err(e) => {
            let msg = panic_message(&e);
            if msg.contains(want) {
                audit(id);
                STATE.with_borrow_mut(|s| s.passed += 1);
            } else {
                STATE.with_borrow_mut(|s| {
                    s.failed += 1;
                    s.failures
                        .push(format!("{}.{} threw <{msg}>, wanted {want}", s.group, id));
                });
            }
        }
        Ok(()) => {
            STATE.with_borrow_mut(|s| {
                s.failed += 1;
                s.failures
                    .push(format!("{}.{} threw nothing, wanted {want}", s.group, id));
            });
        }
    }
}

/// Prints the counters and every failure; returns the exit code
/// (`Check.report`).
pub fn report() -> i32 {
    STATE.with_borrow(|s| {
        println!("== {} passed, {} failed ==", s.passed, s.failed);
        for f in &s.failures {
            println!("FAIL {f}");
        }
        if s.failed == 0 { 0 } else { 1 }
    })
}
