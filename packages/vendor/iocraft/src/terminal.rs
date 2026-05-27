use crate::{canvas::Canvas, element::Output};
use crossterm::{
    cursor,
    event::{self, Event, EventStream},
    terminal, ExecutableCommand, QueueableCommand,
};
use futures::{
    channel::mpsc,
    future::pending,
    stream::{self, BoxStream, Stream, StreamExt},
};
use std::{
    collections::VecDeque,
    fmt,
    io::{self, stdin, IsTerminal, Write},
    mem,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};

const SPECTACULAR_KEY_DEBUG_ENV: &str = "SPECTACULAR_TUI_KEY_DEBUG";
const SPECTACULAR_KEY_DEBUG_FILE: &str = "spectacular-tui-key-debug.log";
const SPECTACULAR_FORCE_KEYBOARD_ENHANCEMENT_ENV: &str =
    "SPECTACULAR_TUI_FORCE_KEYBOARD_ENHANCEMENT";

// Re-exports for basic types.
pub use crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers, MouseEventKind};

/// An event fired when a key is pressed.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct KeyEvent {
    /// A code indicating the key that was pressed.
    pub code: KeyCode,

    /// The modifiers that were active when the key was pressed.
    pub modifiers: KeyModifiers,

    /// Whether the key was pressed or released.
    pub kind: KeyEventKind,
}

impl KeyEvent {
    /// Creates a new `KeyEvent`.
    pub fn new(kind: KeyEventKind, code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::empty(),
            kind,
        }
    }
}

/// An event fired when the mouse is moved, clicked, scrolled, etc. in fullscreen mode.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct FullscreenMouseEvent {
    /// The modifiers that were active when the event occurred.
    pub modifiers: KeyModifiers,

    /// The column that the event occurred on.
    pub column: u16,

    /// The row that the event occurred on.
    pub row: u16,

    /// The kind of mouse event.
    pub kind: MouseEventKind,
}

impl FullscreenMouseEvent {
    /// Creates a new `FullscreenMouseEvent`.
    pub fn new(kind: MouseEventKind, column: u16, row: u16) -> Self {
        Self {
            modifiers: KeyModifiers::empty(),
            column,
            row,
            kind,
        }
    }
}

/// An event fired by the terminal.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum TerminalEvent {
    /// A key event, fired when a key is pressed.
    Key(KeyEvent),
    /// A paste event, fired when bracketed paste mode reports pasted text.
    Paste(String),
    /// A mouse event, fired when the mouse is moved, clicked, scrolled, etc. in fullscreen mode.
    FullscreenMouse(FullscreenMouseEvent),
    /// A resize event, fired when the terminal is resized.
    Resize(u16, u16),
}

struct TerminalEventsInner {
    pending: VecDeque<TerminalEvent>,
    waker: Option<Waker>,
}

/// A stream of terminal events.
pub struct TerminalEvents {
    inner: Arc<Mutex<TerminalEventsInner>>,
}

impl Stream for TerminalEvents {
    type Item = TerminalEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(event) = inner.pending.pop_front() {
            Poll::Ready(Some(event))
        } else {
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

trait TerminalImpl: Write + Send {
    fn refresh_size(&mut self) {}
    fn size(&self) -> Option<(u16, u16)> {
        None
    }
    fn set_mouse_capture(&mut self, _enabled: bool) -> io::Result<()> {
        Ok(())
    }

    fn is_raw_mode_enabled(&self) -> bool;
    fn clear_canvas(&mut self) -> io::Result<()>;
    fn write_canvas(&mut self, canvas: &Canvas) -> io::Result<()>;
    fn event_stream(&mut self) -> io::Result<BoxStream<'static, TerminalEvent>>;
    fn dest(&mut self) -> &mut dyn Write;
    fn alt(&mut self) -> &mut dyn Write;
}

fn clear_canvas_inline(
    dest: &mut (impl Write + ?Sized),
    prev_canvas_height: u16,
) -> io::Result<()> {
    let lines_to_rewind = prev_canvas_height - 1;
    if lines_to_rewind == 0 {
        dest.queue(cursor::MoveToColumn(0))?
            .queue(terminal::Clear(terminal::ClearType::FromCursorDown))?;
        Ok(())
    } else {
        dest.queue(cursor::MoveToPreviousLine(lines_to_rewind as _))?
            .queue(terminal::Clear(terminal::ClearType::FromCursorDown))?;
        Ok(())
    }
}

struct StdTerminal<'a> {
    input_is_terminal: bool,
    dest: Box<dyn Write + Send + 'a>,
    alt: Box<dyn Write + Send + 'a>,
    fullscreen: bool,
    mouse_capture: bool,
    raw_mode_enabled: bool,
    bracketed_paste_enabled: bool,
    enabled_keyboard_enhancement: bool,
    #[cfg(windows)]
    previous_windows_input_mode: Option<u32>,
    prev_canvas_height: u16,
    size: Option<(u16, u16)>,
}

impl Write for StdTerminal<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.dest.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.dest.flush()
    }
}

