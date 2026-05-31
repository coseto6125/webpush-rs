//! Native Rust benchmark mirroring bench_pywebpush.py: crypto (VAPID +
//! AES128GCM) + HTTP POST to a loopback endpoint, N iterations. Reports CPU
//! time, wall time, peak RSS, and resident thread count as JSON so the numbers
//! line up directly against the Python harness.

mod core;

use std::time::Instant;

fn peak_rss_mb() -> f64 {
    // ru_maxrss in KB on Linux.
    let usage = unsafe {
        let mut u: libc_rusage = std::mem::zeroed();
        getrusage(0, &mut u);
        u
    };
    usage.ru_maxrss as f64 / 1024.0
}

// Minimal getrusage FFI (avoid a libc crate dep for the bench).
// glibc struct rusage on x86-64 is 144 bytes: two timevals (16 each), then
// ru_maxrss + 15 more long fields. We only read ru_maxrss; _rest pads the tail.
#[repr(C)]
#[derive(Clone, Copy)]
struct libc_rusage {
    ru_utime: [i64; 2],
    ru_stime: [i64; 2],
    ru_maxrss: i64,
    _rest: [i64; 13],
}
extern "C" {
    fn getrusage(who: i32, usage: *mut libc_rusage) -> i32;
}

fn count_threads() -> usize {
    std::fs::read_dir("/proc/self/task").map(|d| d.count()).unwrap_or(0)
}

async fn run(n: usize, flavor: &str, multi: bool) {
    let endpoint = "http://127.0.0.1:8099/push/FAKE";
    let p256dh = std::env::var("BENCH_P256DH").unwrap();
    let auth = std::env::var("BENCH_AUTH").unwrap();
    let vapid = std::env::var("BENCH_VAPID").unwrap();
    let subject = "mailto:bench@example.com";
    let payload =
        format!("{{\"title\":\"bench\",\"body\":\"{}\"}}", "x".repeat(200)).into_bytes();

    let client = web_push::HyperWebPushClient::new();
    let rss_before = peak_rss_mb();

    // warm
    let _ = core::send_one(&client, endpoint, &p256dh, &auth, &vapid, subject, &payload).await;

    let threads = count_threads();
    let cpu0 = cpu_time();
    let wall0 = Instant::now();
    for _ in 0..n {
        let _ = core::send_one(&client, endpoint, &p256dh, &auth, &vapid, subject, &payload).await;
    }
    let wall = wall0.elapsed().as_secs_f64();
    let cpu = cpu_time() - cpu0;
    let rss = peak_rss_mb();

    println!(
        "{{\"impl\":\"rust-{flavor}\",\"iterations\":{n},\"peak_rss_mb\":{:.1},\"rss_delta_total_mb\":{:.2},\"cpu_per_send_us\":{:.1},\"wall_per_send_us\":{:.1},\"threads_resident\":{threads},\"runtime\":\"{}\"}}",
        rss,
        rss - rss_before,
        cpu / n as f64 * 1e6,
        wall / n as f64 * 1e6,
        if multi { "multi-thread" } else { "current-thread" },
    );
}

fn cpu_time() -> f64 {
    let u = unsafe {
        let mut u: libc_rusage = std::mem::zeroed();
        getrusage(0, &mut u);
        u
    };
    (u.ru_utime[0] + u.ru_stime[0]) as f64 + (u.ru_utime[1] + u.ru_stime[1]) as f64 / 1e6
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let multi = std::env::args().nth(2).as_deref() == Some("multi");

    if multi {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        rt.block_on(run(n, "multi", true));
    } else {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(run(n, "current", false));
    }
}
