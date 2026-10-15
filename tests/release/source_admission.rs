//! Exercise the real patch entrypoint on an explicitly supplied Chromium checkout.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs};
mod source_state;

#[derive(Debug)]
enum Expectation {
    Applied,
    Dirty,
    Unfinished,
    Nested,
    BuildUnconfigured,
    ExtractionOccupied,
    ExtractionSymlink,
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    head: String,
    refs: String,
    status: String,
    worktree_diff: String,
    index_diff: String,
    untracked: Vec<String>,
    operations: Vec<(PathBuf, source_state::State)>,
}

struct Run {
    report: String,
}

impl Run {
    fn command(&mut self, cwd: &Path, program: &str, args: &[&str]) -> Result<Output, String> {
        let result = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|error| format!("{program} {args:?}: {error}"))?;
        self.report.push_str(&format!(
            "cwd: {}\ncommand: {program} {args:?}\nstatus: {}\nstdout:\n{}\nstderr:\n{}\n",
            cwd.display(),
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
        Ok(result)
    }

    fn git(&mut self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        let result = self.command(cwd, "git", args)?;
        if !result.status.success() {
            return Err(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        String::from_utf8(result.stdout).map_err(|error| error.to_string())
    }

    fn snapshot(&mut self, source: &Path) -> Result<Snapshot, String> {
        let mut untracked = Vec::new();
        let paths = self.git(
            source,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        for name in paths.split('\0').filter(|name| !name.is_empty()) {
            let path = source.join(name);
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path).map_err(|error| error.to_string())?;
                untracked.push(format!("{name}: symlink {target:?}"));
            } else {
                untracked.push(format!(
                    "{name}: {}",
                    self.git(source, &["hash-object", "--", name])?
                ));
            }
        }
        let git_dir = PathBuf::from(
            self.git(source, &["rev-parse", "--absolute-git-dir"])?
                .trim(),
        );
        Ok(Snapshot {
            head: self.git(source, &["rev-parse", "HEAD"])?,
            refs: self.git(source, &["show-ref"])?,
            status: self.git(
                source,
                &["status", "--porcelain=v1", "--untracked-files=all"],
            )?,
            worktree_diff: self.git(source, &["diff", "--binary"])?,
            index_diff: self.git(source, &["diff", "--cached", "--binary"])?,
            untracked,
            operations: source_state::capture(&git_dir)?,
        })
    }

    fn exercise(
        &mut self,
        repo: &Path,
        source: &Path,
        expected: Expectation,
        output: &Path,
    ) -> Result<(), String> {
        let declared = env::var("WELES_CHROMIUM_TEST_SOURCE_REVISION")
            .map_err(|error| format!("WELES_CHROMIUM_TEST_SOURCE_REVISION is required: {error}"))?;
        let observed = self.git(repo, &["rev-parse", "HEAD"])?;
        self.report
            .push_str(&format!("declared-source-revision: {declared}\n"));
        if declared.trim().is_empty() || observed.trim() != declared {
            return Err(format!(
                "candidate revision mismatch: declared {declared:?}, observed {observed:?}"
            ));
        }
        let status = self.git(repo, &["status", "--porcelain", "--untracked-files=all"])?;
        if !status.is_empty() {
            return Err(format!(
                "commit the product candidate before qualification: {status}"
            ));
        }
        self.git(
            repo,
            &[
                "hash-object",
                "apply.sh",
                "browser-capabilities.json",
                "release/build.sh",
                "tests/release/source_admission.rs",
                "tests/release/source_state.rs",
            ],
        )?;
        let root = PathBuf::from(self.git(source, &["rev-parse", "--show-toplevel"])?.trim())
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if repo.starts_with(&root) {
            return Err("Chromium test source must not contain the product checkout".into());
        }
        let before = self.snapshot(source)?;
        self.report.push_str(&format!("before-state: {before:?}\n"));
        let apply = repo.join("apply.sh");
        let path = |path: &Path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("non-UTF-8 test path {}", path.display()))
        };
        let result = if matches!(
            expected,
            Expectation::BuildUnconfigured
                | Expectation::ExtractionOccupied
                | Expectation::ExtractionSymlink
        ) {
            source_state::build_refusal(self, repo, source, output, &expected)?
        } else {
            self.command(repo, "bash", &[&path(&apply)?, &path(source)?])?
        };
        let after = self.snapshot(source)?;
        self.report.push_str(&format!("after-state: {after:?}\n"));
        let cause = match expected {
            Expectation::Applied => {
                if !result.status.success() {
                    return Err("real patch application failed; see retained command cause".into());
                }
                let descriptor = self.command(
                    repo,
                    "jq",
                    &["-er", ".forkPoint", "browser-capabilities.json"],
                )?;
                if !descriptor.status.success() {
                    return Err("could not read declared fork point".into());
                }
                let fork =
                    String::from_utf8(descriptor.stdout).map_err(|error| error.to_string())?;
                self.git(
                    source,
                    &["merge-base", "--is-ancestor", fork.trim(), "HEAD"],
                )?;
                let count = self
                    .git(
                        source,
                        &["rev-list", "--count", &format!("{}..HEAD", fork.trim())],
                    )?
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| error.to_string())?;
                let mut patches = Vec::new();
                for entry in
                    fs::read_dir(repo.join("patches")).map_err(|error| error.to_string())?
                {
                    let path = entry.map_err(|error| error.to_string())?.path();
                    if path
                        .extension()
                        .is_some_and(|extension| extension == "patch")
                    {
                        patches.push(path);
                    }
                }
                if count != patches.len() {
                    return Err("not every declared patch produced a commit".into());
                }
                if before.refs != after.refs || !after.status.is_empty() {
                    return Err(
                        "patch application changed an existing ref or left unfinished work".into(),
                    );
                }
                let detached = self.command(source, "git", &["symbolic-ref", "-q", "HEAD"])?;
                if detached.status.success() || !detached.stderr.is_empty() {
                    return Err("patch application did not establish detached HEAD".into());
                }
                return Ok(());
            }
            Expectation::Dirty => "uncommitted work",
            Expectation::Unfinished => "unfinished Git operation",
            Expectation::Nested => "must name the checkout root",
            Expectation::BuildUnconfigured => "no configured build graph",
            Expectation::ExtractionOccupied | Expectation::ExtractionSymlink => {
                "package extraction directory already exists"
            }
        };
        if result.status.success() || !String::from_utf8_lossy(&result.stderr).contains(cause) {
            return Err(format!(
                "expected refusal {cause}; see retained command result"
            ));
        }
        if before != after {
            return Err("refused admission changed the real checkout".into());
        }
        Ok(())
    }
}

