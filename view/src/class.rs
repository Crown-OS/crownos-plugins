//! Tailwind-style class strings to typed [`Style`]s.
//!
//! Spacing uses Tailwind's scale (`p-2` = 8px) or an arbitrary pixel value
//! (`p-[10]`). Colors must be theme tokens (`bg-surface.raised`); literal
//! colors are refused so every plugin stays in the system palette.

use thiserror::Error;

use crate::{Align, Justify, Length, Radius, Style, TextSize, Token, Weight};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("class `{class}`: {reason}")]
pub struct ClassError {
    pub class: String,
    pub reason: String,
}

const SPACING_UNIT: f32 = 4.0;

pub fn parse_class(classes: &str) -> Result<Vec<Style>, ClassError> {
    classes.split_whitespace().map(parse_one).collect()
}

fn fail(class: &str, reason: impl Into<String>) -> ClassError {
    ClassError {
        class: class.to_owned(),
        reason: reason.into(),
    }
}

fn spacing(class: &str, value: &str) -> Result<f32, ClassError> {
    let pixels = match value.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
        Some(arbitrary) => arbitrary.trim_end_matches("px").parse::<f32>().ok(),
        None => value.parse::<f32>().ok().map(|units| units * SPACING_UNIT),
    };
    pixels
        .filter(|pixels| pixels.is_finite() && *pixels >= 0.0)
        .ok_or_else(|| fail(class, "expected a spacing step like `2` or pixels like `[10]`"))
}

fn length(class: &str, value: &str) -> Result<Length, ClassError> {
    match value {
        "full" => Ok(Length::Full),
        "fit" => Ok(Length::Fit),
        value => spacing(class, value).map(Length::Px),
    }
}

fn token(class: &str, name: &str) -> Result<Token, ClassError> {
    if name.starts_with('[') || name.starts_with('#') || !name.contains('.') {
        return Err(fail(
            class,
            "literal colors are not allowed; use a theme token like `surface.raised`",
        ));
    }
    Token::new(name).ok_or_else(|| fail(class, format!("`{name}` is not a theme color token")))
}

fn text_size(name: &str) -> Option<TextSize> {
    Some(match name {
        "xs" => TextSize::Xs,
        "sm" => TextSize::Sm,
        "base" => TextSize::Base,
        "lg" => TextSize::Lg,
        "xl" => TextSize::Xl,
        "2xl" => TextSize::X2l,
        "3xl" => TextSize::X3l,
        "4xl" => TextSize::X4l,
        "5xl" => TextSize::X5l,
        "6xl" => TextSize::X6l,
        "7xl" => TextSize::X7l,
        _ => return None,
    })
}

