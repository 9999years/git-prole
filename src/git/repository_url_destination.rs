/// Where will `url` be cloned to?
///
/// It's always in the current directory.
pub fn repository_url_destination(url: &str) -> &str {
    let last_component = match url.rsplit_once('/') {
        Some((_before, after)) => after,
        None => url,
    };
    last_component
        .strip_suffix(".git")
        .unwrap_or(last_component)
}
