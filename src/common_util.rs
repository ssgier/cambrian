pub fn format_path(path: &[&str]) -> String {
    if path.is_empty() {
        "(root)".to_string()
    } else {
        path.join(".")
    }
}

