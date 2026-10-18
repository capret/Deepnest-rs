//! Small native wrappers for clipped disclosure and a shared tooltip anchor.
use crate::{
    style,
    tokens::{color::BACKGROUND, layout as dimensions, radius, scrollbar},
    Message,
};
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::Renderer as _;
use iced::{
    advanced::{
        layout, mouse, overlay, renderer,
        widget::{tree, Operation, Tree},
        Clipboard, Layout, Shell, Widget,
    },
    widget::{canvas, container, scrollable},
    Element, Event, Fill, Length, Padding, Point, Rectangle, Renderer, Size, Theme, Vector,
};

#[derive(Clone, Copy)]
enum Effect {
    Hint(&'static str),
    Panel,
    Reveal {
        horizontal: bool,
        progress: f32,
        extent: f32,
    },
}

struct Wrapper<'a> {
    content: Element<'a, Message>,
    effect: Effect,
}
#[derive(Default)]
struct State {
    hovered: bool,
}

/// Rounded surface for workspace panels, including opaque canvas/scroll children.
/// Iced's container clip is rectangular, so finish the child paint with a corner
/// mask and the panel border. Workspace panels sit on the root background.
pub fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let content = container(content).style(|theme| {
        let mut appearance = style::panel(theme);
        // Paint the border once, after the children, so they cannot cover it.
        appearance.border.width = 0.;
        appearance
    });
    Element::new(Wrapper {
        content: content.into(),
        effect: Effect::Panel,
    })
}

pub fn tool_group<'a>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Container<'a, Message> {
    container(content)
        .padding(dimensions::TOOL_GROUP_INSET)
        .style(style::tool_group)
}

/// Inset the whole scroll viewport, not its scrolling contents, so the track
/// stays clear of panel corners at either end of the scroll range.
pub fn scroll_region<'a>(
    id: &'static str,
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Container<'a, Message> {
    container(
        scrollable(content)
            .id(id)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(scrollbar::RAIL_WIDTH)
                    .scroller_width(scrollbar::THUMB_WIDTH)
                    .spacing(scrollbar::CONTENT_GAP),
            ))
            .style(style::scroll)
            .width(Fill)
            .height(Fill),
    )
    .padding(Padding {
        top: scrollbar::END_INSET,
        right: scrollbar::EDGE_INSET,
        bottom: scrollbar::END_INSET,
        left: 0.,
    })
    .width(Fill)
    .height(Fill)
}

fn panel_contains(bounds: Rectangle, point: Point) -> bool {
    if !bounds.contains(point) {
        return false;
    }
    let r = radius::PANEL.min(bounds.width / 2.).min(bounds.height / 2.);
    let dx = point.x - point.x.clamp(bounds.x + r, bounds.x + bounds.width - r);
    let dy = point.y - point.y.clamp(bounds.y + r, bounds.y + bounds.height - r);
    dx * dx + dy * dy <= r * r
}

