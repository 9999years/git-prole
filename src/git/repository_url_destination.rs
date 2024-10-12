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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_repository_url_destination() {
        assert_eq!(repository_url_destination("puppy/doggy"), "doggy");

        assert_eq!(repository_url_destination("puppy/doggy.git"), "doggy");
        assert_eq!(repository_url_destination("silly/puppy/doggy.git"), "doggy");
        assert_eq!(
            repository_url_destination("git@github.com:silly/doggy.git"),
            "doggy"
        );
        assert_eq!(
            repository_url_destination("git@github.com/silly/doggy.git"),
            "doggy"
        );
        assert_eq!(
            repository_url_destination("https://github.com/silly/doggy.git"),
            "doggy"
        );
    }
}
