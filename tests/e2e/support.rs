use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

pub(crate) struct Machine {
    home: TempDir,
}

impl Machine {
    pub(crate) fn new() -> Self {
        let home = tempfile::tempdir().expect("create a temporary home directory");
        fs::write(
            home.path().join(".gitconfig"),
            "[user]\n\tname = Test\n\temail = test@example.com\n[init]\n\tdefaultBranch = master\n",
        )
        .expect("write the test git config");
        Self { home }
    }

    pub(crate) fn home(&self) -> &Path {
        self.home.path()
    }

    pub(crate) fn settings_file(&self) -> PathBuf {
        self.home().join(".claude/settings.json")
    }

    pub(crate) fn write_settings(&self, settings: &Value) {
        self.write_settings_text(&(serde_json::to_string_pretty(settings).unwrap() + "\n"));
    }

    pub(crate) fn settings(&self) -> Value {
        serde_json::from_str(&self.settings_text()).expect("settings.json is JSON")
    }

    pub(crate) fn write_file(&self, relative: &str, text: &str) {
        let path = self.home().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
    }

    pub(crate) fn read_file(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.home().join(relative)).ok()
    }

    pub(crate) fn settings_text(&self) -> String {
        fs::read_to_string(self.settings_file()).expect("read settings.json")
    }

    pub(crate) fn write_settings_text(&self, text: &str) {
        let path = self.settings_file();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
    }

    pub(crate) fn backups(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.home().join(".cc-sync/backups")) else {
            return Vec::new();
        };
        entries
            .map(|entry| fs::read_to_string(entry.unwrap().path().join("settings.json")).unwrap())
            .collect()
    }

    pub(crate) fn cc_sync(&self, args: &[&str]) -> Output {
        isolated(Command::new(env!("CARGO_BIN_EXE_cc-sync")), self.home())
            .args(args)
            .output()
            .expect("run cc-sync")
    }

    pub(crate) fn cc_sync_in_config_dir(&self, config_dir: &str, args: &[&str]) -> Output {
        isolated(Command::new(env!("CARGO_BIN_EXE_cc-sync")), self.home())
            .env("CLAUDE_CONFIG_DIR", self.home().join(config_dir))
            .args(args)
            .output()
            .expect("run cc-sync")
    }

    pub(crate) fn cc_sync_with_input(&self, args: &[&str], input: &str) -> Output {
        let mut child = isolated(Command::new(env!("CARGO_BIN_EXE_cc-sync")), self.home())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start cc-sync");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(input.as_bytes())
            .expect("write the input of cc-sync");
        child.wait_with_output().expect("run cc-sync")
    }

    pub(crate) fn cc_sync_json(&self, args: &[&str]) -> (Value, Output) {
        let mut all = args.to_vec();
        all.push("--json");
        let output = self.cc_sync(&all);
        let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
            panic!(
                "stdout is not JSON ({err}): {}\nstderr: {}",
                text(&output.stdout),
                text(&output.stderr)
            )
        });
        (report, output)
    }
}

pub(crate) struct Remote {
    dir: TempDir,
}

impl Remote {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().expect("create a temporary remote directory");
        let status = isolated(Command::new("git"), dir.path())
            .args(["init", "--bare", "--quiet"])
            .arg(dir.path().join("remote.git"))
            .status()
            .expect("run git init");
        assert!(status.success(), "git init --bare failed");
        Self { dir }
    }

    pub(crate) fn url(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.dir.path().join("remote.git")
    }

    pub(crate) fn head(&self, branch: &str) -> Option<String> {
        let output = self.git(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ]);
        output
            .status
            .success()
            .then(|| text(&output.stdout).trim().to_owned())
    }

    pub(crate) fn show(&self, rev_path: &str) -> String {
        let output = self.git(&["show", rev_path]);
        assert!(
            output.status.success(),
            "git show {rev_path}: {}",
            text(&output.stderr)
        );
        text(&output.stdout)
    }

    pub(crate) fn commit_file(&self, branch: &str, path: &str, content: &str) {
        let work = self.dir.path().join("work");
        let run = |args: &[&str]| {
            let output = isolated(Command::new("git"), self.dir.path())
                .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
                .args(args)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                text(&output.stderr)
            );
        };
        let work_dir = work.to_string_lossy().into_owned();
        run(&["clone", "--quiet", &self.url(), &work_dir]);
        if self.head(branch).is_some() {
            run(&["-C", &work_dir, "checkout", "--quiet", branch]);
        } else {
            run(&["-C", &work_dir, "checkout", "--quiet", "--orphan", branch]);
        }
        fs::write(work.join(path), content).expect("write the file to commit");
        run(&["-C", &work_dir, "add", path]);
        run(&["-C", &work_dir, "commit", "--quiet", "-m", "add a file"]);
        run(&[
            "-C",
            &work_dir,
            "push",
            "--quiet",
            "origin",
            &format!("HEAD:refs/heads/{branch}"),
        ]);
        fs::remove_dir_all(&work).expect("remove the work clone");
    }

    pub(crate) fn all_objects(&self) -> String {
        text(
            &self
                .git(&["cat-file", "--batch-all-objects", "--batch"])
                .stdout,
        )
    }

    pub(crate) fn git(&self, args: &[&str]) -> Output {
        isolated(Command::new("git"), self.dir.path())
            .arg("--git-dir")
            .arg(self.path())
            .args(args)
            .output()
            .expect("run git")
    }
}

fn isolated(mut command: Command, home: &Path) -> Command {
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("CLAUDE_CONFIG_DIR");
    command
}

pub(crate) fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[track_caller]
pub(crate) fn assert_exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
