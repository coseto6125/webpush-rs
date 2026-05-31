//! Shared Web Push send core, independent of the Python binding. Builds the
//! VAPID signature + AES128GCM-encrypted message and sends it via the hyper
//! client. Returns the HTTP status (or a status surrogate for endpoint-gone
//! errors so the caller can prune dead subscriptions).

use web_push::{
    ContentEncoding, HyperWebPushClient, SubscriptionInfo, VapidSignatureBuilder,
    WebPushClient, WebPushError, WebPushMessageBuilder,
};

/// HTTP status to report. For `EndpointNotValid` / `EndpointNotFound` the
/// crate already mapped the upstream 404/410 — surface those so the caller
/// deletes the subscription. Anything else maps to a 0 + the error string.
pub struct SendOutcome {
    pub status: u16,
    pub error: Option<String>,
}

/// Send one push. `vapid_b64` is the raw url-safe-no-pad VAPID private key
/// (the format `web-push generate-vapid-keys` and the browser both use).
pub async fn send_one(
    client: &HyperWebPushClient,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    vapid_b64: &str,
    subject: &str,
    payload: &[u8],
) -> SendOutcome {
    let sub = SubscriptionInfo::new(endpoint, p256dh, auth);

    let mut sig = match VapidSignatureBuilder::from_base64(vapid_b64, &sub) {
        Ok(s) => s,
        Err(e) => return SendOutcome { status: 0, error: Some(format!("vapid: {e}")) },
    };
    sig.add_claim("sub", subject);
    let signature = match sig.build() {
        Ok(s) => s,
        Err(e) => return SendOutcome { status: 0, error: Some(format!("vapid-build: {e}")) },
    };

    let mut builder = WebPushMessageBuilder::new(&sub);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload);
    builder.set_vapid_signature(signature);
    let message = match builder.build() {
        Ok(m) => m,
        Err(e) => return SendOutcome { status: 0, error: Some(format!("build: {e}")) },
    };

    match client.send(message).await {
        Ok(()) => SendOutcome { status: 201, error: None },
        Err(WebPushError::EndpointNotValid(_)) => SendOutcome { status: 410, error: None },
        Err(WebPushError::EndpointNotFound(_)) => SendOutcome { status: 404, error: None },
        Err(e) => SendOutcome { status: 0, error: Some(e.to_string()) },
    }
}
