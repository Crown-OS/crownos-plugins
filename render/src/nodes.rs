//! One [`Node`] at a time to a boxed crownui view.

use crownplugin_view::{ButtonVariant, Node, NodeId, Style, Tone, UiEvent};
use geometry::{Point, Rect};
use view::{AnyView, Cx, Listeners, Styled, View, bind};
use widgets::kit::{
    self, ButtonOptions as _, SliderOptions as _,
    ToggleOptions as _,
};
use widgets::{canvas, hstack, spacer, text, vstack, zstack};

use crate::style::{BoxedProp, Palette, apply, text_size, text_token};
use crate::{EventSink, Resolver, icons};

const ICON_SIZE: f32 = 16.0;
const FALLBACK_ICON: &str = "circle-help";

pub struct Builder<'a> {
    resolver: &'a dyn Resolver,
    palette: Palette,
    events: Option<EventSink>,
}

fn nothing() -> AnyView {
    vstack(()).boxed()
}

impl<'a> Builder<'a> {
    pub fn new(cx: &mut Cx, resolver: &'a dyn Resolver) -> Self {
        Self {
            palette: Palette {
                theme: kit::use_theme(cx),
                extra: resolver.tokens(),
                radii: resolver.radii(),
                mono: resolver.mono_family(),
            },
            events: resolver.events(),
            resolver,
        }
    }

    fn sender(&self) -> impl Fn(UiEvent) + 'static {
        let events = self.events.clone();
        move |event| {
            if let Some(events) = &events {
                events(event);
            }
        }
    }

    fn children(&self, cx: &mut Cx, children: &[Node]) -> Vec<AnyView> {
        children.iter().map(|child| self.node(cx, child)).collect()
    }

    fn style<S: Styled + View>(&self, element: S, styles: &[Style]) -> AnyView {
        apply(element, styles, &self.palette).boxed()
    }

    pub fn node(&self, cx: &mut Cx, node: &Node) -> AnyView {
        match node {
            Node::VStack { style, children } => self.style(vstack(self.children(cx, children)), style),
            Node::HStack { style, children } => self.style(hstack(self.children(cx, children)), style),
            Node::ZStack { style, children } => self.style(zstack(self.children(cx, children)), style),
            Node::Card { style, children } => self.style(kit::card(self.children(cx, children)), style),
            Node::Text { style, text: content } => {
                let theme = self.palette.theme;
                let element = text(content.clone())
                    .text_color(bind(move |runtime| theme.with(runtime, |theme| theme.text.body)));
                self.style(element, style)
            }
            Node::Icon { style, name } => self.icon(name, style),
            Node::Image { style, src } => match self.resolver.image(src) {
                Some(image) => self.style(
                    canvas(move |_, size, list| list.image(image, Rect::new(Point::ZERO, size))),
                    style,
                ),
                None => nothing(),
            },
            Node::Spacer => spacer().flex_1().boxed(),
            Node::Divider => kit::divider().boxed(),
            Node::Badge { text: label, tone } => {
                let (theme, tone) = (self.palette.theme, *tone);
                kit::badge(
                    label.clone(),
                    bind(move |runtime| {
                        theme.with(runtime, |theme| match tone {
                            Tone::Neutral => theme.status.neutral,
                            Tone::Accent => theme.accent.end,
                            Tone::Success => theme.status.success,
                            Tone::Warning => theme.status.warning,
                            Tone::Danger => theme.status.danger,
                        })
                    }),
                )
                .boxed()
            }
            Node::Progress { style, value } => self.style(kit::progress(*value), style),
            Node::Button { id, style, label, icon, variant } => self.button(*id, style, label, icon.as_deref(), *variant),
            Node::Toggle { id, label, on } => self.toggle(cx, *id, label, *on),
            Node::Slider { id, value, min, max, step } => self.slider(cx, *id, *value, *min, *max, *step),
            Node::Select { id, options, selected } => self.select(cx, *id, options, *selected),
            Node::Placeholder { name } => self.resolver.placeholder(name, cx).unwrap_or_else(nothing),
        }
    }

    fn icon(&self, name: &str, style: &[Style]) -> AnyView {
        let body = icons::lucide(name)
            .or_else(|| icons::lucide(FALLBACK_ICON))
            .unwrap_or_default();
        let size = text_size(style).unwrap_or(ICON_SIZE);
        let tint = match text_token(style) {
            Some(token) => self.palette.color(token, |theme| theme.text.icon).boxed_prop(),
            None => {
                let theme = self.palette.theme;
                bind(move |runtime| theme.with(runtime, |theme| theme.text.icon)).boxed_prop()
            }
        };
        let element = widgets::icon(blinc_icons::to_svg(body, size), tint).size(size);
        let style: Vec<Style> = style
            .iter()
            .filter(|style| !matches!(style, Style::TextColor(_) | Style::TextSize(_)))
            .cloned()
            .collect();
        self.style(element, &style)
    }

    fn button(&self, id: NodeId, style: &[Style], label: &str, icon: Option<&str>, variant: ButtonVariant) -> AnyView {
        let send = self.sender();
        let body = icon.and_then(icons::lucide);
        let element = match (label.is_empty(), body) {
            (true, Some(body)) => kit::icon_button(body, icon.unwrap_or_default().to_owned()),
            (_, Some(body)) => kit::button(label.to_owned()).leading_icon(body),
            (_, None) => kit::button(label.to_owned()),
        }
        .variant(match variant {
            ButtonVariant::Primary => kit::ButtonVariant::Primary,
            ButtonVariant::Secondary => kit::ButtonVariant::Secondary,
            ButtonVariant::Ghost => kit::ButtonVariant::Ghost,
            ButtonVariant::Danger => kit::ButtonVariant::Destructive,
        })
        .on_click(move |_, event| {
            event.stop_propagation();
            send(UiEvent::Click(id));
        });
        self.style(element, style)
    }

    fn toggle(&self, cx: &mut Cx, id: NodeId, label: &str, on: bool) -> AnyView {
        let signal = cx.signal(on);
        let send = self.sender();
        let mut last = on;
        cx.effect(move |runtime| {
            let now = signal.get(runtime);
            if now != last {
                last = now;
                send(UiEvent::Toggle(id, now));
            }
        });
        kit::toggle(signal).caption(label.to_owned()).boxed()
    }

    fn slider(&self, cx: &mut Cx, id: NodeId, value: f32, min: f32, max: f32, step: f32) -> AnyView {
        let signal = cx.signal(value);
        let send = self.sender();
        let mut last = value;
        cx.effect(move |runtime| {
            let now = signal.get(runtime);
            if (now - last).abs() > f32::EPSILON {
                last = now;
                send(UiEvent::Change(id, now));
            }
        });
        kit::slider(signal).range(min, max).step(step).boxed()
    }

    fn select(&self, cx: &mut Cx, id: NodeId, options: &[String], selected: u32) -> AnyView {
        let options = cx.signal(options.to_vec());
        let chosen = cx.signal(Some(selected as usize));
        let send = self.sender();
        let mut last = Some(selected as usize);
        cx.effect(move |runtime| {
            let now = chosen.get(runtime);
            if now != last {
                last = now;
                if let Some(index) = now {
                    send(UiEvent::Select(id, index as u32));
                }
            }
        });
        kit::select(options, chosen).boxed()
    }
}
