use std::fs;
use std::path::Path;
use std::process::Command;

const ROOT_REFS: &[&str] = &["AUTO_MERGE", "MERGE_AUTOSTASH"];

/// Preserve merge root refs when Git stores them in reftable rather than files.
pub(super) fn preserve<T>(
    directory: &Path,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let backend = Command::new("git")
        .args(["rev-parse", "--show-ref-format"])
        .output()
        .map_err(|error| format!("failed to inspect reference storage: {error}"))?;
    if !backend.status.success() {
        return Err("failed to inspect reference storage".to_owned());
    }
    // Older Git versions echo the unknown option and only support loose refs.
    if backend.stdout != b"reftable\n" {
        return operation();
    }
    let originals = ROOT_REFS
        .iter()
        .map(|&name| read(name).map(|oid| (name, oid)))
        .collect::<Result<Vec<_>, _>>()?;
    if originals.iter().all(|(_, oid)| oid.is_none()) && !directory.join("MERGE_HEAD").is_file() {
        return operation();
    }
    let recovery = tempfile::Builder::new()
        .prefix("git-std-merge-refs-")
        .tempdir_in(directory)
        .map_err(|error| format!("failed to protect merge references: {error}"))?;
    for (name, oid) in &originals {
        if let Some(oid) = oid {
            fs::write(recovery.path().join(name), format!("{oid}\n"))
                .map_err(|error| format!("failed to protect {name}: {error}"))?;
        }
    }
    // Persist OIDs before removing refs so failures and process termination retain them.
    let recovery = recovery.keep();
    let capture = originals.iter().try_for_each(|(name, oid)| {
        if let Some(oid) = oid {
            update(name, None, Some(oid))
        } else {
            Ok(())
        }
    });
    let did_run = capture.is_ok();
    let result = capture.and_then(|()| operation());
    let mut failures = Vec::new();
    for (name, original) in &originals {
        let restored = (|| {
            let current = read(name)?;
            if current == *original {
                return Ok(());
            }
            if current.is_some() && !(did_run && *name == "AUTO_MERGE") {
                return Err(format!("refusing to replace merge reference {name}"));
            }
            update(name, original.as_deref(), current.as_deref())
        })();
        if let Err(error) = restored {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        for (name, oid) in &originals {
            if oid.is_some()
                && let Err(error) = fs::remove_file(recovery.join(name))
            {
                failures.push(format!("failed to remove recovered {name} record: {error}"));
            }
        }
        if failures.is_empty()
            && let Err(error) = fs::remove_dir(&recovery)
        {
            failures.push(format!(
                "failed to remove reference recovery directory: {error}"
            ));
        }
    }
    if failures.is_empty() {
        result
    } else {
        let restore = format!(
            "{}; merge reference recovery directory: {}",
            failures.join("; "),
            recovery.display()
        );
        match result {
            Ok(_) => Err(restore),
            Err(original) => Err(format!("{original}; {restore}")),
        }
    }
}

fn read(name: &str) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", name])
        .output()
        .map_err(|error| format!("failed to read merge reference {name}: {error}"))?;
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(format!(
            "failed to read merge reference {name}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let oid = String::from_utf8(output.stdout)
        .map_err(|error| format!("invalid merge reference {name}: {error}"))?;
    Ok(Some(oid.trim().to_owned()))
}

fn update(name: &str, original: Option<&str>, current: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("git");
    command.args(["update-ref", "--no-deref"]);
    match original {
        Some(oid) => {
            command.args([name, oid, current.unwrap_or("")]);
        }
        None => {
            let Some(oid) = current else {
                return Ok(());
            };
            command.args(["-d", name, oid]);
        }
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to update merge reference {name}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to update merge reference {name}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
