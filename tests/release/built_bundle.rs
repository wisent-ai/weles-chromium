//! The browser a release build staged, run as Weles runs it: the release
//! worker hands this journey its output directory (`WISENT_OUTPUT_DIR`), the
//! staged executable is started for its identity and headless for one page,
//! and both are checked against what `browser-capabilities.json` declares.
//! The operations, their exit statuses and their output are kept in
//! `WISENT_TEST_EVIDENCE_DIR` when the worker names it.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn required(name: &str) -> String {
    let value = match env::var(name) {
        Ok(value) => value,
        Err(error) => panic!("{name} is required: the release worker's contract names it: {error}"),
    };
    assert!(!value.trim().is_empty(), "{name} must not be empty");
    value
}

/// The executable the release stages for `platform`, as `release/install.sh`
/// installs it.
fn staged_executable(output: &Path, platform: &str) -> PathBuf {
    match platform {
        "darwin-arm64" => output.join("stage/Chromium.app/Contents/MacOS/Chromium"),
        "linux-amd64" => output.join("stage/chromium/chrome"),
        _ => panic!("the release declares no browser for platform {platform}"),
    }
}

/// The `upstreamVersion` string `declared` states.
fn upstream_version(declared: &str) -> &str {
    let marker = "\"upstreamVersion\":";
    let Some(start) = declared.find(marker) else {
        panic!("browser-capabilities.json declares no upstreamVersion: {declared}");
    };
    let Some(quoted) = declared[start + marker.len()..].trim_start().strip_prefix('"') else {
        panic!("browser-capabilities.json states upstreamVersion as no string: {declared}");
    };
    match quoted.split('"').next().filter(|version| !version.trim().is_empty()) {
        Some(version) => version,
        None => panic!("browser-capabilities.json states upstreamVersion without a value: {declared}"),
    }
}

struct Evidence {
    report: String,
    file: Option<PathBuf>,
}

impl Evidence {
    fn run(&mut self, label: &str, command: &mut Command) -> Output {
        let result = match command.output() {
            Ok(result) => result,
            Err(error) => panic!("{label}: {command:?} could not run: {error}"),
        };
        self.report.push_str(&format!(
            "{label}: {command:?}\nstatus: {}\nstdout: {}\nstderr: {}\n",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
        self.keep();
        result
    }

    fn keep(&self) {
        if let Some(file) = &self.file {
            if let Err(error) = fs::write(file, &self.report) {
                panic!("the evidence {} could not be written: {error}", file.display());
            }
        }
    }
}

#[test]
fn the_staged_browser_reports_the_declared_version_and_renders_a_page() {
    let output = PathBuf::from(required("WISENT_OUTPUT_DIR"));
    let source = PathBuf::from(required("WISENT_SOURCE_DIR"));
    let platform = required("WISENT_PLATFORM");
    let file = match env::var_os("WISENT_TEST_EVIDENCE_DIR") {
        Some(base) => {
            let directory = PathBuf::from(base).join("built-bundle");
            if let Err(error) = fs::create_dir_all(&directory) {
                panic!("the evidence directory {} could not be made: {error}", directory.display());
            }
            Some(directory.join("report.txt"))
        }
        None => None,
    };
    let mut evidence = Evidence { report: format!("output: {}\nplatform: {platform}\n", output.display()), file };

    let declared = match fs::read_to_string(source.join("browser-capabilities.json")) {
        Ok(text) => text,
        Err(error) => panic!("browser-capabilities.json in {} could not be read: {error}", source.display()),
    };
    let version = upstream_version(&declared);

    let executable = staged_executable(&output, &platform);
    assert!(executable.is_file(), "the build staged no browser at {}", executable.display());

    let identity = evidence.run("staged browser identity", Command::new(&executable).arg("--version"));
    assert!(identity.status.success(), "the staged browser refused --version: {}", evidence.report);
    let said = String::from_utf8_lossy(&identity.stdout);
    assert!(said.contains(version), "the staged browser is not upstream {version}: {said}");

    // One page rendered headless proves the browser starts and lays out a
    // document, not only that it prints its name.
    let profile = output.join("built-bundle-profile");
    let page = evidence.run(
        "staged browser renders a page",
        Command::new(&executable)
            .arg("--headless")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--dump-dom")
            .arg("data:text/html,<title>weles</title><p id=built>rendered</p>"),
    );
    if let Err(error) = fs::remove_dir_all(&profile) {
        evidence.report.push_str(&format!("profile {} not removed: {error}\n", profile.display()));
    }
    assert!(page.status.success(), "the staged browser did not render a page: {}", evidence.report);
    let dom = String::from_utf8_lossy(&page.stdout);
    assert!(
        dom.contains("id=\"built\"") && dom.contains("rendered"),
        "the rendered document lacks the page's body: {dom}"
    );
    evidence.report.push_str("result: passed\n");
    evidence.keep();
}
