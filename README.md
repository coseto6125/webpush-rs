# webpush-rs

Fast [Web Push](https://datatracker.ietf.org/doc/html/rfc8030) (VAPID + AES128GCM)
for Python — the Rust [`web-push`](https://crates.io/crates/web-push) crate behind
a [PyO3](https://pyo3.rs) binding.

The VAPID JWT signing and payload encryption run in native Rust; Python sees a
single blocking `send_one`. Compared to a pure-Python sender it roughly halves
per-send CPU and adds no Python dependency footprint (the crypto is statically
linked, not pulled in as `cryptography`/`http-ece`).

> End-to-end latency is dominated by the network round-trip to the push service
> (FCM/Apple), so this is not "faster delivery" — it's lower CPU and memory on
> the sending host, which matters when fanning out to many subscribers.

## Install

```bash
pip install webpush-rs        # or: uv pip install webpush-rs
```

Prebuilt wheels are published for CPython 3.8+ (abi3, one wheel per platform) on
Linux (manylinux x86_64 + aarch64), macOS, and Windows.

## Usage

```python
import asyncio
import webpush_rs

# send_one is blocking (it drives a small internal Tokio runtime); call it off
# the event loop from async code.
status = await asyncio.to_thread(
    webpush_rs.send_one,
    endpoint,        # subscription.endpoint
    p256dh,          # subscription.keys.p256dh
    auth,            # subscription.keys.auth
    vapid_private,   # url-safe base64 VAPID private key
    "mailto:you@example.com",  # VAPID subject (mailto: or https URL)
    payload_bytes,   # the notification payload
)
# 201 = delivered; 404/410 = endpoint gone (delete the subscription); 0 = error (raises).
```

## License

MIT OR Apache-2.0.
