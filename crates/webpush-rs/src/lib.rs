//! PyO3 binding for Web Push. Exposes a single sync `send_one` to Python; the
//! crypto (VAPID + AES128GCM) and HTTP send run inside a module-level Tokio
//! runtime. The runtime is multi-thread but pinned to a SMALL worker + blocking
//! pool: enough for the Python side to fan sends out concurrently (it dispatches
//! send_one via asyncio.to_thread), without the per-core thread pool the default
//! multi-thread builder spins up (measured: 33 resident threads vs the handful
//! below). `block_on` is also concurrency-safe here — tokio explicitly supports
//! concurrent block_on from multiple OS threads on a shared runtime.

mod core;

#[cfg(feature = "python")]
mod py {
    use crate::core;
    use pyo3::prelude::*;
    use std::sync::OnceLock;
    use tokio::runtime::Runtime;
    use web_push::HyperWebPushClient;

    // One small multi-thread runtime + one reusable client for the whole
    // process. 2 workers + a tiny blocking pool keeps concurrent fan-out cheap
    // without the default builder's per-core pool.
    fn runtime() -> &'static Runtime {
        static RT: OnceLock<Runtime> = OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(2)
                .enable_all()
                .build()
                .expect("tokio rt")
        })
    }
    fn client() -> &'static HyperWebPushClient {
        static C: OnceLock<HyperWebPushClient> = OnceLock::new();
        C.get_or_init(HyperWebPushClient::new)
    }

    /// Send one push notification. Returns the HTTP status (201 ok; 404/410 =
    /// dead endpoint, caller should delete; 0 = local error, see `raise`).
    /// Blocking: call via `asyncio.to_thread` from async Python.
    #[pyfunction]
    #[pyo3(signature = (endpoint, p256dh, auth, vapid_b64, subject, payload))]
    fn send_one(
        py: Python<'_>,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        vapid_b64: &str,
        subject: &str,
        payload: &[u8],
    ) -> PyResult<u16> {
        // Release the GIL: the send is network/CPU in Rust, touches no Python.
        let outcome = py.allow_threads(|| {
            runtime().block_on(core::send_one(
                client(), endpoint, p256dh, auth, vapid_b64, subject, payload,
            ))
        });
        if let Some(err) = outcome.error {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(err));
        }
        Ok(outcome.status)
    }

    #[pymodule]
    fn webpush_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_function(wrap_pyfunction!(send_one, m)?)?;
        Ok(())
    }
}