impl TerminalImpl for StdTerminal<'_> {
    fn refresh_size(&mut self) {
        self.size = terminal::size().ok()
    }

    fn size(&self) -> Option<(u16, u16)> {
        self.size
    }

    fn set_mouse_capture(&mut self, enabled: bool) -> io::Result<()> {
        if self.mouse_capture != enabled {
            self.mouse_capture = enabled;
            if self.raw_mode_enabled {
                if enabled {
                    self.dest.execute(event::EnableMouseCapture)?;
                } else {
                    self.dest.execute(event::DisableMouseCapture)?;
                }
            }
        }
        Ok(())
    }

    fn is_raw_mode_enabled(&self) -> bool {
        self.raw_mode_enabled
    }

    fn clear_canvas(&mut self) -> io::Result<()> {
        if self.prev_canvas_height == 0 {
            return Ok(());
        }

        if !self.fullscreen {
            if let Some(size) = self.size {
                if self.prev_canvas_height >= size.1 {
                    // We have to clear the entire terminal to avoid leaving artifacts.
                    // See: https://github.com/ccbrown/iocraft/issues/118
                    self.dest
                        .queue(terminal::Clear(terminal::ClearType::All))?
                        .queue(terminal::Clear(terminal::ClearType::Purge))?
                        .queue(cursor::MoveTo(0, 0))?;
                    return Ok(());
                }
            }
        }

        clear_canvas_inline(&mut *self.dest, self.prev_canvas_height)
    }

    fn write_canvas(&mut self, canvas: &Canvas) -> io::Result<()> {
        self.prev_canvas_height = canvas.height() as _;
        canvas.write_ansi_without_final_newline(self)?;
        Ok(())
    }

    fn event_stream(&mut self) -> io::Result<BoxStream<'static, TerminalEvent>> {
        if !self.input_is_terminal {
            return Ok(stream::pending().boxed());
        }

        self.set_raw_mode_enabled(true)?;

        Ok(EventStream::new()
            .filter_map(|event| async move { event.ok().and_then(terminal_event_from_crossterm) })
            .boxed())
    }

    fn dest(&mut self) -> &mut dyn Write {
        &mut *self.dest
    }

    fn alt(&mut self) -> &mut dyn Write {
        &mut *self.alt
    }
}

impl<'a> StdTerminal<'a> {
    fn new(
        dest: Box<dyn Write + Send + 'a>,
        alt: Box<dyn Write + Send + 'a>,
        fullscreen: bool,
        mouse_capture: bool,
    ) -> io::Result<Self> {
        let mut term = Self {
            dest,
            alt,
            input_is_terminal: stdin().is_terminal(),
            fullscreen,
            mouse_capture,
            raw_mode_enabled: false,
            bracketed_paste_enabled: false,
            enabled_keyboard_enhancement: false,
            #[cfg(windows)]
            previous_windows_input_mode: None,
            prev_canvas_height: 0,
            size: None,
        };
        term.dest.queue(cursor::Hide)?;
        if fullscreen {
            term.dest.queue(terminal::EnterAlternateScreen)?;
        }
        Ok(term)
    }

