use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub(super) fn resolve(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let path_ext = std::env::var_os("PATHEXT");
    resolve_in(
        name,
        &path,
        path_ext.as_deref(),
        OsStr::new(std::env::consts::EXE_SUFFIX),
        cfg!(windows),
    )
}

fn resolve_in(
    name: &str,
    path: &OsStr,
    path_ext: Option<&OsStr>,
    executable_suffix: &OsStr,
    is_windows: bool,
) -> Option<String> {
    let names = executable_names(name, path_ext, executable_suffix, is_windows);

    std::env::split_paths(path)
        .flat_map(|directory| candidates_in(&directory, &names))
        .find(|candidate| is_executable(candidate))
        .and_then(|candidate| std::fs::canonicalize(candidate).ok())
        .map(|candidate| candidate.to_string_lossy().into_owned())
}

fn candidates_in(directory: &Path, names: &[OsString]) -> Vec<PathBuf> {
    names.iter().map(|name| directory.join(name)).collect()
}

fn executable_names(
    name: &str,
    path_ext: Option<&OsStr>,
    executable_suffix: &OsStr,
    is_windows: bool,
) -> Vec<OsString> {
    if !is_windows || Path::new(name).extension().is_some() {
        return vec![OsString::from(name)];
    }

    let mut extensions: Vec<String> = Vec::new();
    let suffix = executable_suffix.to_string_lossy();
    if !suffix.is_empty() {
        extensions.push(suffix.into_owned());
    }
    let configured = path_ext
        .and_then(OsStr::to_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(".COM;.EXE;.BAT;.CMD");
    for extension in configured
        .split(';')
        .filter(|extension| !extension.is_empty())
    {
        if !extensions
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(extension))
        {
            extensions.push(extension.to_string());
        }
    }

    extensions
        .into_iter()
        .map(|extension| OsString::from(format!("{name}{extension}")))
        .collect()
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `path` is a live, NUL-terminated C string and `faccessat`
        // only reads it. AT_EACCESS matches the credentials used by exec.
        unsafe { libc::faccessat(libc::AT_FDCWD, path.as_ptr(), libc::X_OK, libc::AT_EACCESS) == 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_executable_names_include_suffix_and_path_ext_candidates() {
        assert_eq!(
            executable_names(
                "cargo",
                Some(OsStr::new(".COM;.EXE;.CMD")),
                OsStr::new(".exe"),
                true,
            ),
            vec![
                OsString::from("cargo.exe"),
                OsString::from("cargo.COM"),
                OsString::from("cargo.CMD"),
            ]
        );
    }

    #[test]
    fn non_windows_executable_name_is_unchanged() {
        assert_eq!(
            executable_names("cargo", None, OsStr::new(""), false),
            vec![OsString::from("cargo")]
        );
    }

    #[test]
    fn windows_resolver_finds_an_executable_with_a_path_ext_suffix() {
        let directory = tempfile::tempdir().expect("tool directory");
        let executable = directory.path().join("cargo.EXE");
        std::fs::write(&executable, b"fixture").expect("fake executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = executable.metadata().expect("metadata").permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&executable, permissions).expect("executable permissions");
        }
        let path = std::env::join_paths([directory.path()]).expect("PATH");

        let resolved = resolve_in(
            "cargo",
            &path,
            Some(OsStr::new(".EXE;.CMD")),
            OsStr::new(""),
            true,
        );

        assert_eq!(
            resolved,
            Some(
                std::fs::canonicalize(executable)
                    .expect("canonical executable")
                    .to_string_lossy()
                    .into_owned()
            )
        );
    }
}
