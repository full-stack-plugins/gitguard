mod common;
use gitguard::{
    Repository,
    scope::{ImmutableScopeSource, ProtectedScopeRequest, ProtectedTaskScope},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
struct Counting;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static MAX: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            MAX.fetch_max(l.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            MAX.fetch_max(n, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
#[test]
fn oversize_metadata_rejected_without_allocating_or_reading_git() {
    let r = common::Repo::new("sha1");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    for kind in 0..4 {
        let mut request = ProtectedScopeRequest {
            repo_id: "repo".into(),
            task_id: "t".into(),
            requirement_ids: vec!["R".into()],
            allowed_paths: vec![b"a".to_vec()],
            policy: ImmutableScopeSource {
                commit_oid: r.base.clone(),
                path: b"missing".to_vec(),
                digest: "a".repeat(64),
            },
            baseline: None,
        };
        match kind {
            0 => request.task_id = "t".repeat(17 * 1024 * 1024),
            1 => request.allowed_paths = vec![vec![b'a'; 4096]; 256],
            2 => request.requirement_ids = vec!["R".into(); 257],
            _ => request.policy.path = vec![b'a'; 17 * 1024 * 1024],
        }
        MAX.store(0, Ordering::Relaxed);
        ACTIVE.store(true, Ordering::Relaxed);
        let result = ProtectedTaskScope::freeze(&repo, &request);
        ACTIVE.store(false, Ordering::Relaxed);
        assert!(matches!(result, Err(gitguard::Diagnostic::LimitExceeded)));
        assert_eq!(MAX.load(Ordering::Relaxed), 0);
    }
}
