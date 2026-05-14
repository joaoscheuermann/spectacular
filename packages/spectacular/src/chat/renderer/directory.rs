use std::path::{Path, PathBuf, MAIN_SEPARATOR};

/// Formats a working directory using the current user's home directory when available.
pub(super) fn format_directory(directory: &Path) -> String {
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