    fn set_raw_mode_enabled(&mut self, raw_mode_enabled: bool) -> io::Result<()> {
        if raw_mode_enabled != self.raw_mode_enabled {
            if raw_mode_enabled {
                self.enable_keyboard_enhancement()?;
                self.disable_windows_selection_interception()?;
                if self.mouse_capture {
                    self.dest.execute(event::EnableMouseCapture)?;
                }
                self.dest.execute(event::EnableBracketedPaste)?;
                self.bracketed_paste_enabled = true;
                if let Err(error) = terminal::enable_raw_mode() {
                    let _ = self.disable_bracketed_paste();
                    let _ = self.restore_windows_input_mode();
                    return Err(error);
                }
            } else {
                self.disable_bracketed_paste()?;
                terminal::disable_raw_mode()?;
                self.restore_windows_input_mode()?;
                if self.mouse_capture {
                    self.dest.execute(event::DisableMouseCapture)?;
                }
                self.disable_keyboard_enhancement()?;
            }
            self.raw_mode_enabled = raw_mode_enabled;
        }
        Ok(())
    }

    fn enable_keyboard_enhancement(&mut self) -> io::Result<()> {
        if self.enabled_keyboard_enhancement || !should_enable_keyboard_enhancement() {
            return Ok(());
        }

        write!(self.dest, "\x1b[>{}u", keyboard_enhancement_flags().bits())?;
        self.dest.flush()?;
        self.enabled_keyboard_enhancement = true;
        debug_terminal_note(format_args!(
            "keyboard enhancement enabled: flags={}",
            keyboard_enhancement_flags().bits()
        ));
        Ok(())
    }

    fn disable_keyboard_enhancement(&mut self) -> io::Result<()> {
        if !self.enabled_keyboard_enhancement {
            return Ok(());
        }

        self.dest.write_all(b"\x1b[<1u")?;
        self.dest.flush()?;
        self.enabled_keyboard_enhancement = false;
        debug_terminal_note(format_args!("keyboard enhancement disabled"));
        Ok(())
    }

    fn disable_bracketed_paste(&mut self) -> io::Result<()> {
        if self.bracketed_paste_enabled {
            self.dest.execute(event::DisableBracketedPaste)?;
            self.bracketed_paste_enabled = false;
        }
        Ok(())
    }

    fn disable_windows_selection_interception(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        {
            if self.previous_windows_input_mode.is_none() {
                self.previous_windows_input_mode =
                    windows_console_input::disable_selection_interception()?;
            }
        }
        Ok(())
    }

    fn restore_windows_input_mode(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        {
            let Some(mode) = self.previous_windows_input_mode.take() else {
                return Ok(());
            };
            windows_console_input::restore(mode)?;
        }
        Ok(())
    }
}

impl Drop for StdTerminal<'_> {
    fn drop(&mut self) {
        let _ = self.set_raw_mode_enabled(false);
        if self.fullscreen {
            let _ = self.dest.queue(terminal::LeaveAlternateScreen);
        } else if self.prev_canvas_height > 0 {
            let _ = self.dest.write_all(b"\r\n");
        }
        let _ = self.dest.execute(cursor::Show);
    }
}

fn terminal_event_from_crossterm(event: Event) -> Option<TerminalEvent> {
    debug_terminal_event(&event);
    match event {
        Event::Key(event) => Some(TerminalEvent::Key(KeyEvent {
            code: event.code,
            modifiers: event.modifiers,
            kind: event.kind,
        })),
        Event::Paste(text) => Some(TerminalEvent::Paste(text)),
        Event::Mouse(event) => Some(TerminalEvent::FullscreenMouse(FullscreenMouseEvent {
            modifiers: event.modifiers,
            column: event.column,
            row: event.row,
            kind: event.kind,
        })),
        Event::Resize(width, height) => Some(TerminalEvent::Resize(width, height)),
        _ => None,
    }
}

fn keyboard_enhancement_flags() -> event::KeyboardEnhancementFlags {
    event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
}

fn should_enable_keyboard_enhancement() -> bool {
    terminal::supports_keyboard_enhancement().unwrap_or(false)
        || env_flag_enabled(SPECTACULAR_FORCE_KEYBOARD_ENHANCEMENT_ENV)
        || should_enable_windows_terminal_keyboard_enhancement()
}

fn should_enable_windows_terminal_keyboard_enhancement() -> bool {
    cfg!(windows)
        && (std::env::var_os("WT_SESSION").is_some()
            || std::env::var_os("WEZTERM_EXECUTABLE").is_some()
            || std::env::var_os("ALACRITTY_LOG").is_some()
            || std::env::var_os("KITTY_WINDOW_ID").is_some())
}