fn parse_one(class: &str) -> Result<Style, ClassError> {
    let exact = match class {
        "rounded" => Some(Style::Rounded(Radius::Md)),
        "border" => Some(Style::Border(1.0)),
        "text-center" => Some(Style::TextCenter),
        "font-mono" => Some(Style::Mono),
        "flex-1" | "grow" => Some(Style::Grow),
        "truncate" => Some(Style::Truncate),
        "hidden" => Some(Style::Hidden),
        _ => None,
    };
    if let Some(style) = exact {
        return Ok(style);
    }
    let (prefix, value) = class
        .rsplit_once('-')
        .filter(|(_, value)| !value.is_empty())
        .ok_or_else(|| fail(class, "unknown class"))?;
    let (prefix, value) = match class.split_once('-') {
        Some((head, rest)) if matches!(head, "bg" | "text" | "border") && rest.contains('.') => {
            (head, rest)
        }
        _ => (prefix, value),
    };
    match prefix {
        "p" => spacing(class, value).map(Style::P),
        "px" => spacing(class, value).map(Style::Px),
        "py" => spacing(class, value).map(Style::Py),
        "pt" => spacing(class, value).map(Style::Pt),
        "pb" => spacing(class, value).map(Style::Pb),
        "pl" => spacing(class, value).map(Style::Pl),
        "pr" => spacing(class, value).map(Style::Pr),
        "gap" => spacing(class, value).map(Style::Gap),
        "w" => length(class, value).map(Style::W),
        "h" => length(class, value).map(Style::H),
        "min-w" => spacing(class, value).map(Style::MinW),
        "max-w" => spacing(class, value).map(Style::MaxW),
        "rounded" => Ok(Style::Rounded(match value {
            "none" => Radius::None,
            "sm" => Radius::Sm,
            "md" => Radius::Md,
            "lg" => Radius::Lg,
            "xl" => Radius::Xl,
            "full" => Radius::Full,
            _ => return Err(fail(class, "use rounded-none|sm|md|lg|xl|full")),
        })),
        "bg" => token(class, value).map(Style::Bg),
        "border" => match value.parse::<f32>() {
            Ok(width) if width >= 0.0 => Ok(Style::Border(width)),
            _ => token(class, value).map(Style::BorderColor),
        },
        "text" => match text_size(value) {
            Some(size) => Ok(Style::TextSize(size)),
            None => token(class, value).map(Style::TextColor),
        },
        "font" => Ok(Style::Weight(match value {
            "light" => Weight::Light,
            "normal" => Weight::Normal,
            "medium" => Weight::Medium,
            "semibold" => Weight::Semibold,
            "bold" => Weight::Bold,
            _ => return Err(fail(class, "use font-light|normal|medium|semibold|bold|mono")),
        })),
        "opacity" => value
            .parse::<f32>()
            .ok()
            .filter(|percent| (0.0..=100.0).contains(percent))
            .map(|percent| Style::Opacity(percent / 100.0))
            .ok_or_else(|| fail(class, "use opacity-0 to opacity-100")),
        "items" => Ok(Style::Items(match value {
            "start" => Align::Start,
            "center" => Align::Center,
            "end" => Align::End,
            "stretch" => Align::Stretch,
            _ => return Err(fail(class, "use items-start|center|end|stretch")),
        })),
        "justify" => Ok(Style::Justify(match value {
            "start" => Justify::Start,
            "center" => Justify::Center,
            "end" => Justify::End,
            "between" => Justify::Between,
            _ => return Err(fail(class, "use justify-start|center|end|between")),
        })),
        _ => Err(fail(class, "unknown class")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_follows_the_tailwind_scale() {
        assert_eq!(
            parse_class("p-2 px-[10] gap-0.5 w-full h-8").expect("valid"),
            vec![
                Style::P(8.0),
                Style::Px(10.0),
                Style::Gap(2.0),
                Style::W(Length::Full),
                Style::H(Length::Px(32.0)),
            ]
        );
    }

    #[test]
    fn colors_are_tokens_and_sizes_are_not_confused_with_them() {
        let styles = parse_class("bg-surface.raised text-text.primary text-sm border-surface.border")
            .expect("valid");
        assert!(matches!(&styles[0], Style::Bg(token) if token.as_str() == "surface.raised"));
        assert!(matches!(&styles[1], Style::TextColor(token) if token.as_str() == "text.primary"));
        assert_eq!(styles[2], Style::TextSize(TextSize::Sm));
        assert!(matches!(&styles[3], Style::BorderColor(_)));
    }

    #[test]
    fn literal_and_unknown_colors_are_refused() {
        for class in ["bg-[#ffffff]", "text-red-500", "bg-#fff", "bg-surface.nope"] {
            assert!(parse_class(class).is_err(), "{class} should be refused");
        }
    }

    #[test]
    fn layout_and_type_classes_parse() {
        assert_eq!(
            parse_class("items-center justify-between font-semibold opacity-50 rounded-lg flex-1")
                .expect("valid"),
            vec![
                Style::Items(Align::Center),
                Style::Justify(Justify::Between),
                Style::Weight(Weight::Semibold),
                Style::Opacity(0.5),
                Style::Rounded(Radius::Lg),
                Style::Grow,
            ]
        );
    }

    #[test]
    fn errors_name_the_class() {
        let error = parse_class("p-2 sparkle").expect_err("unknown");
        assert_eq!(error.class, "sparkle");
    }
}
