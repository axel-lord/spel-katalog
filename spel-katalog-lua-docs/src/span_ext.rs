use ::std::borrow::Cow;

use ::iced::{
    Color, Font,
    font::{Style, Weight},
    widget::text::Span,
};

pub trait SpanExt<S>: Sized {
    type Quoted;
    type QuoteBy;

    fn into_span(self) -> S;
    fn doc(self) -> S;
    fn name(self) -> S;
    fn ty(self) -> S;
    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted;
    fn dquoted(self, q: impl Into<Self::QuoteBy> + Clone) -> Self::Quoted {
        self.quoted(q.clone(), q)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for Span<'a, L, F> {
    type Quoted = [Span<'a, L, F>; 3];
    type QuoteBy = Cow<'a, str>;

    fn into_span(self) -> Span<'a, L, F> {
        self
    }

    fn doc(self) -> Span<'a, L, F> {
        const DOC_CLR: Color = Color::from_rgb(1.0, 1.0, 0.7);
        const FONT: Font = Font {
            style: Style::Italic,
            ..Font::DEFAULT
        };
        self.color(DOC_CLR).font(FONT)
    }

    fn name(self) -> Span<'a, L, F> {
        const NM_CLR: Color = Color::from_rgb(0.5, 0.7, 1.0);
        const FONT: Font = Font {
            weight: Weight::Bold,
            ..Font::DEFAULT
        };
        self.color(NM_CLR).font(FONT)
    }

    fn ty(self) -> Span<'a, L, F> {
        const TY_CLR: Color = Color::from_rgb(0.5, 1.0, 0.5);
        self.color(TY_CLR)
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

    fn into_span(self) -> [Span<'a, L, F>; N] {
        self.map(SpanExt::into_span)
    }

    fn doc(self) -> [Span<'a, L, F>; N] {
        self.map(SpanExt::doc)
    }

    fn name(self) -> [Span<'a, L, F>; N] {
        self.map(SpanExt::name)
    }

    fn ty(self) -> [Span<'a, L, F>; N] {
        self.map(SpanExt::ty)
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        (Span::new(l.into()), self.into_span(), Span::new(r.into()))
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for &'a str {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    fn doc(self) -> Span<'a, L, F> {
        self.into_span().doc()
    }

    fn name(self) -> Span<'a, L, F> {
        self.into_span().name()
    }

    fn ty(self) -> Span<'a, L, F> {
        self.into_span().ty()
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for String {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    fn doc(self) -> Span<'a, L, F> {
        self.into_span().doc()
    }

    fn name(self) -> Span<'a, L, F> {
        self.into_span().name()
    }

    fn ty(self) -> Span<'a, L, F> {
        self.into_span().ty()
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}

impl<'a, L, F: From<Font>> SpanExt<Span<'a, L, F>> for Cow<'a, str> {
    type Quoted = [Span<'a, L, F>; 3];

    type QuoteBy = Cow<'a, str>;

    fn into_span(self) -> Span<'a, L, F> {
        Span::new(self)
    }

    fn doc(self) -> Span<'a, L, F> {
        self.into_span().doc()
    }

    fn name(self) -> Span<'a, L, F> {
        self.into_span().name()
    }

    fn ty(self) -> Span<'a, L, F> {
        self.into_span().ty()
    }

    fn quoted(self, l: impl Into<Self::QuoteBy>, r: impl Into<Self::QuoteBy>) -> Self::Quoted {
        self.into_span().quoted(l, r)
    }
}