fn debug_terminal_event(event: &Event) {
    debug_terminal_note(format_args!("event: {event:?}"));
}

fn debug_terminal_note(args: fmt::Arguments<'_>) {
    if !env_flag_enabled(SPECTACULAR_KEY_DEBUG_ENV) {
        return;
    }

    let path = std::env::temp_dir().join(SPECTACULAR_KEY_DEBUG_FILE);
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(file, "{args}");
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        !value.is_empty() && !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off")
    })
}

#[cfg(windows)]
mod windows_console_input {
    use std::{ffi::c_void, io};

    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
    const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;

    type Handle = *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> Handle;
        fn GetConsoleMode(h_console_handle: Handle, lp_mode: *mut u32) -> i32;
        fn SetConsoleMode(h_console_handle: Handle, dw_mode: u32) -> i32;
    }

    /// Disables console text-selection interception so modified arrows can reach the app.
    ///
    /// This only controls the Windows console input mode. Terminal hosts may still
    /// reserve their own shortcuts before the process receives a console input
    /// record. Windows Terminal can do this for `Shift+Up/Down`, which means
    /// crossterm receives no `Up` or `Down` key event for the app to handle.
    pub(super) fn disable_selection_interception() -> io::Result<Option<u32>> {
        let Some((handle, mode)) = input_mode()? else {
            return Ok(None);
        };
        let next_mode = (mode | ENABLE_EXTENDED_FLAGS) & !ENABLE_QUICK_EDIT_MODE;
        if next_mode != mode && unsafe { SetConsoleMode(handle, next_mode) } == 0 {
            return Err(io::Error::last_os_error());
        }
        super::debug_terminal_note(format_args!(
            "windows input mode: original=0x{mode:08x}, active=0x{next_mode:08x}"
        ));
        Ok(Some(mode))
    }

    /// Restores the console input mode captured before raw-mode terminal ownership.
    pub(super) fn restore(mode: u32) -> io::Result<()> {
        let Some((handle, _)) = input_mode()? else {
            return Ok(());
        };
        if unsafe { SetConsoleMode(handle, mode) } == 0 {
            return Err(io::Error::last_os_error());
        }
        super::debug_terminal_note(format_args!(
            "windows input mode restored: mode=0x{mode:08x}"
        ));
        Ok(())
    }

    fn input_mode() -> io::Result<Option<(Handle, u32)>> {
        let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if handle.is_null() || handle == invalid_handle_value() {
            return Ok(None);
        }

        let mut mode = 0;
        if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
            return Ok(None);
        }

        Ok(Some((handle, mode)))
    }

    fn invalid_handle_value() -> Handle {
        -1isize as Handle
    }
}

pub(crate) struct MockTerminalOutputStream {
    inner: mpsc::UnboundedReceiver<Canvas>,
}

impl Stream for MockTerminalOutputStream {
    type Item = Canvas;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

/// Used to provide the configuration for a mock terminal which can be used for testing.
///
/// This can be passed to [`ElementExt::mock_terminal_render_loop`](crate::ElementExt::mock_terminal_render_loop) for testing your dynamic components.
#[non_exhaustive]
pub struct MockTerminalConfig {
    /// The events to be emitted by the mock terminal.
    pub events: BoxStream<'static, TerminalEvent>,
}

impl MockTerminalConfig {
    /// Creates a new `MockTerminalConfig` with the given event stream.
    pub fn with_events<T: Stream<Item = TerminalEvent> + Send + 'static>(events: T) -> Self {
        Self {
            events: events.boxed(),
        }
    }
}

impl Default for MockTerminalConfig {
    fn default() -> Self {
        Self {
            events: stream::pending().boxed(),
        }
    }
}

struct MockTerminal {
    config: MockTerminalConfig,
    output: mpsc::UnboundedSender<Canvas>,
    dummy_dest: io::Sink,
    dummy_alt: io::Sink,
}

