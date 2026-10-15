//! Memory figures for development: resident size and glibc heap use.
//!
//! Enabled with `SPACEPODS_MEMSTATS=<seconds>`. Logs one line per interval under the
//! `MEM` category. Off by default; costs one small file read per interval when on.

use crate::log::Log;

/// Start logging if the environment asks for it.
pub fn start_if_enabled() {
    let Some(secs) = std::env::var("SPACEPODS_MEMSTATS").ok().and_then(|v| v.parse::<u32>().ok()) else {
        return;
    };
    let secs = secs.max(1);
    log_once("start");
    glib::timeout_add_seconds_local(secs, || {
        log_once("tick");
        glib::ControlFlow::Continue
    });
}

/// Log one sample now, tagged with `label`.
pub fn log_once(label: &str) {
    let st = proc_status();
    let heap = heap_in_use();
    Log::info(
        "MEM",
        &format!(
            "{label}: rss={:.1} [anon={:.1} file={:.1} shmem={:.1}] MiB heap_in_use={:.1} mapped={:.1} MiB",
            st.rss as f64 / 1024.0,
            st.anon as f64 / 1024.0,
            st.file as f64 / 1024.0,
            st.shmem as f64 / 1024.0,
            heap.0 as f64 / 1048576.0,
            heap.1 as f64 / 1048576.0,
        ),
    );
}

/// Resident sizes from /proc/self/status, in KiB. `rss` is the total; `anon`, `file`
/// and `shmem` are its parts (private heap, mapped files, shared memory).
struct ProcStatus {
    rss: u64,
    anon: u64,
    file: u64,
    shmem: u64,
}

fn proc_status() -> ProcStatus {
    let text = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    ProcStatus {
        rss: field("VmRSS:"),
        anon: field("RssAnon:"),
        file: field("RssFile:"),
        shmem: field("RssShmem:"),
    }
}

/// (bytes in use by malloc, bytes mapped for malloc), from mallinfo2.
fn heap_in_use() -> (u64, u64) {
    // SAFETY: mallinfo2 only reads allocator bookkeeping and has no preconditions.
    let info = unsafe { libc::mallinfo2() };
    (info.uordblks as u64, (info.hblkhd + info.arena) as u64)
}
