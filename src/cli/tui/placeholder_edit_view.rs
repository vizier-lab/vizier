#![allow(unused)]
use cursive::{
    Printer,
    style::{BaseColor, Color, ColorStyle},
    view::{View, ViewWrapper},
    views::EditView,
};

/// A wrapper around `EditView` that displays placeholder text when the input is empty.
pub struct PlaceholderEditView {
    inner: EditView,
    placeholder: String,
}

impl PlaceholderEditView {
    /// Creates a new `PlaceholderEditView` with the given placeholder text.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            inner: EditView::new().filler(" "),
            placeholder: placeholder.into(),
        }
    }

    /// Access the inner `EditView` for chaining configuration.
    pub fn inner_mut(&mut self) -> &mut EditView {
        &mut self.inner
    }

    /// Clears the content of the edit view.
    pub fn clear(&mut self) {
        self.inner.set_content("");
    }

    /// Sets the content of the edit view.
    pub fn set_content<S: Into<String>>(&mut self, content: S) {
        self.inner.set_content(content);
    }

    /// Sets a callback for when Enter is pressed. Chainable.
    #[must_use]
    pub fn on_submit<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut cursive::Cursive, &str) + 'static + Send + Sync,
    {
        self.inner.set_on_submit(callback);
        self
    }

    /// Sets a callback for when content is edited. Chainable.
    #[must_use]
    pub fn on_edit<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut cursive::Cursive, &str, usize) + 'static + Send + Sync,
    {
        self.inner.set_on_edit(callback);
        self
    }
}

impl ViewWrapper for PlaceholderEditView {
    cursive::wrap_impl!(self.inner: EditView);

    fn wrap_draw(&self, printer: &Printer) {
        if self.inner.get_content().is_empty() {
            // Draw placeholder text in a dimmed style
            let style = ColorStyle::new(Color::Light(BaseColor::White), Color::TerminalDefault);
            printer.with_color(style, |printer| {
                printer.print((0, 0), &self.placeholder);
            });
        } else {
            self.inner.draw(printer);
        }
    }
}
