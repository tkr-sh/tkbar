//! Logging is deliberately just `eprintln!` — no `log`/`tracing` crate.
//!
//! A status bar emits a handful of lines, almost all at startup; a logging
//! framework would add dependencies and setup for no real benefit. We aim for a
//! minimal dependency tree, since every crate is code running with your full
//! user privileges. See the README security notes.

pub(crate) fn warn(component: &str, message: &str) {
    eprintln!("tkbar: warning: {component}: {message}");
}

pub(crate) fn error(component: &str, message: &str) {
    eprintln!("tkbar: error: {component}: {message}");
}
