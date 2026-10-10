use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub(super) enum State {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}

pub(super) fn capture(git_dir: &Path) -> Result<Vec<(PathBuf, State)>, String> {
    let mut entries = Vec::new();
    for operation in [
        "rebase-apply",
        "rebase-merge",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "sequencer",
    ] {
        visit(git_dir, Path::new(operation), &mut entries)?;
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

fn visit(root: &Path, relative: &Path, entries: &mut Vec<(PathBuf, State)>) -> Result<(), String> {
    let path = root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("inspect Git operation {}: {error}", path.display())),
    };
    let state = if metadata.file_type().is_symlink() {
        State::Link(
            fs::read_link(&path)
                .map_err(|error| format!("read Git operation link {}: {error}", path.display()))?,
        )
    } else if metadata.is_dir() {
        for entry in fs::read_dir(&path)
            .map_err(|error| format!("list Git operation {}: {error}", path.display()))?
        {
            let entry = entry.map_err(|error| format!("list {}: {error}", path.display()))?;
            visit(root, &relative.join(entry.file_name()), entries)?;
        }
        State::Directory
    } else if metadata.is_file() {
        State::File(
            fs::read(&path)
                .map_err(|error| format!("read Git operation {}: {error}", path.display()))?,
        )
    } else {
        return Err(format!(
            "unsupported Git operation entry {}",
            path.display()
        ));
    };
    entries.push((relative.to_path_buf(), state));
    Ok(())
}
