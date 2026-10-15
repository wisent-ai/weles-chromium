use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

pub(super) fn build_refusal(
    run: &mut super::Run,
    repo: &Path,
    source: &Path,
    output: &Path,
    expected: &super::Expectation,
) -> Result<std::process::Output, String> {
    use super::Expectation;
    let occupied = matches!(
        expected,
        Expectation::ExtractionOccupied | Expectation::ExtractionSymlink
    );
    if occupied {
        if std::env::var("WISENT_PLATFORM").as_deref() != Ok("linux-amd64") {
            return Err("occupied extraction qualification requires native linux-amd64".into());
        }
        if !source.join("out/Weles/build.ninja").is_file() {
            return Err(
                "occupied extraction qualification requires configured real Chromium source".into(),
            );
        }
    } else if source.join("out/Weles/build.ninja").exists() {
        return Err("this refusal journey requires an unconfigured real source".into());
    }
    let extraction = output.join("linux-package");
    if matches!(expected, Expectation::ExtractionSymlink) {
        std::os::unix::fs::symlink("absent-extraction-target", &extraction)
            .map_err(|error| error.to_string())?;
    } else if occupied {
        fs::create_dir(&extraction).map_err(|error| error.to_string())?;
        fs::write(
            extraction.join("retained-input"),
            source.as_os_str().as_encoded_bytes(),
        )
        .map_err(|error| error.to_string())?;
    }
    let mut before = Vec::new();
    visit(output, Path::new("linux-package"), &mut before)?;
    run.report
        .push_str(&format!("extraction-before: {before:?}\n"));
    let utf8 = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("non-UTF-8 test path {}", path.display()))
    };
    let result = run.command(
        repo,
        "env",
        &[
            &format!("WISENT_SOURCE_DIR={}", utf8(repo)?),
            &format!("WISENT_OUTPUT_DIR={}", utf8(output)?),
            &format!("WELES_CHROMIUM_SRC={}", utf8(source)?),
            "bash",
            "release/build.sh",
        ],
    );
    let mut after = Vec::new();
    visit(output, Path::new("linux-package"), &mut after)?;
    run.report
        .push_str(&format!("extraction-after: {after:?}\n"));
    if before != after {
        return Err("refused build changed retained extraction state".into());
    }
    if occupied {
        if matches!(expected, Expectation::ExtractionSymlink) {
            fs::remove_file(&extraction).map_err(|error| error.to_string())?;
        } else {
            fs::remove_dir_all(&extraction).map_err(|error| error.to_string())?;
        }
    }
    result
}

/// One source-admission journey against the dedicated Chromium source
/// WELES_CHROMIUM_TEST_SOURCE names, with its operations kept under the
/// product checkout's .build/source-admission.
pub(super) fn qualify(expected: super::Expectation) {
    let source =
        env::var("WELES_CHROMIUM_TEST_SOURCE").expect("WELES_CHROMIUM_TEST_SOURCE is required");
    let source = PathBuf::from(source)
        .canonicalize()
        .expect("real Chromium source");
    // Built by Cargo, the journeys' package is tests/release inside the
    // product checkout; built by rustc as the README shows, the journey runs
    // from the checkout itself.
    let repo = match option_env!("CARGO_MANIFEST_DIR") {
        Some(package) => PathBuf::from(package).join("../.."),
        None => env::current_dir().expect("product checkout"),
    }
    .canonicalize()
    .expect("product root");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let output = repo
        .join(".build/source-admission")
        .join(format!("{}-{stamp}", std::process::id()));
    fs::create_dir_all(&output).expect("retained report directory");
    let mut run = super::Run {
        report: format!("source: {}\nexpectation: {expected:?}\n", source.display()),
    };
    let result = run.exercise(&repo, &source, expected, &output);
    run.report.push_str(&format!("result: {result:?}\n"));
    fs::write(output.join("report.txt"), run.report).expect("retain command outcomes");
    result.expect("real Chromium source admission; inspect retained report");
}
