//! Ansi escape code removal.

use ::std::io::Write;

use ::vte::{Parser, Perform};

/// Remove ansi escapes for bytes.
pub fn clean_bytes(vec: &mut Vec<u8>) {
    let Some((prior, bytes)) =
        ::memchr::memchr(b'\x1B', vec).and_then(|idx| vec.split_at_checked(idx))
    else {
        return;
    };

    let mut buf = ::smallvec::SmallVec::<[u8; 128]>::new_const();

    buf.extend_from_slice(bytes);
    vec.truncate(prior.len());

    let mut cleaner = Cleaner(Ok(vec));
    let mut parser = Parser::new();

    parser.advance(&mut cleaner, &buf);

    _ = cleaner
        .0
        .unwrap_or_else(|err| panic!("write to vec should not fail, {err}"));
}

/// Struct cleaning ansi strings.
#[derive(Debug)]
pub struct Cleaner<W: Write>(::std::io::Result<W>);

impl<W: Write> Perform for Cleaner<W> {
    fn print(&mut self, c: char) {
        if let Ok(w) = &mut self.0
            && let Err(err) = write!(w, "{c}")
        {
            self.0 = Err(err);
        }
    }
}
