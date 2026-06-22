use crate::email_policy::PatchworkPolicy;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize)]
struct PatchworkListResponse {
    id: u64,
}

#[derive(Debug, Serialize)]
struct PatchworkCheckRequest {
    state: String,
    target_url: String,
    description: String,
    context: String,
}

pub async fn post_patchwork_check(
    policy: &PatchworkPolicy,
    msgid: &str,
    status: &str,
    description: &str,
    target_url: &str,
) {
    if !policy.enabled {
        return;
    }

    let api_url = match &policy.api_url {
        Some(url) => url.trim_end_matches('/'),
        None => {
            warn!("Patchwork enabled but no api_url provided");
            return;
        }
    };

    let client = Client::new();

    // 1. Get the patch ID using the Message-ID
    // Patchwork expects the msgid to be without the angle brackets, or URL encoded.
    // The msgid from the database or headers might contain `<` and `>`.
    let clean_msgid = msgid.trim_matches(|c| c == '<' || c == '>');
    let list_url = format!("{}/patches/?msgid={}", api_url, clean_msgid);

    debug!("Fetching Patchwork ID from: {}", list_url);

    let mut get_req = client.get(&list_url);
    if let Some(token) = &policy.token {
        get_req = get_req.header(header::AUTHORIZATION, format!("Token {}", token));
    }

    let resp = match get_req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to fetch patchwork patch list: {}", e);
            return;
        }
    };

    if !resp.status().is_success() {
        error!(
            "Patchwork API returned {}: {:?}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
        return;
    }

    let patches: Vec<PatchworkListResponse> = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to parse patchwork list response: {}", e);
            return;
        }
    };

    if patches.is_empty() {
        debug!("Patchwork returned no patches for msgid {}", msgid);
        return;
    }

    // We take the first match if there are multiple.
    let patch_id = patches[0].id;
    let check_url = format!("{}/patches/{}/checks/", api_url, patch_id);

    let payload = PatchworkCheckRequest {
        state: status.to_string(),
        target_url: target_url.to_string(),
        description: description.to_string(),
        context: "sashiko".to_string(),
    };

    debug!("Posting check to Patchwork: {} {:?}", check_url, payload);

    let mut post_req = client.post(&check_url).json(&payload);
    if let Some(token) = &policy.token {
        post_req = post_req.header(header::AUTHORIZATION, format!("Token {}", token));
    }

    let post_resp = match post_req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to post patchwork check: {}", e);
            return;
        }
    };

    if post_resp.status().is_success() {
        info!("Successfully posted check to Patchwork for msgid {}", msgid);
    } else {
        error!(
            "Patchwork check post failed with status {}: {:?}",
            post_resp.status(),
            post_resp.text().await.unwrap_or_default()
        );
    }
}
