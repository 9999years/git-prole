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
