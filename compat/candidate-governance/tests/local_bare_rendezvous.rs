#[path = "../examples/support/local_bare_rendezvous.rs"]
mod rendezvous;
use rendezvous::{Rendezvous, WaitError};
use std::time::{Duration, Instant};

#[test]
fn missing_participant_fails_with_bounded_diagnostic() {
    let gate = Rendezvous::new(Duration::from_millis(30));
    let start = Instant::now();
    assert_eq!(gate.arrive(), Err(WaitError::TimedOut));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(gate.snapshot().unwrap().arrivals, 1);
    assert!(gate.snapshot().unwrap().timed_out);
}

#[test]
fn participant_returning_early_releases_peer_without_accepting_success() {
    let gate = Rendezvous::new(Duration::from_secs(2));
    let results = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| gate.arrive());
        let early = scope.spawn(|| {
            let result = Err::<(), _>("fixture preflight failed before rendezvous");
            gate.participant_finished();
            result
        });
        (waiting.join().unwrap(), early.join().unwrap())
    });
    assert_eq!(results.0, Err(WaitError::Aborted));
    assert!(results.1.is_err());
    assert!(gate.snapshot().unwrap().aborted);
    assert!(!gate.snapshot().unwrap().timed_out);
}

#[test]
fn both_participants_are_released_and_completion_does_not_revoke_gate() {
    let gate = Rendezvous::new(Duration::from_secs(2));
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let result = gate.arrive();
            gate.participant_finished();
            result
        });
        let second = scope.spawn(|| {
            let result = gate.arrive();
            gate.participant_finished();
            result
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(results, (Ok(()), Ok(())));
    assert_eq!(gate.snapshot().unwrap().arrivals, 2);
    assert!(!gate.snapshot().unwrap().aborted);
}
