//! Small native wrappers for clipped disclosure and a shared tooltip anchor.
use crate::Message;
use iced::advanced::Renderer as _;
use iced::{
    advanced::{
        layout, mouse, overlay, renderer,
        widget::{tree, Operation, Tree},
        Clipboard, Layout, Shell, Widget,
    },
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
};

#[derive(Clone, Copy)]
enum Effect {
    Hint(&'static str),
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
            Effect::Hint(_) => self.content.as_widget().size(),
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
            Effect::Hint(_) => *limits,
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
                )
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
            let cursor = if cursor.is_over(clip) {
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
        if cursor.is_over(layout.bounds()) {
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