impl MockTerminal {
    fn new(config: MockTerminalConfig) -> (Self, MockTerminalOutputStream) {
        let (output_tx, output_rx) = mpsc::unbounded();
        let output = MockTerminalOutputStream { inner: output_rx };
        (
            Self {
                config,
                output: output_tx,
                dummy_dest: io::sink(),
                dummy_alt: io::sink(),
            },
            output,
        )
    }
}

impl Write for MockTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TerminalImpl for MockTerminal {
    fn is_raw_mode_enabled(&self) -> bool {
        false
    }

    fn clear_canvas(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_canvas(&mut self, canvas: &Canvas) -> io::Result<()> {
        let _ = self.output.unbounded_send(canvas.clone());
        Ok(())
    }

    fn event_stream(&mut self) -> io::Result<BoxStream<'static, TerminalEvent>> {
        let mut events = stream::pending().boxed();
        mem::swap(&mut events, &mut self.config.events);
        Ok(events.chain(stream::pending()).boxed())
    }

    fn dest(&mut self) -> &mut dyn Write {
        &mut self.dummy_dest
    }

    fn alt(&mut self) -> &mut dyn Write {
        &mut self.dummy_alt
    }
}

pub(crate) struct Terminal<'a> {
    inner: Box<dyn TerminalImpl + 'a>,
    output: Output,
    event_stream: Option<BoxStream<'static, TerminalEvent>>,
    subscribers: Vec<Weak<Mutex<TerminalEventsInner>>>,
    received_ctrl_c: bool,
    ignore_ctrl_c: bool,
}

impl<'a> Terminal<'a> {
    pub fn new(
        stdout: Box<dyn Write + Send + 'a>,
        stderr: Box<dyn Write + Send + 'a>,
        output: Output,
        fullscreen: bool,
        mouse_capture: bool,
    ) -> io::Result<Self> {
        // dest is the render destination, alt is the other stream
        let (dest, alt) = match output {
            Output::Stdout => (stdout, stderr),
            Output::Stderr => (stderr, stdout),
        };
        Ok(Self {
            inner: Box::new(StdTerminal::new(dest, alt, fullscreen, mouse_capture)?),
            output,
            event_stream: None,
            subscribers: Vec::new(),
            received_ctrl_c: false,
            ignore_ctrl_c: false,
        })
    }

    pub fn enable_mouse_capture(&mut self) -> io::Result<()> {
        self.inner.set_mouse_capture(true)
    }

    pub fn disable_mouse_capture(&mut self) -> io::Result<()> {
        self.inner.set_mouse_capture(false)
    }

    pub fn ignore_ctrl_c(&mut self) {
        self.ignore_ctrl_c = true;
    }

    pub fn is_raw_mode_enabled(&self) -> bool {
        self.inner.is_raw_mode_enabled()
    }

    pub fn refresh_size(&mut self) {
        self.inner.refresh_size()
    }

    pub fn size(&self) -> Option<(u16, u16)> {
        self.inner.size()
    }

    pub fn clear_canvas(&mut self) -> io::Result<()> {
        self.inner.clear_canvas()
    }

    pub fn write_canvas(&mut self, canvas: &Canvas) -> io::Result<()> {
        self.inner.write_canvas(canvas)
    }

    pub fn received_ctrl_c(&self) -> bool {
        self.received_ctrl_c
    }

    /// Returns a mutable reference to the stdout handle.
    pub fn stdout(&mut self) -> &mut dyn Write {
        match self.output {
            Output::Stdout => self.inner.dest(),
            Output::Stderr => self.inner.alt(),
        }
    }

    /// Returns a mutable reference to the stderr handle.
    pub fn stderr(&mut self) -> &mut dyn Write {
        match self.output {
            Output::Stdout => self.inner.alt(),
            Output::Stderr => self.inner.dest(),
        }
    }

    /// Returns a mutable reference to the render output handle (stdout or stderr based on output setting).
    pub fn render_output(&mut self) -> &mut dyn Write {
        self.inner.dest()
    }

