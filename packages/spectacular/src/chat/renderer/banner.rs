use super::{paint, RuntimeSelection, OPENING_BANNER_MIN_WIDTH};
use crate::terminal_style;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use unicode_width::UnicodeWidthStr;

/// Data needed to render the opening session banner without reading runtime state.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct OpeningBannerView {
    pub(super) version: String,
    pub(super) model: String,
    pub(super) reasoning: String,
    pub(super) directory: String,
    pub(super) session_id: String,
}

impl OpeningBannerView {
    /// Creates a banner view from the active runtime and working directory.
    pub(super) fn from_runtime(id: &str, runtime: &RuntimeSelection, directory: &Path) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            model: runtime.model.clone(),
            reasoning: runtime.reasoning.to_string(),
            directory: format_directory(directory),
            session_id: id.to_owned(),
        }
    }
}

/// Formats the opening banner as a fixed-width terminal block.
pub(super) fn format_opening_banner(view: &OpeningBannerView) -> String {
    let title = format!("Spectacular (v{})", view.version);
    let spacer = String::new();
    let model = format!("model:     {} {}", view.model, view.reasoning);
    let directory = format!("directory: {}", view.directory);
    let session = format!("session:   {}", view.session_id);
    let lines = [&title, &spacer, &model, &directory, &session];
    let content_width = lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "─".repeat(content_width + 2);
    let green = terminal_style::title_style();
    let mut rendered = vec![paint(green, format!("╭{horizontal}╮"))];
    rendered.push(format!(
        "{} {} {}",
        paint(green, "│"),
        paint(green, pad_banner_line(&title, content_width)),
        paint(green, "│")
    ));
    rendered.extend(lines.iter().skip(1).map(|line| {
        format!(
            "{} {} {}",
            paint(green, "│"),
            pad_banner_line(line, content_width),
            paint(green, "│")
        )
    }));
    rendered.push(paint(green, format!("╰{horizontal}╯")));
    rendered.join("\n")
}

/// Pads a banner row to the computed display width, accounting for Unicode width.
fn pad_banner_line(line: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(line));
    format!("{line}{}", " ".repeat(padding))
}

/// Formats a working directory using the current user's home directory when available.
fn format_directory(directory: &Path) -> String {
    format_directory_with_home(directory, home_dir().as_deref())
}

/// Formats a directory with an injected home path for deterministic tests.
pub(super) fn format_directory_with_home(directory: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return directory.display().to_string();
    };

    if directory == home {
        return "~".to_owned();
    }

    let Ok(relative) = directory.strip_prefix(home) else {
        return directory.display().to_string();
    };

    if relative.as_os_str().is_empty() {
        return "~".to_owned();
    }

    format!("~{}{}", MAIN_SEPARATOR, relative.display())
}

/// Resolves the current user's home directory across Windows and Unix-style environments.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut home = drive;
            home.push(path);
            Some(PathBuf::from(home)).filter(|path| !path.as_os_str().is_empty())
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
        })
}
