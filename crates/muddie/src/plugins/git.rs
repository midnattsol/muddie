//! Downloading plugins from git repositories, and bringing them up to
//! date, with the system's `git`, into the folder of the plugins
//! downloaded.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use crate::profile::valid_name;

/// The name a plugin gets from the address it is installed from: the last
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

/// How a git run ended: the plugin's name, or what went wrong.
pub type Outcome = Result<String, String>;

/// Clones `address` into `folder/name` in the background.
pub fn download(folder: &Path, name: &str, address: &str) -> Receiver<Outcome> {
    let target = folder.join(name);
    let mut git = Command::new("git");
    git.args(["clone", "--depth", "1", "--"])
        .arg(address)
        .arg(&target);
    let folder = folder.to_owned();
    run(
        name,
        git,
        move || {
            std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
            if target.exists() {
                return Err("a plugin of that name is downloaded already".to_owned());
            }
            Ok(())
        },
        check_plugin,
    )
}

/// Brings the plugin `name` up to date with its repository.
pub fn update(folder: &Path, name: &str) -> Receiver<Outcome> {
    let mut git = Command::new("git");
    git.arg("-C")
        .arg(folder.join(name))
        .args(["pull", "--ff-only"]);
    run(name, git, || Ok(()), |_| Ok(()))
}

/// A repository with no script would install nothing: it is not one.
fn check_plugin(folder: PathBuf) -> Result<(), String> {
    match super::Plugin::from_folder(&folder, super::Origin::Git) {
        Some(_) => Ok(()),
        None => {
            let _ = std::fs::remove_dir_all(&folder);
            Err("it has no .luau script, so it is not a Muddie plugin".to_owned())
        }
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
    fn a_plugin_is_named_after_its_address() {
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
        let dir = std::env::temp_dir().join(format!("muddie-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let source = dir.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("init.luau"), "echo('v1')").unwrap();
        git(&source, &["init", "-q"]);
        git(&source, &["add", "."]);
        git(&source, &["commit", "-q", "-m", "first"]);

        let wait =
            |outcome: Receiver<Outcome>| outcome.recv_timeout(Duration::from_secs(30)).unwrap();
        let folder = dir.join("plugins");
        let address = source.display().to_string();
        assert_eq!(
            wait(download(&folder, "pack", &address)),
            Ok("pack".to_owned())
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("pack/init.luau")).unwrap(),
            "echo('v1')"
        );
        assert!(
            wait(download(&folder, "pack", &address))
                .unwrap_err()
                .contains("downloaded already")
        );

        std::fs::write(source.join("init.luau"), "echo('v2')").unwrap();
        git(&source, &["commit", "-q", "-am", "second"]);
        assert_eq!(wait(update(&folder, "pack")), Ok("pack".to_owned()));
        assert_eq!(
            std::fs::read_to_string(folder.join("pack/init.luau")).unwrap(),
            "echo('v2')"
        );

        // A repository that is not a plugin is said to be none, and
        // nothing of it is kept.
        std::fs::remove_file(source.join("init.luau")).unwrap();
        std::fs::write(source.join("readme"), "x").unwrap();
        git(&source, &["add", "-A"]);
        git(&source, &["commit", "-q", "-m", "third"]);
        assert!(
            wait(download(&folder, "other", &address))
                .unwrap_err()
                .contains("no .luau script")
        );
        assert!(wait(update(&folder, "missing")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
