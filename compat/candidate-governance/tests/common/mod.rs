#![allow(dead_code)]
use std::{
    path::{Path, PathBuf},
    process::Command,
};
pub struct Repo {
    pub dir: tempfile::TempDir,
    pub base: String,
}
pub fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap().trim().into()
}
impl Repo {
    pub fn new(format: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        git(
            dir.path(),
            &["init", "-q", &format!("--object-format={format}")],
        );
        std::fs::write(dir.path().join("a"), "base\n").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "base"]);
        let base = git(dir.path(), &["rev-parse", "HEAD"]);
        Self { dir, base }
    }
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
    pub fn commit(&self, path: &str, body: &str) -> String {
        std::fs::write(self.path().join(path), body).unwrap();
        git(self.path(), &["add", "."]);
        git(self.path(), &["commit", "-qm", "next"]);
        git(self.path(), &["rev-parse", "HEAD"])
    }
    pub fn state(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(p: &Path, base: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            let mut es = std::fs::read_dir(p)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>();
            es.sort();
            for e in es {
                if e.is_dir() {
                    walk(&e, base, out)
                } else {
                    out.push((
                        e.strip_prefix(base).unwrap().into(),
                        std::fs::read(e).unwrap(),
                    ));
                }
            }
        }
        let mut out = vec![];
        walk(self.path(), self.path(), &mut out);
        out
    }
}
pub mod evidence;
