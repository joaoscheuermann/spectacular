mod clipboard;
mod event;
mod paste_burst;
mod root;
mod shell;

pub use clipboard::{system_clipboard, ClipboardError, ClipboardService, SystemClipboard};
pub(crate) use event::effects_with_clipboard_and_paste;
pub use event::{
    effects, effects_with_clipboard, timer_tick_effects, EventEffect, MAX_PASTE_BYTES,
    SPINNER_TICK_INTERVAL,
};
pub(crate) use paste_burst::PasteBurst;
pub use root::{merge_controller_state_update, Root, RootProps};
pub use shell::{Intent, Shell};
