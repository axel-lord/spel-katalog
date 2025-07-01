//! Trait for visiting strings of scripts.

/// Visitor for visiting strings of parsed scripts.
pub trait VisitStrings<E> {
    /// Visit a string.
    fn visit<'v>(&'v mut self, string: &mut String) -> Result<&'v mut dyn VisitStrings<E>, E>;

    /// Visit multiple of strings.
    fn visit_slice<'v>(
        &'v mut self,
        strings: &mut [String],
    ) -> Result<&'v mut dyn VisitStrings<E>, E>;

    /// Complete a chain by returning ok.
    fn finish(&self) -> Result<(), E>;
}

impl<F, E> VisitStrings<E> for F
where
    F: for<'s> FnMut(&'s mut String) -> Result<(), E>,
{
    fn visit<'v>(&'v mut self, string: &mut String) -> Result<&'v mut dyn VisitStrings<E>, E> {
        self(string)?;
        Ok(self)
    }

    fn visit_slice<'v>(
        &'v mut self,
        strings: &mut [String],
    ) -> Result<&'v mut dyn VisitStrings<E>, E> {
        for string in strings {
            self.visit(string)?;
        }
        Ok(self)
    }

    fn finish(&self) -> Result<(), E> {
        Ok(())
    }
}
