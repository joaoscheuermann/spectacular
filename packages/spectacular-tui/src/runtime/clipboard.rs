use arboard::Clipboard;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Error returned when the platform clipboard cannot be read or written.
#[derive(Debug)]
pub struct ClipboardError {
    kind: ClipboardErrorKind,
}

#[derive(Debug)]
enum ClipboardErrorKind {
    Arboard(arboard::Error),
    Message(String),
}

impl ClipboardError {
    /// Wraps an arboard error so clipboard details stay isolated at the runtime boundary.
    fn new(source: arboard::Error) -> Self {
        Self {
            kind: ClipboardErrorKind::Arboard(source),
        }
    }

    /// Creates a clipboard error from a caller-owned diagnostic message.
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            kind: ClipboardErrorKind::Message(message.into()),
        }
    }
}

impl Display for ClipboardError {
    /// Formats the underlying clipboard error for diagnostics.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ClipboardErrorKind::Arboard(source) => Display::fmt(source, formatter),
            ClipboardErrorKind::Message(message) => formatter.write_str(message),
        }
    }
}

impl Error for ClipboardError {
    /// Returns the native clipboard source error when available.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ClipboardErrorKind::Arboard(source) => Some(source),
            ClipboardErrorKind::Message(_) => None,
        }
    }
}

impl From<arboard::Error> for ClipboardError {
    /// Converts an arboard error into the runtime clipboard error type.
    fn from(value: arboard::Error) -> Self {
        Self::new(value)
    }
}

/// Clipboard access required by local TUI copy and paste commands.
pub trait ClipboardService: Send {
    /// Reads the current OS clipboard text.
    fn get_text(&mut self) -> Result<String, ClipboardError>;

    /// Writes text to the OS clipboard.
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// System clipboard implementation backed by arboard.
pub struct SystemClipboard {
    inner: Clipboard,
}

impl SystemClipboard {
    /// Opens the platform clipboard for local TUI clipboard operations.
    pub fn new() -> Result<Self, ClipboardError> {
        Ok(Self {
            inner: Clipboard::new()?,
        })
    }
}

impl ClipboardService for SystemClipboard {
    /// Reads text from the native clipboard.
    fn get_text(&mut self) -> Result<String, ClipboardError> {
        self.inner.get_text().map_err(ClipboardError::from)
    }

    /// Writes text to the native clipboard.
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.inner
            .set_text(text.to_owned())
            .map_err(ClipboardError::from)
    }
}

/// Creates a system clipboard service when the OS clipboard is available.
pub fn system_clipboard() -> Option<Box<dyn ClipboardService>> {
    Some(Box::new(SystemClipboard::new().ok()?))
}