#[test]
#[ignore = "Requires a clean dedicated Chromium source holding the declared fork point"]
fn applies_real_patch_series() {
    source_state::qualify(Expectation::Applied);
}

#[test]
#[ignore = "Requires a dedicated Chromium source with real uncommitted work"]
fn preserves_dirty_source() {
    source_state::qualify(Expectation::Dirty);
}

#[test]
#[ignore = "Requires a dedicated Chromium source with an unfinished Git operation"]
fn preserves_unfinished_operation() {
    source_state::qualify(Expectation::Unfinished);
}

#[test]
#[ignore = "Requires a directory nested inside the dedicated Chromium source"]
fn refuses_nested_source() {
    source_state::qualify(Expectation::Nested);
}

#[test]
#[ignore = "Requires an unconfigured real Chromium source and native WISENT_PLATFORM"]
fn preserves_unconfigured_build_source() {
    source_state::qualify(Expectation::BuildUnconfigured);
}

#[test]
#[ignore = "Requires configured dedicated Linux Chromium source and native packaging tools"]
fn preserves_occupied_extraction_source() {
    source_state::qualify(Expectation::ExtractionOccupied);
}

#[test]
#[ignore = "Requires configured dedicated Linux Chromium source and native packaging tools"]
fn preserves_dangling_extraction_source() {
    source_state::qualify(Expectation::ExtractionSymlink);
}
