mod lines;
mod null;

pub use lines::line_ending_or_eof;
pub use lines::rest_of_line;
pub use lines::until_newline;
pub use null::till_null;
