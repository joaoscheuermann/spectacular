use tokio::process::Command;

/// Description of the host shell used to execute terminal tool commands.
#[derive(Clone, Debug)]
pub(crate) enum ShellSpec {
    #[cfg(windows)]
    PowerShell { program: String },
    #[cfg(windows)]
    Cmd { program: String },
    #[cfg(not(windows))]
    Bash,
}

impl ShellSpec {
    /// Detects the preferred host shell for the current platform.
    pub(crate) fn detect() -> Self {
        #[cfg(windows)]
        {
            windows_shell_spec()
        }

        #[cfg(not(windows))]
        {
            Self::Bash
        }
    }

    /// Builds a Tokio command that runs the supplied shell command text.
    pub(crate) fn command(&self, command_text: &str) -> Command {
        match self {
            #[cfg(windows)]
            Self::PowerShell { program } => {
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
            Self::Cmd { program } => {
                let mut command = Command::new(program);
                command.args(["/C", command_text]);
                command
            }
            #[cfg(not(windows))]
            Self::Bash => {
                let mut command = Command::new("bash");
                command.args(["-lc", command_text]);
                command
            }
        }
    }
}

#[cfg(windows)]
/// Chooses the best available Windows shell using PATH, SystemRoot, and ComSpec fallbacks.
fn windows_shell_spec() -> ShellSpec {
    if executable_in_path("pwsh") {
        return ShellSpec::PowerShell {
            program: "pwsh".to_owned(),
        };
    }

    if executable_in_path("powershell.exe") {
        return ShellSpec::PowerShell {
            program: "powershell.exe".to_owned(),
        };
    }

    let system_powershell = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.is_file());
    if let Some(program) = system_powershell {
        return ShellSpec::PowerShell {
            program: program.to_string_lossy().into_owned(),
        };
    }

    ShellSpec::Cmd {
        program: std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned()),
    }
}

#[cfg(windows)]
/// Reports whether a command executable can be found on Windows PATH.
fn executable_in_path(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let has_extension = std::path::Path::new(command).extension().is_some();
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned(), ".CMD".to_owned(), ".BAT".to_owned()]);

    std::env::split_paths(&path).any(|directory| {
        if has_extension && directory.join(command).is_file() {
            return true;
        }

        extensions
            .iter()
            .any(|extension| directory.join(format!("{command}{extension}")).is_file())
    })
}
