use gitguard::{cli::CheckRequest, evidence::envelope::BoundCheck};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
};
static CANCELLED: AtomicBool = AtomicBool::new(false);
extern "C" fn cancel(_: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
}
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(_) => {
            eprintln!("gitguard: check failed; no successful result");
            4
        }
    };
    std::process::exit(code)
}
fn run() -> Result<i32, ()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["api"] {
        let bytes = gitguard::api::read_bounded(std::io::stdin()).map_err(|_| ())?;
        let result = gitguard::api::dispatch(&bytes, &CANCELLED);
        serde_json::to_writer(std::io::stdout().lock(), &result).map_err(|_| ())?;
        return Ok(result["exitCode"].as_i64().unwrap_or(4) as i32);
    }
    if args == ["mcp-stdio"] {
        gitguard::mcp::serve(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
            &CANCELLED,
        )
        .map_err(|_| ())?;
        return Ok(0);
    }
    if args != ["check"] {
        return Err(());
    }
    // Cooperative cancellation at observation boundaries; no Git write capability exists.
    unsafe {
        libc::signal(libc::SIGINT, cancel as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, cancel as *const () as libc::sighandler_t);
    }
    let mut bytes = vec![];
    std::io::stdin()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() > 1_048_576 {
        return Err(());
    }
    let request: CheckRequest = serde_json::from_slice(&bytes).map_err(|_| ())?;
    let bundle = BoundCheck::prepare(request)
        .map_err(|_| ())?
        .run(&CANCELLED)
        .map_err(|_| ())?;
    let code = bundle.exit_code();
    if code == 4 {
        eprintln!("gitguard: bound check did not complete");
    }
    let bytes = serde_json::to_vec(&bundle).map_err(|_| ())?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes).map_err(|_| ())?;
    stdout.write_all(b"\n").map_err(|_| ())?;
    Ok(code)
}
