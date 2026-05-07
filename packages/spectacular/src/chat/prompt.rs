use crate::chat::paste_burst::{CharDecision, FlushResult, PasteBurst};
use crate::chat::renderer::{dim_style, paint, selection_style, user_style, Renderer};
use crate::chat::ChatError;
use anstyle::RgbColor;
use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::queue;
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType};
use spectacular_commands::{
    fuzzy_filter, fuzzy_rank, parse_line, CommandRegistry, CompletionCommandSpec,
    CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueSource, ParseOutcome,
};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

const DEFAULT_TERMINAL_WIDTH: u16 = 80;
const MAX_SUGGESTIONS: usize = 8;
const PROMPT_WIDTH: u16 = 2;
const MISSING_ORANGE: RgbColor = RgbColor(251, 191, 36);

include!("prompt/editor_types.rs");
include!("prompt/editor.rs");
include!("prompt/terminal.rs");
include!("prompt/state.rs");
include!("prompt/layout.rs");
include!("prompt/completion.rs");
include!("prompt/completion_fields.rs");
include!("prompt/navigation.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_commands::{Command, CommandControl, CommandFuture};

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_support.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_completion.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_editing.rs"
    ));
}