    /// Wraps a series of terminal updates in a synchronized update block, making sure to end the
    /// synchronized update even if there is an error or panic.
    pub fn synchronized_update<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Self) -> io::Result<()>,
    {
        let t = SynchronizedUpdate::begin(self)?;
        f(t.inner)
    }

    pub async fn wait(&mut self) {
        match &mut self.event_stream {
            Some(event_stream) => {
                while let Some(event) = event_stream.next().await {
                    if !self.ignore_ctrl_c {
                        if let TerminalEvent::Key(KeyEvent {
                            code: KeyCode::Char('c'),
                            kind: KeyEventKind::Press,
                            modifiers: KeyModifiers::CONTROL,
                        }) = event
                        {
                            self.received_ctrl_c = true;
                        }
                        if self.received_ctrl_c {
                            return;
                        }
                    }
                    self.subscribers.retain(|subscriber| {
                        if let Some(subscriber) = subscriber.upgrade() {
                            let mut subscriber = subscriber.lock().unwrap();
                            subscriber.pending.push_back(event.clone());
                            if let Some(waker) = subscriber.waker.take() {
                                waker.wake();
                            }
                            true
                        } else {
                            false
                        }
                    });
                }
            }
            None => pending().await,
        }
    }

    pub fn events(&mut self) -> io::Result<TerminalEvents> {
        if self.event_stream.is_none() {
            self.event_stream = Some(self.inner.event_stream()?);
        }
        let inner = Arc::new(Mutex::new(TerminalEventsInner {
            pending: VecDeque::new(),
            waker: None,
        }));
        self.subscribers.push(Arc::downgrade(&inner));
        Ok(TerminalEvents { inner })
    }
}

impl Terminal<'static> {
    pub fn mock(config: MockTerminalConfig) -> (Self, MockTerminalOutputStream) {
        let (term, output_stream) = MockTerminal::new(config);
        (
            Self {
                inner: Box::new(term),
                output: Output::Stdout,
                event_stream: None,
                subscribers: Vec::new(),
                received_ctrl_c: false,
                ignore_ctrl_c: false,
            },
            output_stream,
        )
    }
}

impl Write for Terminal<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Synchronized update terminal guard.
/// Enters synchronized update on creation, exits when dropped.
pub(crate) struct SynchronizedUpdate<'a, 'b> {
    inner: &'a mut Terminal<'b>,
}

impl<'a, 'b> SynchronizedUpdate<'a, 'b> {
    pub fn begin(terminal: &'a mut Terminal<'b>) -> io::Result<Self> {
        terminal.execute(terminal::BeginSynchronizedUpdate)?;
        Ok(Self { inner: terminal })
    }
}