pub fn hint<'a>(
    content: impl Into<Element<'a, Message>>,
    label: &'static str,
) -> Element<'a, Message> {
    Element::new(Wrapper {
        content: content.into(),
        effect: Effect::Hint(label),
    })
}
pub fn reveal<'a>(
    content: impl Into<Element<'a, Message>>,
    progress: f64,
    horizontal: bool,
    extent: f32,
) -> Element<'a, Message> {
    Element::new(Wrapper {
        content: content.into(),
        effect: Effect::Reveal {
            horizontal,
            progress: progress.clamp(0., 1.) as f32,
            extent,
        },
    })
}
impl Widget<Message, Theme, Renderer> for Wrapper<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        match self.effect {
            Effect::Hint(_) | Effect::Panel => self.content.as_widget().size(),
            Effect::Reveal {
                horizontal: true,
                progress,
                extent,
            } => Size::new(Length::Fixed(extent * progress), Length::Fill),
            Effect::Reveal {
                horizontal: false, ..
            } => Size::new(Length::Fill, Length::Shrink),
        }
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let inner_limits = match self.effect {
            Effect::Hint(_) | Effect::Panel => *limits,
            Effect::Reveal {
                horizontal, extent, ..
            } => layout::Limits::new(
                Size::ZERO,
                if horizontal {
                    Size::new(extent, limits.max().height)
                } else {
                    Size::new(limits.max().width, extent)
                },
            ),
        };
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &inner_limits);
        let mut size = child.size();
        if let Effect::Reveal {
            horizontal,
            progress,
            ..
        } = self.effect
        {
            if horizontal {
                size.width *= progress;
            } else {
                size.height *= progress;
            }
        }
        layout::Node::with_children(size, vec![child])
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(clip) = layout.bounds().intersection(viewport) {
            renderer.with_layer(clip, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout.children().next().unwrap(),
                    cursor,
                    &clip,
                );
                if matches!(self.effect, Effect::Panel) {
                    let bounds = layout.bounds();
                    let size = bounds.size();
                    let mut frame = canvas::Frame::new(renderer, size);
                    let mask = canvas::Path::new(|p| {
                        p.rectangle(Point::ORIGIN, size);
                        p.rounded_rectangle(Point::ORIGIN, size, radius::PANEL.into());
                    });
                    frame.fill(
                        &mask,
                        canvas::Fill {
                            style: canvas::Style::Solid(BACKGROUND),
                            rule: canvas::fill::Rule::EvenOdd,
                        },
                    );
                    let border = style::panel(theme).border;
                    let inset = border.width / 2.;
                    frame.stroke(
                        &canvas::Path::rounded_rectangle(
                            Point::new(inset, inset),
                            Size::new(
                                (size.width - border.width).max(0.),
                                (size.height - border.width).max(0.),
                            ),
                            radius::inset(radius::PANEL, inset).into(),
                        ),
                        canvas::Stroke::default()
                            .with_width(border.width)
                            .with_color(border.color),
                    );
                    renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                        renderer.draw_geometry(frame.into_geometry());
                    });
                }
            });
        }
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if !matches!(self.effect,Effect::Reveal {progress,..} if progress<0.99) {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        }
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Effect::Hint(label) = self.effect {
            if matches!(
                event,
                Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
            ) {
                let state = tree.state.downcast_mut::<State>();
                let hovered = layout
                    .bounds()
                    .intersection(viewport)
                    .is_some_and(|b| cursor.is_over(b));
                if hovered != state.hovered {
                    state.hovered = hovered;
                    shell.publish(if hovered {
                        Message::HintEnter(label, layout.bounds())
                    } else {
                        Message::HintLeave(label)
                    });
                }
            }
        }
        if let Some(clip) = layout.bounds().intersection(viewport) {
            let over_panel = !matches!(self.effect, Effect::Panel)
                || cursor
                    .position()
                    .is_some_and(|p| panel_contains(layout.bounds(), p));
            let cursor = if cursor.is_over(clip) && over_panel {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout.children().next().unwrap(),
                cursor,
                renderer,
                clipboard,
                shell,
                &clip,
            );
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let over_panel = !matches!(self.effect, Effect::Panel)
            || cursor
                .position()
                .is_some_and(|p| panel_contains(layout.bounds(), p));
        if cursor.is_over(layout.bounds()) && over_panel {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout.children().next().unwrap(),
                cursor,
                viewport,
                renderer,
            )
        } else {
            mouse::Interaction::None
        }
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        if matches!(self.effect,Effect::Reveal {progress,..} if progress<0.99) {
            return None;
        }
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::color::PRIMARY;
    use iced::advanced::renderer::Headless;

    struct OpaqueCanvas;
    impl canvas::Program<Message> for OpaqueCanvas {
        type State = ();

        fn draw(
            &self,
            _: &(),
            renderer: &Renderer,
            _: &Theme,
            bounds: Rectangle,
            _: mouse::Cursor,
        ) -> Vec<canvas::Geometry> {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), PRIMARY);
            vec![frame.into_geometry()]
        }
    }

    #[test]
    fn rounded_panels_mask_opaque_canvas_children_and_exclude_corner_hits() {
        let mut renderer = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(Renderer::new(
                iced::Font::DEFAULT,
                12.into(),
                Some("tiny-skia"),
            ))
            .expect("The CPU renderer is available for panel regression checks");
        let viewport = Rectangle::with_size(Size::new(120., 100.));
        renderer.reset(viewport);
        let mut element = panel(canvas::Canvas::new(OpaqueCanvas).width(96).height(80));
        let mut tree = Tree::new(&element);
        let node = element
            .as_widget_mut()
            .layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, viewport.size()),
            )
            .move_to(Point::new(12., 10.));
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &Theme::Dark,
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        let pixels = renderer.screenshot(Size::new(120, 100), 1., BACKGROUND);
        let pixel = |x: usize, y: usize| &pixels[(y * 120 + x) * 4..][..4];
        // A rectangular canvas fills these corners without the shared finish pass.
        for (x, y) in [(13, 11), (106, 11), (13, 88), (106, 88)] {
            assert_eq!(pixel(x, y), &BACKGROUND.into_rgba8());
            assert!(!panel_contains(
                node.bounds(),
                Point::new(x as f32, y as f32)
            ));
        }
        assert_eq!(pixel(60, 50), &PRIMARY.into_rgba8());
        assert!(panel_contains(node.bounds(), Point::new(60., 50.)));
    }

    #[test]
    fn scrollbars_stay_inset_at_both_scroll_limits_and_leave_content_space() {
        use crate::tokens::color::SIDEBAR;

        let mut renderer = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(Renderer::new(
                iced::Font::DEFAULT,
                12.into(),
                Some("tiny-skia"),
            ))
            .unwrap();
        let viewport = Rectangle::with_size(Size::new(120., 100.));
        let mut element = panel(scroll_region(
            "scroll-inset-test",
            canvas::Canvas::new(OpaqueCanvas).width(Fill).height(400),
        ));
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(viewport.size(), viewport.size()),
        );

        for bottom in [false, true] {
            if bottom {
                element.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Lines { x: 0., y: -1000. },
                    }),
                    Layout::new(&node),
                    mouse::Cursor::Available(Point::new(50., 50.)),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut Vec::new()),
                    &viewport,
                );
            }
            renderer.reset(viewport);
            element.as_widget().draw(
                &tree,
                &mut renderer,
                &Theme::Dark,
                &renderer::Style::default(),
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &viewport,
            );
            let pixels = renderer.screenshot(Size::new(120, 100), 1., BACKGROUND);
            let pixel = |x: usize, y: usize| &pixels[(y * 120 + x) * 4..][..4];
            // The thumb moves, while the top/bottom/right inset stays empty.
            assert_ne!(
                pixel(112, if bottom { 88 } else { 12 }),
                &SIDEBAR.into_rgba8()
            );
            for (x, y) in [(112, 6), (112, 94), (116, 12)] {
                assert_eq!(pixel(x, y), &SIDEBAR.into_rgba8());
            }
            assert_eq!(pixel(100, 40), &PRIMARY.into_rgba8());
            assert_eq!(pixel(106, 40), &SIDEBAR.into_rgba8());
        }
    }
}
