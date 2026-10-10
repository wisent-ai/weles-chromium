//! Real qualified Chromium archive adoption through the delivery endpoint.
//! Supply the normal WISENT_RELEASE_* and WISENT_VERSION/PLATFORM/PRODUCT inputs.
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Run {
    root: PathBuf,
    home: PathBuf,
    output: PathBuf,
    report: String,
}
impl Run {
    fn command(&mut self, label: &str, command: &mut Command) -> Output {
        let result = command.output().expect("execute actual release operation");
        self.report.push_str(&format!(
            "{label}: {command:?}\nstatus: {}\nstdout: {}\nstderr: {}\n",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
        fs::write(self.output.join("report.txt"), &self.report).expect("retain operation result");
        result
    }
    fn install(&mut self, digest: Option<&str>) -> Output {
        let mut command = Command::new("bash");
        command
            .arg(self.root.join("release/install.sh"))
            .env("HOME", &self.home)
            .env_remove("WELES_CHROMIUM_DIR");
        if let Some(digest) = digest {
            command.env("WISENT_RELEASE_SHA256", digest);
        }
        self.command("install", &mut command)
    }
}
fn required(name: &str) -> String {
    let value = match env::var(name) {
        Ok(value) => value,
        Err(error) => panic!("{name} is required: {error}"),
    };
    assert!(!value.trim().is_empty(), "{name} must not be empty");
    value
}
#[test]
#[ignore = "Requires the actual qualified darwin-arm64 Chromium release archive"]
fn installs_real_bundle_and_refuses_conflicting_archive_digest() {
    let root = env::current_dir().expect("product checkout");
    assert!(root.join("release/install.sh").is_file());
    let version = required("WISENT_VERSION");
    let platform = required("WISENT_PLATFORM");
    let product = required("WISENT_PRODUCT");
    let uri = required("WISENT_RELEASE_URI");
    let digest = required("WISENT_RELEASE_SHA256");
    let archive =
        fs::canonicalize(required("WISENT_RELEASE_ARCHIVE")).expect("real release archive");
    assert_eq!(product, "weles-chromium");
    assert_eq!(platform, "darwin-arm64");
    assert_eq!(
        uri,
        format!("stado://releases/{product}/{version}/{platform}/release.tar.gz")
    );
    assert!(!version.starts_with('.'));
    assert!(version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c)));
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = root
        .join(".build/release-installation")
        .join(format!("{}-{unique}", std::process::id()));
    let home = output.join("home");
    fs::create_dir_all(&home).expect("isolated install home");
    let mut run = Run {
        root: root.clone(),
        home: home.clone(),
        output,
        report: format!(
            "archive: {}\nselected_digest: {digest}\n",
            archive.display()
        ),
    };
    let revision = run.command(
        "source revision",
        Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"]),
    );
    assert!(revision.status.success());
    let installed = run.install(None);
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let directory = home.join(".local/share/weles-chromium").join(&version);
    let receipt = fs::read(directory.join(".weles-release")).expect("persisted receipt");
    assert_eq!(
        receipt,
        format!("release_uri={uri}\narchive_sha256={digest}\nplatform={platform}\n").as_bytes()
    );
    let executable = directory.join("Chromium.app/Contents/MacOS/Chromium");
    let identity = run.command(
        "real installed browser identity",
        Command::new(&executable).arg("--version"),
    );
    assert!(identity.status.success());
    assert!(String::from_utf8_lossy(&identity.stdout).contains(&version));
    let hash = run.command(
        "installed executable digest",
        Command::new("shasum").args(["-a", "256"]).arg(&executable),
    );
    assert!(hash.status.success());
    let repeated = run.install(None);
    assert!(repeated.status.success());
    assert_eq!(fs::read(directory.join(".weles-release")).unwrap(), receipt);
    let different = run.command(
        "different real input digest",
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(root.join("release/install.sh")),
    );
    assert!(different.status.success());
    let text = String::from_utf8(different.stdout).unwrap();
    let other_digest = text.split_whitespace().next().unwrap();
    assert_ne!(other_digest, digest);
    let refused = run.install(Some(other_digest));
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("archive checksum mismatch"));
    assert_eq!(fs::read(directory.join(".weles-release")).unwrap(), receipt);
    let lock = directory
        .parent()
        .unwrap()
        .join(format!(".install-{version}.lock"));
    fs::create_dir(&lock).expect("reserve the isolated destination for another installer");
    let locked = run.install(None);
    assert!(!locked.status.success());
    assert!(String::from_utf8_lossy(&locked.stderr).contains("another installation owns"));
    assert!(
        lock.is_dir(),
        "refused installer removed another operation's lock"
    );
    assert_eq!(fs::read(directory.join(".weles-release")).unwrap(), receipt);
    fs::remove_dir(&lock).expect("release test-owned reservation");
    let conflicting_receipt =
        format!("release_uri={uri}\narchive_sha256={other_digest}\nplatform={platform}\n");
    fs::write(directory.join(".weles-release"), &conflicting_receipt).unwrap();
    let conflict = run.install(None);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("installed receipt conflicts"));
    assert_eq!(
        fs::read(directory.join(".weles-release")).unwrap(),
        conflicting_receipt.as_bytes()
    );
    assert!(
        !lock.exists(),
        "failed installation retained its own reservation"
    );
    fs::write(directory.join(".weles-release"), &receipt).unwrap();
    let recovered = run.install(None);
    assert!(
        recovered.status.success(),
        "delivery after a refused installation must succeed: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(fs::read(directory.join(".weles-release")).unwrap(), receipt);
    let after = run.command(
        "executable after refusal",
        Command::new("shasum").args(["-a", "256"]).arg(&executable),
    );
    assert!(after.status.success());
    assert_eq!(after.stdout, hash.stdout);
    fs::remove_dir_all(home).expect("remove only the isolated installation");
    run.report.push_str("result: passed\n");
    fs::write(run.output.join("report.txt"), &run.report).unwrap();
}
