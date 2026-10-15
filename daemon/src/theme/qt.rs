//! A qt5ct/qt6ct color scheme from the theme tokens.

use super::gtk::{Palette, Rgba};

fn argb([r, g, b, a]: Rgba) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", byte(a), byte(r), byte(g), byte(b))
}

fn mix([r1, g1, b1, _]: Rgba, [r2, g2, b2, _]: Rgba, t: f32) -> Rgba {
    [r1 + (r2 - r1) * t, g1 + (g2 - g1) * t, b1 + (b2 - b1) * t, 1.0]
}

pub fn color_scheme(palette: &Palette) -> String {
    let p = palette;
    let light = mix(p.window_bg, [1.0, 1.0, 1.0, 1.0], 0.3);
    let dark = mix(p.window_bg, [0.0, 0.0, 0.0, 1.0], 0.4);
    // QPalette::ColorRole order.
    let active = [
        p.window_fg,
        p.view_bg,
        light,
        mix(p.window_bg, light, 0.5),
        dark,
        mix(p.window_bg, dark, 0.5),
        p.window_fg,
        [1.0, 1.0, 1.0, 1.0],
        p.window_fg,
        p.view_bg,
        p.window_bg,
        [0.0, 0.0, 0.0, 1.0],
        p.accent,
        p.accent_fg,
        p.accent,
        mix(p.accent, p.danger, 0.5),
        mix(p.view_bg, p.window_bg, 0.5),
        [0.0, 0.0, 0.0, 1.0],
        p.popover_bg,
        p.window_fg,
        p.muted,
    ];
    let disabled: Vec<Rgba> = active.iter().map(|color| mix(*color, p.window_bg, 0.5)).collect();
    let join = |colors: &[Rgba]| colors.iter().map(|color| argb(*color)).collect::<Vec<_>>().join(", ");
    format!(
        "[ColorScheme]\nactive_colors={}\ndisabled_colors={}\ninactive_colors={}\n",
        join(&active),
        join(&disabled),
        join(&active)
    )
}
