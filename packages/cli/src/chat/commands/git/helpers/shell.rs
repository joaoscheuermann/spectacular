#[cfg(windows)]
use std::path::{Path, PathBuf};
use tokio::process::Command;

const SHELL_SPECIAL_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', '"', '\'', '(', ')', '{', '}', '|', '&', ';', '<', '>', '`', '$', '\\',
];

/// Escapes a single argument for safe inclusion in a shell command string.
pub(super) fn shell_escape(arg: &str) -> String {
    if needs_quoting(arg) {
        quote(arg)
    } else {
        arg.to_owned()
    }
}

fn needs_quoting(arg: &str) -> bool {
    arg.chars().any(is_shell_special)
}

fn is_shell_special(character: char) -> bool {
    SHELL_SPECIAL_CHARS.contains(&character)
}

fn quote(arg: &str) -> String {
    format!("\"{}\"", arg.replace('"', "\\\""))
}

// Minimal shell detection reused from the terminal tool patterns.
#[derive(Clone, Debug)]
pub(super) enum ShellSpec {
    #[cfg(windows)]
    PowerShell { program: String },
    #[cfg(windows)]
    Cmd { program: String },
    #[cfg(not(windows))]
    Bash,
}

impl ShellSpec {
    pub(super) fn detect() -> Self {
        detect_shell()
    }

    pub(super) fn to_command(&self, command_text: &str) -> Command {
        match self {
            #[cfg(windows)]
            ShellSpec::PowerShell { program } => powershell_command(program, command_text),
            #[cfg(windows)]
            ShellSpec::Cmd { program } => cmd_command(program, command_text),
            #[cfg(not(windows))]
            ShellSpec::Bash => bash_command(command_text),
        }
    }
}

#[cfg(windows)]
fn detect_shell() -> ShellSpec {
    detect_windows_shell()
}

#[cfg(not(windows))]
fn detect_shell() -> ShellSpec {
    ShellSpec::Bash
}

#[cfg(windows)]
fn detect_windows_shell() -> ShellSpec {
    find_powershell().unwrap_or_else(cmd_shell)
}

#[cfg(windows)]
fn find_powershell() -> Option<ShellSpec> {
    path_powershell()
        .or_else(system_powershell)
        .map(|program| ShellSpec::PowerShell { program })
}

#[cfg(windows)]
fn path_powershell() -> Option<String> {
    ["pwsh", "powershell.exe"]
        .into_iter()
        .find(|program| is_in_path(program))
        .map(str::to_owned)
}

#[cfg(windows)]
fn system_powershell() -> Option<String> {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn cmd_shell() -> ShellSpec {
    ShellSpec::Cmd {
        program: std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned()),
    }
}

#[cfg(windows)]
fn powershell_command(program: &str, command_text: &str) -> Command {
    let mut command = Command::new(program);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        command_text,
    ]);
    command
}

#[cfg(windows)]
fn cmd_command(program: &str, command_text: &str) -> Command {
    let mut command = Command::new(program);
    command.args(["/C", command_text]);
    command
}

#[cfg(not(windows))]
fn bash_command(command_text: &str) -> Command {
    let mut command = Command::new("bash");
    command.args(["-lc", command_text]);
    command
}

#[cfg(windows)]
fn is_in_path(executable: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path_var).any(|directory| contains_executable(&directory, executable))
}

#[cfg(windows)]
fn contains_executable(directory: &Path, executable: &str) -> bool {
    if Path::new(executable).extension().is_some() {
        return directory.join(executable).is_file();
    }

    executable_extensions()
        .iter()
        .any(|extension| directory.join(format!("{executable}{extension}")).is_file())
}

#[cfg(windows)]
fn executable_extensions() -> Vec<String> {
    std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned(), ".CMD".to_owned(), ".BAT".to_owned()])
}
