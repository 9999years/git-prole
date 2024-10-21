/// Get the final component of a path-like value.
pub fn final_component(path_ish: &str) -> &str {
    match path_ish.rsplit_once('/') {
        Some((_left, right)) => right,
        None => path_ish,
    }
}
