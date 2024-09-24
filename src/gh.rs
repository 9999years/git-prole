use std::sync::OnceLock;

use regex::Regex;

pub fn looks_like_gh_url(url: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?xm)
                    ^
                    [[:word:]]{1,39}(/[[:word:]]+)?
                    $
                    ",
        )
        .expect("Regex parses")
    })
    .is_match(url)
}
