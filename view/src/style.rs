use serde::{Deserialize, Serialize};

/// A theme color token such as `surface.raised`; only names in
/// [`crownos_config::schema::theme::COLOR_TOKENS`] can be constructed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Token(String);

impl TryFrom<String> for Token {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(&name).ok_or_else(|| format!("`{name}` is not a theme color token"))
    }
}

impl From<Token> for String {
    fn from(token: Token) -> Self {
        token.0
    }
}

impl Token {
    pub fn new(name: &str) -> Option<Self> {
        crownos_config::schema::theme::is_color_token(name).then(|| Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Length {
    Px(f32),
    Full,
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Radius {
    None,
    Sm,
    Md,
    Lg,
    Xl,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextSize {
    Xs,
    Sm,
    Base,
    Lg,
    Xl,
    X2l,
    X3l,
    X4l,
    X5l,
    X6l,
    X7l,
}

impl TextSize {
    pub const fn px(self) -> f32 {
        match self {
            Self::Xs => 12.0,
            Self::Sm => 14.0,
            Self::Base => 16.0,
            Self::Lg => 18.0,
            Self::Xl => 20.0,
            Self::X2l => 24.0,
            Self::X3l => 30.0,
            Self::X4l => 36.0,
            Self::X5l => 48.0,
            Self::X6l => 60.0,
            Self::X7l => 72.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weight {
    Light,
    Normal,
    Medium,
    Semibold,
    Bold,
}

impl Weight {
    pub const fn value(self) -> u16 {
        match self {
            Self::Light => 300,
            Self::Normal => 400,
            Self::Medium => 500,
            Self::Semibold => 600,
            Self::Bold => 700,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Justify {
    Start,
    Center,
    End,
    Between,
}

/// One parsed utility class. Spacing is already in logical pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Style {
    P(f32),
    Px(f32),
    Py(f32),
    Pt(f32),
    Pb(f32),
    Pl(f32),
    Pr(f32),
    Gap(f32),
    W(Length),
    H(Length),
    MinW(f32),
    MaxW(f32),
    Rounded(Radius),
    Bg(Token),
    TextColor(Token),
    BorderColor(Token),
    Border(f32),
    TextSize(TextSize),
    Weight(Weight),
    Mono,
    TextCenter,
    Opacity(f32),
    Items(Align),
    Justify(Justify),
    Grow,
    Truncate,
    Hidden,
}
