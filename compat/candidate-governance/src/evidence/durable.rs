//! Linux owner-private local-filesystem history. Archival eligibility is not authority.
use super::{consume::Consumption, freshness::ReuseKey};
use crate::subject::hash;
use guardengine::integration::attempt_store::{
    AttemptRecord, AttemptStore, InMemoryAttemptStore, Target,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
const VERSION: &str = "gitguard.local-history/v1alpha1";
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 10_000;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTarget {
    repo_id: String,
    task_id: String,
    requirement_ids: Vec<String>,
}
impl From<&Target> for WireTarget {
    fn from(t: &Target) -> Self {
        Self {
            repo_id: t.repo_id.clone(),
            task_id: t.task_id.clone(),
            requirement_ids: t.requirement_ids.clone(),
        }
    }
}
impl WireTarget {
    fn native(&self) -> Target {
        Target {
            repo_id: self.repo_id.clone(),
            task_id: self.task_id.clone(),
            requirement_ids: self.requirement_ids.clone(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Event {
    Advance {
        target: WireTarget,
        expected: u64,
        digest: String,
        run: String,
    },
    Append {
        target: WireTarget,
        generation: u64,
        digest: String,
        run: String,
        envelope: String,
        eligible: bool,
    },
    Publish {
        run: String,
    },
}
impl Event {
    fn apply(&self, store: &mut InMemoryAttemptStore) -> Result<Option<u64>, String> {
        match self {
            Self::Advance {
                target,
                expected,
                digest,
                run,
            } => store
                .advance(target.native(), *expected, digest.clone(), run.clone())
                .map(Some),
            Self::Append {
                target,
                generation,
                digest,
                run,
                envelope,
                eligible,
            } => store
                .append(AttemptRecord {
                    target: target.native(),
                    generation: *generation,
                    content_digest: digest.clone(),
                    run_id: run.clone(),
                    envelope_digest: envelope.clone(),
                    eligible: *eligible,
                })
                .map(|_| None),
            Self::Publish { run } => store.publish(run).map(|_| None),
        }
        .map_err(|_| "attempt transition rejected".into())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    api_version: String,
    store_id: String,
    events: Vec<Event>,
    checksum: String,
}
impl Snapshot {
    fn checksum(&self) -> Result<String, String> {
        serde_json::to_vec(&(&self.api_version, &self.store_id, &self.events))
            .map(|b| hash(&b))
            .map_err(|_| "snapshot serialization failed".into())
    }
}
struct FileStore {
    directory: File,
    identity: String,
}
struct Lock(File);
impl Lock {
    fn acquire(file: File) -> Result<Self, String> {
        // flock is tied to this freshly opened descriptor, shared by all backend processes.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("history busy or lock unavailable; retry observation".into());
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}
impl FileStore {
    fn directory(root: &Path) -> Result<File, String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(root)
            .map_err(|_| "private history directory unavailable")?;
        let meta = file
            .metadata()
            .map_err(|_| "history metadata unavailable")?;
        if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err("history directory must be owner-private".into());
        }
        // Linux local filesystems only; reject known network/pseudo filesystems and unknown types.
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err("filesystem capability unavailable".into());
        }
        let kind = unsafe { stat.assume_init() }.f_type;
        if ![0xef53, 0x58465342, 0x9123683e, 0x794c7630].contains(&kind) {
            return Err("unsupported history filesystem".into());
        }
        Ok(file)
    }
    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd())).join(name)
    }
    fn checked(file: &File) -> Result<(), String> {
        let m = file
            .metadata()
            .map_err(|_| "history metadata unavailable")?;
        if !m.is_file()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
        {
            return Err("unsafe history file".into());
        }
        Ok(())
    }
    fn lock(&self) -> Result<Lock, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(self.path("history.lock"))
            .map_err(|_| "history lock missing")?;
        Self::checked(&file)?;
        let mut id = String::new();
        (&mut file)
            .take(65)
            .read_to_string(&mut id)
            .map_err(|_| "invalid history identity")?;
        if id != self.identity {
            return Err("history identity changed".into());
        }
        Lock::acquire(file)
    }
    fn create(root: &Path) -> Result<Self, String> {
        let directory = Self::directory(root)?;
        let mut store = Self {
            directory,
            identity: String::new(),
        };
        if std::fs::read_dir(store.path("."))
            .map_err(|_| "history directory unavailable")?
            .next()
            .is_some()
        {
            return Err("history initialization requires an empty directory".into());
        }
        let nonce = tempfile::NamedTempFile::new_in(store.path("."))
            .map_err(|_| "history identity unavailable")?;
        store.identity = hash(nonce.path().as_os_str().as_encoded_bytes());
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(store.path("history.lock"))
            .map_err(|_| "history already initialized or unavailable")?;
        let _lock = Lock::acquire(file.try_clone().map_err(|_| "lock unavailable")?)?;
        file.write_all(store.identity.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|_| "history initialization outcome uncertain")?;
        store
            .directory
            .sync_all()
            .map_err(|_| "history initialization outcome uncertain")?;
        let snapshot = Snapshot {
            api_version: VERSION.into(),
            store_id: store.identity.clone(),
            events: vec![],
            checksum: String::new(),
        };
        store.save(snapshot)?;
        Ok(store)
    }
    fn open(root: &Path) -> Result<Self, String> {
        let mut store = Self {
            directory: Self::directory(root)?,
            identity: String::new(),
        };
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(store.path("history.lock"))
            .map_err(|_| "history not initialized")?;
        Self::checked(&file)?;
        (&mut file)
            .take(65)
            .read_to_string(&mut store.identity)
            .map_err(|_| "invalid history identity")?;
        if !crate::scope::digest_valid(&store.identity) {
            return Err("invalid history identity".into());
        }
        let _lock = store.lock()?;
        store.load()?;
        Ok(store)
    }
    fn load(&self) -> Result<(Snapshot, InMemoryAttemptStore), String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(self.path("state.json"))
            .map_err(|_| "history state missing; recovery required")?;
        Self::checked(&file)?;
        let mut bytes = vec![];
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "history state unavailable")?;
        if bytes.len() > MAX_BYTES {
            return Err("history capacity exceeded".into());
        }
        let snapshot: Snapshot =
            serde_json::from_slice(&bytes).map_err(|_| "history corrupt; recovery required")?;
        if snapshot.api_version != VERSION
            || snapshot.store_id != self.identity
            || snapshot.events.len() > MAX_EVENTS
            || snapshot.checksum != snapshot.checksum()?
        {
            return Err("history integrity failure; recovery required".into());
        }
        let mut store = InMemoryAttemptStore::default();
        for event in &snapshot.events {
            event.apply(&mut store)?;
        }
        Ok((snapshot, store))
    }
    fn save(&self, mut snapshot: Snapshot) -> Result<(), String> {
        snapshot.checksum = snapshot.checksum()?;
        let bytes = serde_json::to_vec(&snapshot).map_err(|_| "history serialization failed")?;
        if bytes.len() > MAX_BYTES || snapshot.events.len() > MAX_EVENTS {
            return Err("history capacity exceeded".into());
        }
        let mut tmp = tempfile::NamedTempFile::new_in(self.path("."))
            .map_err(|_| "history staging unavailable")?;
        tmp.write_all(&bytes)
            .and_then(|_| tmp.as_file().sync_all())
            .map_err(|_| "history staging failed")?;
        #[cfg(test)]
        if std::env::var("GG_DURABLE_TEST_CRASH_POINT").as_deref() == Ok("before-rename") {
            std::process::exit(71);
        }
        tmp.persist(self.path("state.json"))
            .map_err(|_| "history publication outcome uncertain; reopen and inspect")?;
        #[cfg(test)]
        if std::env::var("GG_DURABLE_TEST_CRASH_POINT").as_deref() == Ok("after-rename") {
            std::process::exit(72);
        }
        self.directory
            .sync_all()
            .map_err(|_| "history publication outcome uncertain; reopen and inspect")?;
        Ok(())
    }
    fn mutate(&self, event: Event) -> Result<Option<u64>, String> {
        if serde_json::to_vec(&event)
            .map_err(|_| "event serialization failed")?
            .len()
            > 65536
        {
            return Err("event too large".into());
        }
        let _lock = self.lock()?;
        let (mut snapshot, mut store) = self.load()?;
        let result = event.apply(&mut store)?;
        snapshot.events.push(event);
        self.save(snapshot)?;
        Ok(result)
    }
    fn advance(
        &self,
        target: Target,
        expected: u64,
        digest: String,
        run: String,
    ) -> Result<u64, String> {
        self.mutate(Event::Advance {
            target: WireTarget::from(&target),
            expected,
            digest,
            run,
        })?
        .ok_or("missing generation".into())
    }
    fn append(&self, r: AttemptRecord) -> Result<(), String> {
        self.mutate(Event::Append {
            target: WireTarget::from(&r.target),
            generation: r.generation,
            digest: r.content_digest,
            run: r.run_id,
            envelope: r.envelope_digest,
            eligible: r.eligible,
        })
        .map(|_| ())
    }
    fn publish(&self, run: &str) -> Result<(), String> {
        self.mutate(Event::Publish { run: run.into() }).map(|_| ())
    }
    fn current(&self, target: &Target) -> Result<Option<AttemptRecord>, String> {
        let _lock = self.lock()?;
        Ok(self.load()?.1.current(target).cloned())
    }
    fn history(&self, target: &Target) -> Result<Vec<AttemptRecord>, String> {
        let _lock = self.lock()?;
        Ok(self
            .load()?
            .1
            .history(target)
            .into_iter()
            .cloned()
            .collect())
    }
}
/// Separate Linux capability; existing LocalHistory still cannot provide durability.
pub struct DurableHistory {
    store: FileStore,
}
pub struct DurableTicket {
    key: ReuseKey,
    run: String,
    generation: u64,
    store_id: String,
}
impl DurableHistory {
    pub fn create(directory: &Path) -> Result<Self, String> {
        FileStore::create(directory).map(|store| Self { store })
    }
    pub fn open(directory: &Path) -> Result<Self, String> {
        FileStore::open(directory).map(|store| Self { store })
    }
    pub fn begin(
        &self,
        key: &ReuseKey,
        expected_generation: u64,
        run_id: &str,
    ) -> Result<DurableTicket, String> {
        let generation = self.store.advance(
            key.target.clone(),
            expected_generation,
            key.digest().into(),
            run_id.into(),
        )?;
        Ok(DurableTicket {
            key: key.clone(),
            run: run_id.into(),
            generation,
            store_id: self.store.identity.clone(),
        })
    }
    /// Resume an existing registration after restart; this does not authenticate any result.
    pub fn resume(&self, key: &ReuseKey, run_id: &str) -> Result<DurableTicket, String> {
        let _lock = self.store.lock()?;
        let (snapshot, _) = self.store.load()?;
        for event in snapshot.events {
            if let Event::Advance {
                target,
                expected,
                digest,
                run,
            } = event
                && run == run_id
                && target.native() == key.target
                && digest == key.digest()
            {
                return Ok(DurableTicket {
                    key: key.clone(),
                    run,
                    generation: expected.checked_add(1).ok_or("generation exhausted")?,
                    store_id: self.store.identity.clone(),
                });
            }
        }
        Err("registration not found".into())
    }
    pub fn complete(&self, ticket: &DurableTicket, result: &Consumption) -> Result<(), String> {
        if ticket.store_id != self.store.identity
            || ticket.run != result.run_id
            || ticket.key.candidate_digest != result.candidate_digest
            || ticket.key.policy_digest != result.policy_digest
            || result.reuse_digest.as_deref() != Some(ticket.key.digest())
        {
            return Err("consumption does not match durable attempt".into());
        }
        self.store.append(AttemptRecord {
            target: ticket.key.target.clone(),
            run_id: ticket.run.clone(),
            generation: ticket.generation,
            content_digest: ticket.key.digest().into(),
            envelope_digest: result.result.audit.envelope_digest.clone(),
            eligible: result.result.eligible,
        })
    }
    pub fn publish(&self, ticket: &DurableTicket) -> Result<(), String> {
        if ticket.store_id != self.store.identity {
            return Err("foreign durable ticket".into());
        }
        self.store.publish(&ticket.run)
    }
    /// Historical eligibility only. Consume fresh authority before making a new decision.
    pub fn current(&self, target: &Target) -> Result<Option<AttemptRecord>, String> {
        self.store.current(target)
    }
    pub fn history(&self, target: &Target) -> Result<Vec<AttemptRecord>, String> {
        self.store.history(target)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn target() -> Target {
        Target {
            repo_id: "repo".into(),
            task_id: "task".into(),
            requirement_ids: vec!["R".into()],
        }
    }
    fn digest() -> String {
        format!("sha256:{}", "a".repeat(64))
    }
    fn record(run: &str, generation: u64, eligible: bool) -> AttemptRecord {
        AttemptRecord {
            run_id: run.into(),
            target: target(),
            generation,
            content_digest: digest(),
            envelope_digest: digest(),
            eligible,
        }
    }
    #[test]
    fn restart_preserves_history_and_newer_generation_defeats_late_allow() {
        let dir = tempfile::Builder::new()
            .prefix(".gg-durable-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let a = FileStore::create(dir.path()).unwrap();
        assert_eq!(a.advance(target(), 0, digest(), "old".into()).unwrap(), 1);
        a.append(record("old", 1, true)).unwrap();
        a.publish("old").unwrap();
        drop(a);
        let b = FileStore::open(dir.path()).unwrap();
        assert_eq!(b.current(&target()).unwrap().unwrap().run_id, "old");
        assert_eq!(b.advance(target(), 1, digest(), "new".into()).unwrap(), 2);
        assert!(b.current(&target()).unwrap().is_none());
        b.append(record("new", 2, false)).unwrap();
        b.publish("new").unwrap();
        assert!(b.publish("old").is_err());
        drop(b);
        let c = FileStore::open(dir.path()).unwrap();
        assert!(!c.current(&target()).unwrap().unwrap().eligible);
        assert_eq!(c.history(&target()).unwrap().len(), 2);
        assert!(c.advance(target(), 2, digest(), "old".into()).is_err());
    }
    #[test]
    fn invalid_storage_never_resets_or_follows_symlinks() {
        let dir = tempfile::Builder::new()
            .prefix(".gg-durable-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        assert!(FileStore::open(dir.path()).is_err());
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(FileStore::create(dir.path()).is_err());
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = FileStore::create(dir.path()).unwrap();
        assert!(FileStore::create(dir.path()).is_err());
        std::fs::write(dir.path().join("state.json"), b"truncated").unwrap();
        assert!(FileStore::open(dir.path()).is_err());
        std::fs::remove_file(dir.path().join("state.json")).unwrap();
        assert!(FileStore::open(dir.path()).is_err());
        std::os::unix::fs::symlink("/dev/null", dir.path().join("state.json")).unwrap();
        assert!(FileStore::open(dir.path()).is_err());
    }

    #[test]
    fn process_worker() {
        let Ok(root) = std::env::var("GG_DURABLE_TEST_ROOT") else {
            return;
        };
        let run = std::env::var("GG_DURABLE_TEST_RUN").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let store = loop {
            match FileStore::open(Path::new(&root)) {
                Ok(store) => break store,
                Err(e) if e.starts_with("history busy") && std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                Err(e) => panic!("{e}"),
            }
        };
        if let Ok(gate) = std::env::var("GG_DURABLE_TEST_GATE") {
            std::fs::write(format!("{gate}.{run}.ready"), b"ready").unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !Path::new(&gate).exists() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let won = store.advance(target(), 0, digest(), run.clone()).is_ok();
        if let Ok(output) = std::env::var("GG_DURABLE_TEST_OUTPUT") {
            std::fs::write(output, if won { "won" } else { "lost" }).unwrap();
        }
    }
    fn worker(root: &Path, run: &str) -> std::process::Command {
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "evidence::durable::tests::process_worker",
            "--nocapture",
        ])
        .env("GG_DURABLE_TEST_ROOT", root)
        .env("GG_DURABLE_TEST_RUN", run)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
        cmd
    }
    #[test]
    fn independent_processes_have_exactly_one_cas_winner() {
        let dir = tempfile::Builder::new()
            .prefix(".gg-durable-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let root = dir.path().join("store");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        FileStore::create(&root).unwrap();
        let gate = dir.path().join("gate");
        let mut children = vec![];
        for run in ["a", "b"] {
            children.push(
                worker(&root, run)
                    .env("GG_DURABLE_TEST_GATE", &gate)
                    .env("GG_DURABLE_TEST_OUTPUT", dir.path().join(run))
                    .spawn()
                    .unwrap(),
            );
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !["a", "b"]
            .iter()
            .all(|run| dir.path().join(format!("gate.{run}.ready")).exists())
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::fs::write(&gate, b"go").unwrap();
        for child in children {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(
            ["a", "b"]
                .iter()
                .filter(|r| std::fs::read(dir.path().join(r)).unwrap() == b"won")
                .count(),
            1
        );
        let reopened = FileStore::open(&root).unwrap();
        assert!(
            reopened
                .advance(target(), 0, digest(), "stale".into())
                .is_err()
        );
        assert_eq!(
            reopened
                .advance(target(), 1, digest(), "next".into())
                .unwrap(),
            2
        );
    }
    #[test]
    fn process_crash_around_atomic_replace_preserves_whole_old_or_new_state() {
        for (point, exit, expected) in [("before-rename", 71, 0), ("after-rename", 72, 1)] {
            let dir = tempfile::Builder::new()
                .prefix(".gg-durable-test-")
                .tempdir_in(env!("CARGO_MANIFEST_DIR"))
                .unwrap();
            FileStore::create(dir.path()).unwrap();
            let status = worker(dir.path(), "interrupted")
                .env("GG_DURABLE_TEST_CRASH_POINT", point)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(exit));
            let reopened = FileStore::open(dir.path()).unwrap();
            assert_eq!(
                reopened
                    .advance(target(), expected, digest(), "next".into())
                    .unwrap(),
                expected + 1
            );
        }
    }

    #[test]
    fn checksums_and_native_transition_replay_reject_corrupt_or_invented_history() {
        let dir = tempfile::Builder::new()
            .prefix(".gg-durable-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let store = FileStore::create(dir.path()).unwrap();
        store.advance(target(), 0, digest(), "one".into()).unwrap();
        let path = dir.path().join("state.json");
        let original = std::fs::read(&path).unwrap();
        let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
        snapshot.checksum = "0".repeat(64);
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(FileStore::open(dir.path()).is_err());
        let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
        if let Event::Advance { expected, .. } = &mut snapshot.events[0] {
            *expected = 7;
        }
        snapshot.checksum = snapshot.checksum().unwrap();
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(FileStore::open(dir.path()).is_err());
        let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
        snapshot.api_version = "future-version".into();
        snapshot.checksum = snapshot.checksum().unwrap();
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(FileStore::open(dir.path()).is_err());
    }
}
