//! Local durable admission only. No state or reservation authorizes a Git write.
use super::grant::GrantExpectation;
use crate::subject::hash;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
const VERSION: &str = "gitguard.operation-intent/v1alpha1";
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 12288;
const MAX_OPERATIONS: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentError {
    Invalid,
    Conflict,
    Stale,
    Busy,
    Unavailable,
    UnsafeStorage,
    Corrupt,
    Capacity,
    OutcomeUncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentState {
    Prepared,
    AttemptRecorded,
    RecoveryRequired,
}
/// Historical storage state, never a platform receipt or permission. No Deserialize API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IntentReceipt {
    api_version: String,
    store_id: String,
    operation_id: String,
    request_digest: String,
    generation: u64,
    state: IntentState,
}
impl IntentReceipt {
    pub fn state(&self) -> IntentState {
        self.state
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }
}
/// Unforgeable single returned reservation, not authorization; never resumed from storage.
pub struct AttemptReservation {
    receipt: IntentReceipt,
}
impl AttemptReservation {
    pub fn receipt(&self) -> &IntentReceipt {
        &self.receipt
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    operation_id: String,
    request_digest: String,
    generation: u64,
    state: IntentState,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    api_version: String,
    store_id: String,
    #[serde(deserialize_with = "bounded_events")]
    events: Vec<Event>,
    checksum: String,
}
fn bounded_events<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Event>, D::Error> {
    struct Events;
    impl<'de> serde::de::Visitor<'de> for Events {
        type Value = Vec<Event>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded intent events")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut events = Vec::new();
            while let Some(event) = seq.next_element::<Event>()? {
                if events.len() == MAX_EVENTS {
                    return Err(serde::de::Error::custom("event capacity"));
                }
                events.push(event);
            }
            Ok(events)
        }
    }
    deserializer.deserialize_seq(Events)
}
fn digest_valid(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(crate::scope::digest_valid)
}
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(b.len())
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(|| std::io::Error::other("capacity"))?;
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn bounded_bytes(value: &impl Serialize) -> Result<Vec<u8>, IntentError> {
    serde_json::to_writer(Counter(0), value).map_err(|_| IntentError::Capacity)?;
    serde_json::to_vec(value).map_err(|_| IntentError::Invalid)
}
impl Snapshot {
    fn checksum(&self) -> Result<String, IntentError> {
        Ok(hash(&bounded_bytes(&(
            &self.api_version,
            &self.store_id,
            &self.events,
        ))?))
    }
    fn replay(&self) -> Result<BTreeMap<String, IntentReceipt>, IntentError> {
        if self.api_version != VERSION
            || self.events.len() > MAX_EVENTS
            || !crate::scope::digest_valid(&self.store_id)
            || self.checksum != self.checksum()?
        {
            return Err(IntentError::Corrupt);
        }
        let mut records: BTreeMap<String, IntentReceipt> = BTreeMap::new();
        for event in &self.events {
            if event.operation_id.is_empty()
                || event.operation_id.len() > 128
                || event.operation_id.trim() != event.operation_id
                || event.operation_id.chars().any(char::is_control)
                || !digest_valid(&event.request_digest)
            {
                return Err(IntentError::Corrupt);
            }
            let previous = records.get(&event.operation_id);
            let valid = match previous {
                None => event.state == IntentState::Prepared && event.generation == 0,
                Some(p) => {
                    p.request_digest == event.request_digest
                        && match (p.state, event.state) {
                            (IntentState::Prepared, IntentState::AttemptRecorded) => {
                                event.generation == 1
                            }
                            (IntentState::AttemptRecorded, IntentState::RecoveryRequired) => {
                                event.generation == 2
                            }
                            _ => false,
                        }
                }
            };
            if !valid || (previous.is_none() && records.len() >= MAX_OPERATIONS) {
                return Err(IntentError::Corrupt);
            }
            records.insert(
                event.operation_id.clone(),
                IntentReceipt {
                    api_version: VERSION.into(),
                    store_id: self.store_id.clone(),
                    operation_id: event.operation_id.clone(),
                    request_digest: event.request_digest.clone(),
                    generation: event.generation,
                    state: event.state,
                },
            );
        }
        Ok(records)
    }
}
struct Lock(File);
impl Lock {
    fn acquire(f: File) -> Result<Self, IntentError> {
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::EWOULDBLOCK) {
                    IntentError::Busy
                } else {
                    IntentError::Unavailable
                },
            );
        }
        Ok(Self(f))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
