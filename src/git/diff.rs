use crate::{Diagnostic, Repository, Result, scope::TaskScope};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub paths: Vec<Vec<u8>>,
    pub old_mode: String,
    pub new_mode: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub changes: Vec<Change>,
    pub violations: Vec<Vec<u8>>,
    pub missing_coverage: Vec<Vec<u8>>,
}
impl Repository {
    pub fn scan_scope(&self, base: &str, candidate: &str, scope: &TaskScope) -> Result<ChangeSet> {
        scope.validate()?;
        self.commit(base)?;
        self.commit(candidate)?;
        let raw = self.run(&[
            "diff-tree",
            "--no-commit-id",
            "--no-ext-diff",
            "--no-textconv",
            "--raw",
            "-r",
            "-z",
            "-M",
            base,
            candidate,
            "--",
        ])?;
        let mut parts = raw.split(|x| *x == 0).filter(|x| !x.is_empty());
        let mut result = ChangeSet {
            changes: vec![],
            violations: vec![],
            missing_coverage: vec![],
        };
        while let Some(header) = parts.next() {
            let header = std::str::from_utf8(header).map_err(|_| Diagnostic::GitFailed)?;
            let cols = header.split_whitespace().collect::<Vec<_>>();
            if cols.len() != 5 || !cols[0].starts_with(':') {
                return Err(Diagnostic::GitFailed);
            }
            let mut paths = vec![parts.next().ok_or(Diagnostic::GitFailed)?.to_vec()];
            if cols[4].starts_with('R') || cols[4].starts_with('C') {
                paths.push(parts.next().ok_or(Diagnostic::GitFailed)?.to_vec());
            }
            for path in &paths {
                if !scope.permits(path) {
                    result.violations.push(path.clone());
                }
                if cols[0] == ":160000" || cols[1] == "160000" {
                    result.missing_coverage.push(path.clone());
                }
            }
            result.changes.push(Change {
                paths,
                old_mode: cols[0][1..].into(),
                new_mode: cols[1].into(),
                status: cols[4].into(),
            });
        }
        Ok(result)
    }
}
