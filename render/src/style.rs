//! Typed styles onto crownui's `Styled` methods.

use crownos_config::schema::theme::Radii;
use crownplugin_view::{Align, Justify, Length, Radius, Style, Token};
use geometry::{Color, TRANSPARENT};
use reactive::Signal;
use view::{Styled, bind};
use widgets::kit::Theme;

use crate::TokenSource;

const FULL_RADIUS: f32 = 9999.0;

/// Everything a style needs to resolve tokens reactively.
#[derive(Clone)]
pub struct Palette {
    pub theme: Signal<Theme>,
    pub extra: Option<TokenSource>,
    pub radii: Radii,
    pub mono: String,
}

impl Palette {
    /// A reactive color for `token`; the host's own tokens win over the kit's.
    pub fn color(&self, token: &Token, fallback: fn(&Theme) -> Color) -> view::Bind<impl FnMut(&mut reactive::Runtime) -> Color + 'static> {
        let (theme, extra, name) = (self.theme, self.extra.clone(), token.as_str().to_owned());
        bind(move |runtime| {
            extra
                .as_ref()
                .and_then(|extra| extra(runtime, &name))
                .or_else(|| theme.with(runtime, |theme| theme.token(&name)))
                .unwrap_or_else(|| theme.with(runtime, fallback))
        })
    }

    fn radius(&self, radius: Radius) -> f32 {
        match radius {
            Radius::None => 0.0,
            Radius::Sm => self.radii.sm,
            Radius::Md => self.radii.md,
            Radius::Lg => self.radii.lg,
            Radius::Xl => self.radii.xl,
            Radius::Full => FULL_RADIUS,
        }
    }
}

pub fn apply<S: Styled>(mut element: S, styles: &[Style], palette: &Palette) -> S {
    for style in styles {
        element = match style {
            Style::P(value) => element.p(*value),
            Style::Px(value) => element.px(*value),
            Style::Py(value) => element.py(*value),
            Style::Pt(value) => element.pt(*value),
            Style::Pb(value) => element.pb(*value),
            Style::Pl(value) => element.pl(*value),
            Style::Pr(value) => element.pr(*value),
            Style::Gap(value) => element.gap(*value),
            Style::W(Length::Px(value)) => element.w(*value),
            Style::W(Length::Full) => element.w_full(),
            Style::H(Length::Px(value)) => element.h(*value),
            Style::H(Length::Full) => element.h_full(),
            Style::W(Length::Fit) | Style::H(Length::Fit) => element,
            Style::MinW(value) => element.min_w(*value),
            Style::MaxW(value) => element.max_w(*value),
            Style::Rounded(radius) => element.rounded(palette.radius(*radius)),
            Style::Bg(token) => element.bg(palette.color(token, |_| TRANSPARENT)),
            Style::TextColor(token) => element.text_color(palette.color(token, |theme| theme.text.body)),
            Style::BorderColor(token) => {
                element.border_color(palette.color(token, |theme| theme.surface.border))
            }
            Style::Border(width) => element.border(*width),
            Style::TextSize(size) => element.text_size(size.px()),
            Style::Weight(weight) => element.font_weight(weight.value()),
            Style::Mono => element.font_family(palette.mono.clone()),
            Style::TextCenter => element.text_center(),
            Style::Opacity(value) => element.opacity(*value),
            Style::Items(Align::Start) => element.items_start(),
            Style::Items(Align::Center) => element.items_center(),
            Style::Items(Align::End) => element.items_end(),
            Style::Items(Align::Stretch) => element.items_stretch(),
            Style::Justify(Justify::Start) => element.justify_start(),
            Style::Justify(Justify::Center) => element.justify_center(),
            Style::Justify(Justify::End) => element.justify_end(),
            Style::Justify(Justify::Between) => element.justify_between(),
            Style::Grow => element.flex_1(),
            Style::Truncate => element.truncate(),
            Style::Hidden => element.display_none(),
        };
    }
    element
}

/// The color a `TextColor` style asks for, if any.
pub fn text_token(styles: &[Style]) -> Option<&Token> {
    styles.iter().rev().find_map(|style| match style {
        Style::TextColor(token) => Some(token),
        _ => None,
    })
}

/// The pixel size a `TextSize` style asks for, if any.
pub fn text_size(styles: &[Style]) -> Option<f32> {
    styles.iter().rev().find_map(|style| match style {
        Style::TextSize(size) => Some(size.px()),
        _ => None,
    })
}

/// A reactive color, boxed so differently built colors share one type.
pub trait BoxedProp {
    fn boxed_prop(self) -> view::Prop<Color>;
}

impl<F: FnMut(&mut reactive::Runtime) -> Color + 'static> BoxedProp for view::Bind<F> {
    fn boxed_prop(self) -> view::Prop<Color> {
        view::IntoProp::into_prop(self)
    }
}
