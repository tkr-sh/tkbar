pub(crate) fn warn(component: &str, message: &str) {
    eprintln!("tkbar: warning: {component}: {message}");
}

pub(crate) fn error(component: &str, message: &str) {
    eprintln!("tkbar: error: {component}: {message}");
}
