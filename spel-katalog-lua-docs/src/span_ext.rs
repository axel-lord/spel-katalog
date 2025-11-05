use ::std::borrow::Cow;

use ::iced::{
    Color, Font,
    font::{Style, Weight},
    widget::text::Span,
};

/// Extension trait for spans and span-adjacent types.
pub trait SpanExt<S>: Sized {
    /// Result of quote operation.
    type Quoted;

    /// What to quote by.
    type QuoteBy;

    /// Type used for links.
    type Link;

    /// Convert into span type.
    fn into_span(self) -> S;

    /// Convert into span with the given font.
    fn with_font(self, f: Font) -> S;

    /// Convert into span with the given color.
    fn with_color(self, c: Color) -> S;

    /// Convert into span with the given link
    fn with_link(self, link: impl FnMut() -> Self::Link) -> S;

    /// Convert into span styled for documentation.
    fn doc(self) -> S
    where
        S: SpanExt<S>,
    {
        const DOC_CLR: Color = Color::from_rgb(1.0, 1.0, 0.7);
        const FONT: Font = Font {
            style: Style::Italic,
            ..Font::DEFAULT
        };
        self.with_color(DOC_CLR).with_font(FONT)
    }

    /// Convert into span styled for item names.
    fn name(self) -> S
    where
        S: SpanExt<S>,
    {
        const NM_CLR: Color = Color::from_rgb(0.5, 0.7, 1.0);
        const FONT: Font = Font {
            weight: Weight::Bold,
            ..Font::DEFAULT
        };
        self.with_color(NM_CLR).with_font(FONT)
    }

    /// Convert into span styled for function names.
    fn fn_name(self) -> S
    where
        S: SpanExt<S>,
    {
        const NM_CLR: Color = Color::from_rgb(0.5, 0.7, 1.0);
        self.with_color(NM_CLR)
    }

    /// Convert into span styled for types.
    fn ty(self) -> S
    where
        S: SpanExt<S>,
    {
        const TY_CLR: Color = Color::from_rgb(0.5, 1.0, 0.5);
        self.with_color(TY_CLR)
    }

    /// Convert into span styled for parameters.
    fn param(self) -> S
    where
        S: SpanExt<S>,
    {
        const PR_CLR: Color = Color::from_rgb(0.7, 0.9, 0.9);
        const FONT: Font = Font {
            style: Style::Italic,
            ..Font::DEFAULT
        };
        self.with_color(PR_CLR).with_font(FONT)
    }

    /// Convert into span styled for parameters.
    fn class_param(self) -> S
    where
        S: SpanExt<S>,
    {
        const PR_CLR: Color = Color::from_rgb(0.8, 0.6, 1.0);
        const FONT: Font = Font {
            style: Style::Italic,
            ..Font::DEFAULT
        };
        self.with_color(PR_CLR).with_font(FONT)
    }

    /// Convert into span styled for parameters.
    fn self_param(self) -> S
    where
        S: SpanExt<S>,
    {
        const PR_CLR: Color = Color::from_rgb(1.0, 0.7, 0.9);
        const FONT: Font = Font {
            style: Style::Italic,
            ..Font::DEFAULT
        };
        self.with_color(PR_CLR).with_font(FONT)
    }

    /// Set link if provided function is Some.
    fn with_link_maybe(self, link: Option<impl FnMut() -> Self::Link>) -> S {
        if let Some(link) = link {
            self.with_link(link)
        } else {
            self.into_span()
        }
    }

    /// Quote the span.
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted;

    /// Double quote the span by cloning the quote.
    fn dquoted(self, q: impl Into<Self::QuoteBy> + Clone) -> Self::Quoted {
        self.quoted(q.clone(), q)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for Span<'a, L, F> {
    type Quoted = [Span<'a, L, F>; 3];
    type QuoteBy = Cow<'a, str>;
    type Link = L;

    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> Span<'a, L, F> {
        self.link(link())
    }

    fn into_span(self) -> Span<'a, L, F> {
        self
    }

    fn with_font(self, f: Font) -> Span<'a, L, F> {
        self.font(f)
    }

    fn with_color(self, c: Color) -> Span<'a, L, F> {
        self.color(c)
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        [Span::new(l.into()), self, Span::new(r.into())]
    }
}