impl Drop for SynchronizedUpdate<'_, '_> {
    fn drop(&mut self) {
        let _ = self.inner.execute(terminal::EndSynchronizedUpdate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn test_std_terminal() {
        // There's unfortunately not much here we can really test, but we'll do our best.
        // TODO: Is there a library we can use to emulate terminal input/output?
        let mut terminal = Terminal::new(
            Box::new(std::io::stdout()),
            Box::new(std::io::stderr()),
            Output::Stdout,
            false,
            true,
        )
        .unwrap();
        assert!(!terminal.is_raw_mode_enabled());
        assert!(!terminal.received_ctrl_c());
        assert!(!terminal.is_raw_mode_enabled());
        let canvas = Canvas::new(10, 1);
        terminal.write_canvas(&canvas).unwrap();
    }

    #[test]
    fn test_terminal_event_from_crossterm_maps_paste_events() {
        let event = terminal_event_from_crossterm(Event::Paste("pasted text".to_owned()));

        assert!(matches!(event, Some(TerminalEvent::Paste(text)) if text == "pasted text"));
    }

    #[test]
    fn test_terminal_event_from_crossterm_maps_key_events() {
        let event = terminal_event_from_crossterm(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('v'),
            KeyModifiers::CONTROL,
        )));

        assert!(matches!(
            event,
            Some(TerminalEvent::Key(KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }))
        ));
    }

    fn render_canvas_to_vt(canvas: &Canvas, cols: usize, rows: usize) -> avt::Vt {
        render_canvases_to_vt(&[canvas], cols, rows)
    }

    fn render_canvases_to_vt(canvases: &[&Canvas], cols: usize, rows: usize) -> avt::Vt {
        let mut buf = Vec::new();
        for (i, canvas) in canvases.iter().enumerate() {
            if i > 0 {
                super::clear_canvas_inline(&mut buf, canvases[i - 1].height() as _).unwrap();
            }
            canvas.write_ansi_without_final_newline(&mut buf).unwrap();
        }
        let mut vt = avt::Vt::new(cols, rows);
        vt.feed_str(&String::from_utf8(buf).unwrap());
        vt
    }

    #[test]
    fn test_write_canvas_single_line_cursor_position() {
        let mut canvas = Canvas::new(10, 1);
        canvas
            .subview_mut(0, 0, 0, 0, 10, 1)
            .set_text(0, 0, "hello", CanvasTextStyle::default());

        let vt = render_canvas_to_vt(&canvas, 10, 5);

        assert_eq!(vt.line(0).text(), "hello     ");
        assert_eq!(vt.cursor().row, 0, "cursor should stay on the first row");

        // clear and rerender with new content
        let mut canvas2 = Canvas::new(10, 1);
        canvas2
            .subview_mut(0, 0, 0, 0, 10, 1)
            .set_text(0, 0, "world", CanvasTextStyle::default());

        let vt = render_canvases_to_vt(&[&canvas, &canvas2], 10, 5);

        assert_eq!(vt.line(0).text(), "world     ");
        assert_eq!(vt.cursor().row, 0);
    }

    #[test]
    fn test_write_canvas_multi_line_cursor_position() {
        let mut canvas = Canvas::new(10, 3);
        canvas
            .subview_mut(0, 0, 0, 0, 10, 3)
            .set_text(0, 0, "line1", CanvasTextStyle::default());
        canvas
            .subview_mut(0, 0, 0, 0, 10, 3)
            .set_text(0, 2, "line3", CanvasTextStyle::default());

        let vt = render_canvas_to_vt(&canvas, 10, 5);

        assert_eq!(vt.line(0).text(), "line1     ");
        assert_eq!(vt.line(1).text(), "          ");
        assert_eq!(vt.line(2).text(), "line3     ");
        assert_eq!(
            vt.cursor().row,
            2,
            "cursor should be on the last content row"
        );

        // clear and rerender with fewer lines
        let mut canvas2 = Canvas::new(10, 2);
        canvas2
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 0, "new1", CanvasTextStyle::default());
        canvas2
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 1, "new2", CanvasTextStyle::default());

        let vt = render_canvases_to_vt(&[&canvas, &canvas2], 10, 5);

        assert_eq!(vt.line(0).text(), "new1      ");
        assert_eq!(vt.line(1).text(), "new2      ");
        assert_eq!(
            vt.line(2).text(),
            "          ",
            "old line 3 should be cleared"
        );
        assert_eq!(vt.cursor().row, 1);
    }

    #[test]
    fn test_write_canvas_no_extra_blank_line() {
        let mut canvas = Canvas::new(10, 2);
        canvas
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 0, "first", CanvasTextStyle::default());
        canvas
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 1, "second", CanvasTextStyle::default());

        let vt = render_canvas_to_vt(&canvas, 10, 5);

        assert_eq!(vt.line(0).text(), "first     ");
        assert_eq!(vt.line(1).text(), "second    ");
        assert_eq!(vt.cursor().row, 1, "cursor stays on last content row");

        // clear and rerender
        let mut canvas2 = Canvas::new(10, 2);
        canvas2
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 0, "third", CanvasTextStyle::default());
        canvas2
            .subview_mut(0, 0, 0, 0, 10, 2)
            .set_text(0, 1, "fourth", CanvasTextStyle::default());

        let vt = render_canvases_to_vt(&[&canvas, &canvas2], 10, 5);

        assert_eq!(vt.line(0).text(), "third     ");
        assert_eq!(vt.line(1).text(), "fourth    ");
        assert_eq!(vt.cursor().row, 1);
    }

    #[test]
    fn test_borrowed_writers() {
        let mut stdout_buf: Vec<u8> = Vec::new();
        let mut stderr_buf: Vec<u8> = Vec::new();

        {
            let mut terminal = Terminal::new(
                Box::new(&mut stdout_buf),
                Box::new(&mut stderr_buf),
                Output::Stdout,
                false,
                true,
            )
            .unwrap();
            let canvas = Canvas::new(10, 1);
            terminal.write_canvas(&canvas).unwrap();
        }

        assert!(!stdout_buf.is_empty());
    }
}
