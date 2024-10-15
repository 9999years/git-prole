use std::fmt::Display;

use itertools::Itertools;

/// Format an iterator of items into a bulleted list with line breaks between elements.
pub fn format_bulleted_list(items: impl IntoIterator<Item = impl Display>) -> String {
    let mut items = items.into_iter().peekable();
    if items.peek().is_none() {
        String::new()
    } else {
        // This kind of sucks.
        format!("• {}", items.join("\n• "))
    }
}

/// Like [`format_bulleted_list`], except the second and subsequent lines of multi-line items are
/// indented as well.
pub fn format_bulleted_list_multiline(items: impl IntoIterator<Item = impl Display>) -> String {
    format_bulleted_list(items.into_iter().map(|item| {
        let item = item.to_string();
        let mut lines = item.lines().peekable();
        match lines.next() {
            None => {
                // ???
                String::new()
            }
            Some(first) => {
                if lines.peek().is_none() {
                    // One line.
                    item
                } else {
                    // Two or more lines.
                    format!("{first}\n  {}", lines.join("\n  "))
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_format_bulleted_list() {
        assert_eq!(format_bulleted_list(Vec::<String>::new()), "");

        assert_eq!(format_bulleted_list(["puppy"]), "• puppy");
        assert_eq!(
            format_bulleted_list(["puppy", "doggy"]),
            indoc!(
                "
                • puppy
                • doggy"
            )
        );
    }

    #[test]
    fn test_format_bulleted_list_multiline() {
        assert_eq!(format_bulleted_list_multiline(Vec::<String>::new()), "");

        assert_eq!(format_bulleted_list_multiline(["puppy"]), "• puppy");
        assert_eq!(
            format_bulleted_list_multiline(["puppy", "doggy"]),
            indoc!(
                "
                • puppy
                • doggy"
            )
        );

        assert_eq!(
            format_bulleted_list_multiline([
                "puppy\ndoggy",
                "sammy\ngoldie",
                &format_bulleted_list_multiline(["ears", "tail", "fetch!"])
            ]),
            indoc!(
                "
                • puppy
                  doggy
                • sammy
                  goldie
                • • ears
                  • tail
                  • fetch!"
            )
        );
    }
}