impl<'a, L, F: From<Font>, T, const N: usize> SpanExt<[Span<'a, L, F>; N]> for [T; N]
where
    T: SpanExt<Span<'a, L, F>>,
{
    type Quoted = (Span<'a, L, F>, [Span<'a, L, F>; N], Span<'a, L, F>);
    type QuoteBy = Cow<'a, str>;
    type Link = L;

    #[inline]
    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> [Span<'a, L, F>; N] {
        self.map(|s| s.into_span().link(link()))
    }

    #[inline]
    fn into_span(self) -> [Span<'a, L, F>; N] {
        self.map(SpanExt::into_span)
    }

    #[inline]
    fn with_color(self, c: Color) -> [Span<'a, L, F>; N] {
        self.map(|s| s.with_color(c))
    }

    #[inline]
    fn with_font(self, f: Font) -> [Span<'a, L, F>; N] {
        self.map(|s| s.with_font(f))
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        (Span::new(l.into()), self.into_span(), Span::new(r.into()))
    }
}

impl<'a, L, F: From<Font>, T> SpanExt<Vec<Span<'a, L, F>>> for Vec<T>
where
    T: SpanExt<Span<'a, L, F>>,
{
    type Quoted = Vec<Span<'a, L, F>>;

    type QuoteBy = Cow<'a, str>;

    type Link = L;

    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> Vec<Span<'a, L, F>> {
        self.into_iter()
            .map(|s| s.into_span().link(link()))
            .collect()
    }

    fn into_span(self) -> Vec<Span<'a, L, F>> {
        self.into_iter().map(SpanExt::into_span).collect()
    }

    fn with_font(self, f: Font) -> Vec<Span<'a, L, F>> {
        self.into_iter().map(|s| s.with_font(f)).collect()
    }

    fn with_color(self, c: Color) -> Vec<Span<'a, L, F>> {
        self.into_iter().map(|s| s.with_color(c)).collect()
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        let mut spans = self.into_span();
        spans.reserve(2);
        spans.insert(0, Span::new(l.into()));
        spans.push(Span::new(r.into()));
        spans
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for &'a str {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    type Link = L;

    #[inline]
    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> Span<'a, L, F> {
        self.into_span().link(link())
    }

    #[inline]
    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    #[inline]
    fn with_color(self, c: Color) -> Span<'a, L, F> {
        self.into_span().with_color(c)
    }

    #[inline]
    fn with_font(self, f: Font) -> Span<'a, L, F> {
        self.into_span().with_font(f)
    }

    #[inline]
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for String {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    type Link = L;

    #[inline]
    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> Span<'a, L, F> {
        self.into_span().link(link())
    }

    #[inline]
    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    #[inline]
    fn with_font(self, f: Font) -> Span<'a, L, F> {
        self.into_span().with_font(f)
    }

    #[inline]
    fn with_color(self, c: Color) -> Span<'a, L, F> {
        self.into_span().with_color(c)
    }

    #[inline]
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for Cow<'a, str> {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    type Link = L;

    #[inline]
    fn with_link(self, mut link: impl FnMut() -> Self::Link) -> Span<'a, L, F> {
        self.into_span().link(link())
    }

    #[inline]
    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    #[inline]
    fn with_font(self, f: Font) -> Span<'a, L, F> {
        self.into_span().with_font(f)
    }

    #[inline]
    fn with_color(self, c: Color) -> Span<'a, L, F> {
        self.into_span().with_color(c)
    }

    #[inline]
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}

impl<T, S> SpanExt<Option<S>> for Option<T>
where
    T: SpanExt<S>,
{
    type Quoted = Option<T::Quoted>;

    type QuoteBy = fn() -> T::QuoteBy;

    type Link = T::Link;

    #[inline]
    fn with_link(self, link: impl FnMut() -> Self::Link) -> Option<S> {
        self.map(|s| s.with_link(link))
    }

    #[inline]
    fn into_span(self) -> Option<S> {
        self.map(SpanExt::into_span)
    }

    #[inline]
    fn with_font(self, f: Font) -> Option<S> {
        self.map(|s| s.with_font(f))
    }

    #[inline]
    fn with_color(self, c: Color) -> Option<S> {
        self.map(|s| s.with_color(c))
    }

    #[inline]
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.map(|t| t.quoted((l.into())(), (r.into())()))
    }
}