/// One participating-controller local filesystem namespace; not network-wide uniqueness.
pub struct IntentStore {
    directory: File,
    identity: String,
    lock_dev: u64,
    lock_ino: u64,
}
impl IntentStore {
    fn directory(root: &Path) -> Result<File, IntentError> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(root)
            .map_err(|_| IntentError::UnsafeStorage)?;
        let m = f.metadata().map_err(|_| IntentError::Unavailable)?;
        if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err(IntentError::UnsafeStorage);
        }
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(f.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(IntentError::Unavailable);
        }
        if ![0xef53, 0x58465342, 0x9123683e, 0x794c7630]
            .contains(&unsafe { stat.assume_init() }.f_type)
        {
            return Err(IntentError::UnsafeStorage);
        }
        Ok(f)
    }
    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd())).join(name)
    }
    fn checked(f: &File) -> Result<(), IntentError> {
        let m = f.metadata().map_err(|_| IntentError::Unavailable)?;
        if !m.is_file()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
        {
            return Err(IntentError::UnsafeStorage);
        }
        Ok(())
    }
    fn file(&self, name: &str) -> Result<File, IntentError> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(self.path(name))
            .map_err(|_| IntentError::Corrupt)?;
        Self::checked(&f)?;
        Ok(f)
    }
    fn lock(&self) -> Result<Lock, IntentError> {
        let mut f = self.file("intent.lock")?;
        let m = f.metadata().map_err(|_| IntentError::Unavailable)?;
        if m.dev() != self.lock_dev || m.ino() != self.lock_ino {
            return Err(IntentError::UnsafeStorage);
        }
        let lock = Lock::acquire(f.try_clone().map_err(|_| IntentError::Unavailable)?)?;
        let mut id = String::new();
        (&mut f)
            .take(65)
            .read_to_string(&mut id)
            .map_err(|_| IntentError::Corrupt)?;
        if id != self.identity {
            return Err(IntentError::Corrupt);
        }
        Ok(lock)
    }
    pub fn create(root: &Path) -> Result<Self, IntentError> {
        let mut store = Self {
            directory: Self::directory(root)?,
            identity: String::new(),
            lock_dev: 0,
            lock_ino: 0,
        };
        if std::fs::read_dir(store.path("."))
            .map_err(|_| IntentError::Unavailable)?
            .next()
            .is_some()
        {
            return Err(IntentError::UnsafeStorage);
        }
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(store.path("intent.lock"))
            .map_err(|_| IntentError::UnsafeStorage)?;
        let _lock = Lock::acquire(f.try_clone().map_err(|_| IntentError::Unavailable)?)?;
        let nonce = tempfile::NamedTempFile::new_in(store.path("."))
            .map_err(|_| IntentError::Unavailable)?;
        store.identity = hash(nonce.path().as_os_str().as_encoded_bytes());
        let m = f.metadata().map_err(|_| IntentError::Unavailable)?;
        store.lock_dev = m.dev();
        store.lock_ino = m.ino();
        f.write_all(store.identity.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|_| IntentError::OutcomeUncertain)?;
        store
            .directory
            .sync_all()
            .map_err(|_| IntentError::OutcomeUncertain)?;
        store.save(Snapshot {
            api_version: VERSION.into(),
            store_id: store.identity.clone(),
            events: vec![],
            checksum: String::new(),
        })?;
        Ok(store)
    }
    pub fn open(root: &Path) -> Result<Self, IntentError> {
        let mut store = Self {
            directory: Self::directory(root)?,
            identity: String::new(),
            lock_dev: 0,
            lock_ino: 0,
        };
        let mut f = store.file("intent.lock")?;
        let m = f.metadata().map_err(|_| IntentError::Unavailable)?;
        store.lock_dev = m.dev();
        store.lock_ino = m.ino();
        (&mut f)
            .take(65)
            .read_to_string(&mut store.identity)
            .map_err(|_| IntentError::Corrupt)?;
        if !crate::scope::digest_valid(&store.identity) {
            return Err(IntentError::Corrupt);
        }
        let _lock = store.lock()?;
        store.load()?;
        Ok(store)
    }
    fn load(&self) -> Result<(Snapshot, BTreeMap<String, IntentReceipt>), IntentError> {
        let f = self.file("intent.json")?;
        if f.metadata().map_err(|_| IntentError::Unavailable)?.len() > MAX_BYTES as u64 {
            return Err(IntentError::Capacity);
        }
        let mut bytes = vec![];
        f.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| IntentError::Unavailable)?;
        if bytes.len() > MAX_BYTES {
            return Err(IntentError::Capacity);
        }
        let snapshot: Snapshot =
            serde_json::from_slice(&bytes).map_err(|_| IntentError::Corrupt)?;
        if snapshot.store_id != self.identity {
            return Err(IntentError::Corrupt);
        }
        let records = snapshot.replay()?;
        Ok((snapshot, records))
    }
    fn save(&self, mut snapshot: Snapshot) -> Result<(), IntentError> {
        if snapshot.events.len() > MAX_EVENTS {
            return Err(IntentError::Capacity);
        }
        snapshot.checksum = snapshot.checksum()?;
        let bytes = bounded_bytes(&snapshot)?;
        let mut tmp = tempfile::NamedTempFile::new_in(self.path("."))
            .map_err(|_| IntentError::Unavailable)?;
        tmp.write_all(&bytes)
            .and_then(|_| tmp.as_file().sync_all())
            .map_err(|_| IntentError::Unavailable)?;
        #[cfg(test)]
        crash("before-rename");
        tmp.persist(self.path("intent.json"))
            .map_err(|_| IntentError::OutcomeUncertain)?;
        #[cfg(test)]
        crash("after-rename");
        self.directory
            .sync_all()
            .map_err(|_| IntentError::OutcomeUncertain)?;
        Ok(())
    }
    pub fn prepare(&self, expected: &GrantExpectation) -> Result<IntentReceipt, IntentError> {
        let _lock = self.lock()?;
        let (mut snapshot, records) = self.load()?;
        if let Some(old) = records.get(expected.operation_id()) {
            return if old.request_digest == expected.digest() {
                Ok(old.clone())
            } else {
                Err(IntentError::Conflict)
            };
        }
        if records.len() >= MAX_OPERATIONS {
            return Err(IntentError::Capacity);
        }
        let event = Event {
            operation_id: expected.operation_id().into(),
            request_digest: expected.digest().into(),
            generation: 0,
            state: IntentState::Prepared,
        };
        let receipt = self.receipt(&event);
        snapshot.events.push(event);
        self.save(snapshot)?;
        Ok(receipt)
    }
    fn receipt(&self, e: &Event) -> IntentReceipt {
        IntentReceipt {
            api_version: VERSION.into(),
            store_id: self.identity.clone(),
            operation_id: e.operation_id.clone(),
            request_digest: e.request_digest.clone(),
            generation: e.generation,
            state: e.state,
        }
    }
    fn transition(
        &self,
        expected: &IntentReceipt,
        state: IntentState,
    ) -> Result<IntentReceipt, IntentError> {
        if expected.store_id != self.identity {
            return Err(IntentError::Conflict);
        }
        let _lock = self.lock()?;
        let (mut snapshot, records) = self.load()?;
        let current = records
            .get(&expected.operation_id)
            .ok_or(IntentError::Stale)?;
        if current != expected {
            return Err(IntentError::Stale);
        }
        if !matches!(
            (current.state, state),
            (IntentState::Prepared, IntentState::AttemptRecorded)
                | (IntentState::AttemptRecorded, IntentState::RecoveryRequired)
        ) {
            return Err(IntentError::Stale);
        }
        let event = Event {
            operation_id: current.operation_id.clone(),
            request_digest: current.request_digest.clone(),
            generation: current.generation + 1,
            state,
        };
        let receipt = self.receipt(&event);
        snapshot.events.push(event);
        self.save(snapshot)?;
        Ok(receipt)
    }
    /// Persist the sole attempt reservation. This method performs no authorization or mutation.
    pub fn claim(&self, prepared: &IntentReceipt) -> Result<AttemptReservation, IntentError> {
        if prepared.state != IntentState::Prepared {
            return Err(IntentError::Stale);
        }
        self.transition(prepared, IntentState::AttemptRecorded)
            .map(|receipt| AttemptReservation { receipt })
    }
    pub fn mark_uncertain(
        &self,
        reservation: AttemptReservation,
    ) -> Result<IntentReceipt, IntentError> {
        self.transition(&reservation.receipt, IntentState::RecoveryRequired)
    }
}
#[cfg(test)]
fn crash(point: &str) {
    if std::env::var("GG_INTENT_TEST_CRASH").as_deref() == Ok(point) {
        std::process::exit(if point == "before-rename" { 71 } else { 72 });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    fn root() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(".intent-unit-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
    }
    fn seed(store: &IntentStore) -> IntentReceipt {
        let mut snapshot = store.load().unwrap().0;
        let e = Event {
            operation_id: "op".into(),
            request_digest: format!("sha256:{}", "a".repeat(64)),
            generation: 0,
            state: IntentState::Prepared,
        };
        snapshot.events.push(e.clone());
        store.save(snapshot).unwrap();
        store.receipt(&e)
    }
    #[test]
    fn worker() {
        let Ok(root) = std::env::var("GG_INTENT_WORKER") else {
            return;
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let store = loop {
            match IntentStore::open(Path::new(&root)) {
                Ok(s) => break s,
                Err(IntentError::Busy) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => panic!("open failed: {e:?}"),
            }
        };
        let receipt = store.load().unwrap().1.remove("op").unwrap();
        if let Ok(gate) = std::env::var("GG_INTENT_GATE") {
            let id = std::env::var("GG_INTENT_ID").unwrap();
            std::fs::write(format!("{gate}.{id}"), b"ready").unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !Path::new(&gate).exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let result = store.claim(&receipt);
        if let Ok(out) = std::env::var("GG_INTENT_OUT") {
            std::fs::write(
                out,
                if result.is_ok() {
                    &b"won"[..]
                } else {
                    &b"lost"[..]
                },
            )
            .unwrap();
        }
        if std::env::var("GG_INTENT_EXIT_AFTER").is_ok() {
            assert!(result.is_ok());
            std::process::exit(73);
        }
    }
    fn child(root: &Path) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", "execution::intent::tests::worker", "--nocapture"])
            .env("GG_INTENT_WORKER", root)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        c
    }
    #[test]
    fn two_real_processes_only_one_attempt_and_exit_cannot_reclaim() {
        let d = root();
        let store = IntentStore::create(d.path()).unwrap();
        let receipt = seed(&store);
        let gate = d.path().join("gate");
        let mut children = vec![];
        for id in ["a", "b"] {
            children.push(
                child(d.path())
                    .env("GG_INTENT_GATE", &gate)
                    .env("GG_INTENT_ID", id)
                    .env("GG_INTENT_OUT", d.path().join(id))
                    .spawn()
                    .unwrap(),
            );
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !["a", "b"]
            .iter()
            .all(|id| d.path().join(format!("gate.{id}")).exists())
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        std::fs::write(&gate, b"go").unwrap();
        for c in children {
            let out = c.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        assert_eq!(
            ["a", "b"]
                .iter()
                .filter(|id| std::fs::read(d.path().join(id)).unwrap() == b"won")
                .count(),
            1
        );
        let reopened = IntentStore::open(d.path()).unwrap();
        assert_eq!(reopened.load().unwrap().0.events.len(), 2);
        assert!(reopened.claim(&receipt).is_err());
        let d = root();
        let store = IntentStore::create(d.path()).unwrap();
        let receipt = seed(&store);
        assert_eq!(
            child(d.path())
                .env("GG_INTENT_EXIT_AFTER", "1")
                .status()
                .unwrap()
                .code(),
            Some(73)
        );
        assert!(
            IntentStore::open(d.path())
                .unwrap()
                .claim(&receipt)
                .is_err()
        );
    }
    #[test]
    fn crashes_around_rename_never_resume_a_committed_attempt() {
        for (point, code, state) in [
            ("before-rename", 71, IntentState::Prepared),
            ("after-rename", 72, IntentState::AttemptRecorded),
        ] {
            let d = root();
            let store = IntentStore::create(d.path()).unwrap();
            let receipt = seed(&store);
            assert_eq!(
                child(d.path())
                    .env("GG_INTENT_TEST_CRASH", point)
                    .status()
                    .unwrap()
                    .code(),
                Some(code)
            );
            let reopened = IntentStore::open(d.path()).unwrap();
            assert_eq!(reopened.load().unwrap().1["op"].state, state);
            assert_eq!(
                reopened.claim(&receipt).is_ok(),
                state == IntentState::Prepared
            );
        }
    }
    #[test]
    fn corrupt_missing_unsafe_and_rehashed_invalid_transitions_fail_closed() {
        let d = root();
        let store = IntentStore::create(d.path()).unwrap();
        seed(&store);
        let original = std::fs::read(d.path().join("intent.json")).unwrap();
        for bytes in [
            b"{".to_vec(),
            [original.clone(), b"x".to_vec()].concat(),
            [
                b"{\"api_version\":\"duplicate\",".as_slice(),
                &original[1..],
            ]
            .concat(),
        ] {
            // third case is duplicate field
            std::fs::write(d.path().join("intent.json"), bytes).unwrap();
            assert!(IntentStore::open(d.path()).is_err());
        }
        let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
        snapshot.events[0].state = IntentState::AttemptRecorded;
        snapshot.checksum = snapshot.checksum().unwrap();
        std::fs::write(
            d.path().join("intent.json"),
            bounded_bytes(&snapshot).unwrap(),
        )
        .unwrap();
        assert!(IntentStore::open(d.path()).is_err());
        std::fs::remove_file(d.path().join("intent.json")).unwrap();
        assert!(IntentStore::open(d.path()).is_err());
        std::os::unix::fs::symlink("/dev/null", d.path().join("intent.json")).unwrap();
        assert!(IntentStore::open(d.path()).is_err());
        let d = root();
        std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(IntentStore::create(d.path()).is_err());
    }
    #[test]
    fn capacity_and_foreign_receipts_and_replaced_locks_fail_closed() {
        let d = root();
        let a = IntentStore::create(d.path()).unwrap();
        let receipt = seed(&a);
        let other = root();
        let b = IntentStore::create(other.path()).unwrap();
        assert!(matches!(b.claim(&receipt), Err(IntentError::Conflict)));
        let held = a.lock().unwrap();
        assert!(matches!(a.claim(&receipt), Err(IntentError::Busy)));
        drop(held);
        let path = d.path().join("intent.lock");
        let bytes = std::fs::read(&path).unwrap();
        std::fs::rename(&path, d.path().join("old.lock")).unwrap();
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        f.write_all(&bytes).unwrap();
        assert!(matches!(a.claim(&receipt), Err(IntentError::UnsafeStorage)));
        let d = root();
        let s = IntentStore::create(d.path()).unwrap();
        let mut snapshot = s.load().unwrap().0;
        for i in 0..MAX_OPERATIONS + 1 {
            snapshot.events.push(Event {
                operation_id: format!("op-{i}"),
                request_digest: format!("sha256:{}", "a".repeat(64)),
                generation: 0,
                state: IntentState::Prepared,
            });
        }
        snapshot.checksum = snapshot.checksum().unwrap();
        assert!(snapshot.replay().is_err());
        let f = OpenOptions::new()
            .write(true)
            .open(d.path().join("intent.json"))
            .unwrap();
        f.set_len(MAX_BYTES as u64 + 1).unwrap();
        assert!(matches!(s.load(), Err(IntentError::Capacity)));
    }
}
