//! Installing and updating script packages from git repositories.
//!
//! A package is a folder with an `init.luau`; installing one clones its
//! repository into the `packages` folder with the system's `git`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use crate::profile::valid_name;

/// The name a package gets from the address it is installed from: the last
/// part of the address, without `.git`. `None` if that is not a usable
/// folder name.
pub fn name_from_address(address: &str) -> Option<String> {
    let last = address
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    valid_name(name).then(|| name.to_owned())
}

/// How a git run ended: the package's name, or what went wrong.
pub type Outcome = Result<String, String>;

/// Clones `address` into `packages/name` in the background.
pub fn install(packages: &Path, name: &str, address: &str) -> Receiver<Outcome> {
    let target = packages.join(name);
    let mut git = Command::new("git");
    git.args(["clone", "--depth", "1", "--"])
        .arg(address)
        .arg(&target);
    let folder = packages.to_owned();
    run(
        name,
        git,
        move || {
            std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
            if target.exists() {
                return Err("a package with that name is already installed".to_owned());
            }
            Ok(())
        },
        check_package,
    )
}

/// Brings the package `name` up to date with its repository.
pub fn update(packages: &Path, name: &str) -> Receiver<Outcome> {
    let mut git = Command::new("git");
    git.arg("-C")
        .arg(packages.join(name))
        .args(["pull", "--ff-only"]);
    run(name, git, || Ok(()), |_| Ok(()))
}

/// A package without an `init.luau` would load nothing.
fn check_package(folder: PathBuf) -> Result<(), String> {
    if folder.join("init.luau").is_file() {
        Ok(())
    } else {
        Err("it was downloaded, but has no init.luau, so it is not a Muddie plugin".to_owned())
    }
}

/// Runs `git` on a thread, after `before` and followed by `after`, and
/// reports through the returned channel.
fn run(
    name: &str,
    mut git: Command,
    before: impl FnOnce() -> Result<(), String> + Send + 'static,
    after: fn(PathBuf) -> Result<(), String>,
) -> Receiver<Outcome> {
    let (sender, receiver) = channel();
    let name = name.to_owned();
    std::thread::spawn(move || {
        let folder = git.get_args().last().map(PathBuf::from).unwrap_or_default();
        let outcome = before().and_then(|()| {
            let output = git
                .stdin(Stdio::null())
                // Never stop to ask for a password: fail instead.
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .map_err(|error| format!("could not run git: {error}"))?;
            if output.status.success() {
                after(folder)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(stderr
                    .lines()
                    .last()
                    .unwrap_or("git failed")
                    .trim()
                    .to_owned())
            }
        });
        let _ = sender.send(outcome.map(|()| name));
    });
    receiver
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn package_name_comes_from_the_address() {
        for (address, name) in [
            (
                "https://example.org/ana/combat-pack.git",
                Some("combat-pack"),
            ),
            ("https://example.org/ana/mapper/", Some("mapper")),
            ("git@example.org:ana/tools.git", Some("tools")),
            ("/home/ana/code/my_pack", Some("my_pack")),
            ("https://example.org/ana/bad.name", None),
            ("", None),
        ] {
            assert_eq!(name_from_address(address).as_deref(), name, "{address}");
        }
    }

    fn git(folder: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(folder)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.org"])
            .args(arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?}");
    }

    /// Installs from, and updates against, a repository made on the spot.
    #[test]
    fn installs_and_updates_from_a_local_repository() {
        let dir = std::env::temp_dir().join(format!("muddie-packages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let source = dir.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("init.luau"), "echo('v1')").unwrap();
        git(&source, &["init", "-q"]);
        git(&source, &["add", "."]);
        git(&source, &["commit", "-q", "-m", "first"]);

        let wait =
            |outcome: Receiver<Outcome>| outcome.recv_timeout(Duration::from_secs(30)).unwrap();
        let packages = dir.join("packages");
        let address = source.display().to_string();
        assert_eq!(
            wait(install(&packages, "pack", &address)),
            Ok("pack".to_owned())
        );
        assert_eq!(
            std::fs::read_to_string(packages.join("pack/init.luau")).unwrap(),
            "echo('v1')"
        );
        assert!(
            wait(install(&packages, "pack", &address))
                .unwrap_err()
                .contains("already installed")
        );

        std::fs::write(source.join("init.luau"), "echo('v2')").unwrap();
        git(&source, &["commit", "-q", "-am", "second"]);
        assert_eq!(wait(update(&packages, "pack")), Ok("pack".to_owned()));
        assert_eq!(
            std::fs::read_to_string(packages.join("pack/init.luau")).unwrap(),
            "echo('v2')"
        );

        // A repository that is not a package is reported as such.
        std::fs::remove_file(source.join("init.luau")).unwrap();
        std::fs::write(source.join("readme"), "x").unwrap();
        git(&source, &["add", "-A"]);
        git(&source, &["commit", "-q", "-m", "third"]);
        assert!(
            wait(install(&packages, "other", &address))
                .unwrap_err()
                .contains("no init.luau")
        );
        assert!(wait(update(&packages, "missing")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
