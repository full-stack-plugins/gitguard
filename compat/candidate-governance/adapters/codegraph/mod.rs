//! Frozen actual CodeGraph export consumption; never certifies semantic completeness.
use crate::{Repository, candidate::CandidateSnapshot, conflicts::ConflictObservation};
use std::collections::BTreeMap;
pub const SOURCE_REVISION: &str = "1072f82ce24db3d133258d30165cef6b74d108b2";
/// Controller-selected provenance and mappings, not producer authentication or JSON authority.
pub struct Profile {
    pub source_revision: String,
    pub export_sha256: String,
    pub candidate_oid: String,
    pub file_tasks: BTreeMap<String, String>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Binding,
    Schema,
    Coverage,
    Budget,
}
fn text(v: &serde_json::Value) -> Result<&str, Error> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Schema)
}
fn rows<'a>(v: &'a serde_json::Value, key: &str) -> Result<&'a Vec<serde_json::Value>, Error> {
    let a = v[key].as_array().ok_or(Error::Schema)?;
    if a.len() > 4096 {
        return Err(Error::Budget);
    }
    Ok(a)
}
pub fn analyze(
    repo: &Repository,
    snapshots: &[CandidateSnapshot],
    raw: &[u8],
    profile: &Profile,
) -> Result<ConflictObservation, Error> {
    if raw.len() > 1024 * 1024 || snapshots.len() > 128 || profile.file_tasks.len() > 1024 {
        return Err(Error::Budget);
    }
    if profile.source_revision != SOURCE_REVISION
        || profile.export_sha256 != crate::subject::hash(raw)
        || profile.candidate_oid.len() != 40
        || !profile
            .candidate_oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || snapshots.is_empty()
        || profile.file_tasks.is_empty()
    {
        return Err(Error::Binding);
    }
    let mut tasks = BTreeMap::new();
    for snapshot in snapshots {
        snapshot.validate(repo).map_err(|_| Error::Binding)?;
        if snapshot.candidate_oid() != profile.candidate_oid
            || tasks.insert(snapshot.task_id(), snapshot).is_some()
        {
            return Err(Error::Binding);
        }
    }
    let v = crate::api::unique_json(raw).map_err(|_| Error::Schema)?;
    let tables = &v["tables"];
    let versions = rows(tables, "schema_versions")?;
    if versions.len() != 2 || versions[0]["version"] != 1 || versions[1]["version"] != 11 {
        return Err(Error::Schema);
    }
    let mut metadata = BTreeMap::new();
    for row in rows(tables, "project_metadata")? {
        if metadata
            .insert(text(&row["key"])?, text(&row["value"])?)
            .is_some()
        {
            return Err(Error::Schema);
        }
    }
    for (key, expected) in [
        ("indexed_with_version", "1.6.0"),
        ("indexed_with_extraction_version", "27"),
        ("indexed_at_commit", profile.candidate_oid.as_str()),
        ("index_state", "complete"),
    ] {
        if metadata.get(key) != Some(&expected) {
            return Err(Error::Binding);
        }
    }
    let dirty = crate::api::unique_json(
        metadata
            .get("indexed_dirty_paths")
            .ok_or(Error::Schema)?
            .as_bytes(),
    )
    .map_err(|_| Error::Schema)?;
    if dirty["commit"].as_str() != Some(profile.candidate_oid.as_str())
        || dirty["paths"].as_array().is_none_or(|a| !a.is_empty())
    {
        return Err(Error::Coverage);
    }
    if !rows(tables, "unresolved_refs")?.is_empty() {
        return Err(Error::Coverage);
    }
    let mut files = std::collections::BTreeSet::new();
    for row in rows(tables, "files")? {
        let path = text(&row["path"])?;
        if !crate::scope::path_valid(path.as_bytes())
            || !files.insert(path)
            || row.get("errors") != Some(&serde_json::Value::Null)
            || row["language"] != "javascript"
            || row["generated"] != 0
        {
            return Err(Error::Coverage);
        }
        let task = profile.file_tasks.get(path).ok_or(Error::Coverage)?;
        let snapshot = tasks.get(task.as_str()).ok_or(Error::Binding)?;
        if !snapshot
            .allowed_paths()
            .iter()
            .any(|p| p == path.as_bytes())
        {
            return Err(Error::Binding);
        }
        let blob = repo
            .run(&["show", &format!("{}:{}", profile.candidate_oid, path)])
            .map_err(|_| Error::Binding)?;
        if text(&row["content_hash"])? != crate::subject::hash(&blob)
            || row["size"].as_u64() != Some(blob.len() as u64)
        {
            return Err(Error::Binding);
        }
    }
    if files.len() != profile.file_tasks.len()
        || tasks
            .keys()
            .any(|task| !profile.file_tasks.values().any(|t| t == task))
    {
        return Err(Error::Coverage);
    }
    for key in ["index_files_discovered", "index_files_accounted"] {
        if metadata.get(key).and_then(|v| v.parse::<usize>().ok()) != Some(files.len()) {
            return Err(Error::Coverage);
        }
    }
    let mut nodes = BTreeMap::new();
    for row in rows(tables, "nodes")? {
        let id = text(&row["id"])?;
        if !files.contains(text(&row["file_path"])?)
            || row["language"] != "javascript"
            || nodes.insert(id, row).is_some()
        {
            return Err(Error::Coverage);
        }
    }
    let mut risks = vec![];
    let mut edge_ids = std::collections::BTreeSet::new();
    for edge in rows(tables, "edges")? {
        if !edge_ids.insert(edge["id"].as_u64().ok_or(Error::Schema)?) {
            return Err(Error::Schema);
        }
        let from = nodes.get(text(&edge["source"])?).ok_or(Error::Coverage)?;
        let to = nodes.get(text(&edge["target"])?).ok_or(Error::Coverage)?;
        let kind = text(&edge["kind"])?;
        if !matches!(kind, "calls" | "imports" | "contains") {
            return Err(Error::Coverage);
        }
        if kind == "calls" {
            let consumer = &profile.file_tasks[text(&from["file_path"])?];
            let provider = &profile.file_tasks[text(&to["file_path"])?];
            if consumer != provider {
                risks.push(format!(
                    "{consumer} consumes {provider}: {} -> {}",
                    text(&from["name"])?,
                    text(&to["name"])?
                ));
            }
        }
    }
    risks.sort();
    risks.dedup();
    Ok(ConflictObservation {
        risks,
        missing: vec!["semantic-completeness".into()],
        complete: false,
        unknown: true,
    })
}
