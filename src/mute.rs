use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use crate::config::{MuteConfig, MuteMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuteState {
    pub is_muted: bool,
}

#[derive(Clone)]
struct ApiState {
    muted: Arc<AtomicBool>,
    client: reqwest::Client,
    forward_url: Option<String>,
}

pub async fn run(
    cfg: &MuteConfig,
    muted: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    match cfg.mode {
        MuteMode::Api => run_api(cfg, muted, shutdown).await,
        MuteMode::Poll => run_poll(cfg, muted, shutdown).await,
    }
}

async fn run_api(
    cfg: &MuteConfig,
    muted: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let addr: SocketAddr = cfg
        .listen
        .parse()
        .with_context(|| format!("invalid mute.listen address: {}", cfg.listen))?;

    let forward_url = if cfg.forward_to_remote {
        Some(format!("{}/mute", cfg.remote_url.trim_end_matches('/')))
    } else {
        None
    };

    let state = ApiState {
        muted: muted.clone(),
        client: reqwest::Client::new(),
        forward_url,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/mute", get(get_mute).post(post_mute))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind mute API on {addr}"))?;

    info!(%addr, "mute API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(wait_shutdown(shutdown))
        .await
        .context("mute API server failed")?;
    info!("mute API stopped");
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn get_mute(State(state): State<ApiState>) -> Json<MuteState> {
    Json(MuteState {
        is_muted: state.muted.load(Ordering::Relaxed),
    })
}

async fn post_mute(State(state): State<ApiState>, Json(body): Json<MuteState>) -> Json<MuteState> {
    state.muted.store(body.is_muted, Ordering::Relaxed);
    info!(is_muted = body.is_muted, "mute state updated");

    if let Some(url) = state.forward_url.clone() {
        let client = state.client.clone();
        let is_muted = body.is_muted;
        tokio::spawn(async move {
            match client.post(&url).json(&MuteState { is_muted }).send().await {
                Ok(response) if response.status().is_success() => {
                    info!(%url, is_muted, "forwarded mute state");
                }
                Ok(response) => {
                    warn!(%url, status = %response.status(), "failed to forward mute state");
                }
                Err(err) => {
                    warn!(%url, %err, "failed to forward mute state");
                }
            }
        });
    }

    Json(body)
}

async fn run_poll(
    cfg: &MuteConfig,
    muted: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let url = format!("{}/mute", cfg.remote_url.trim_end_matches('/'));
    let interval = Duration::from_millis(cfg.poll_interval_ms.max(50));
    let client = reqwest::Client::new();
    let mut last = muted.load(Ordering::Relaxed);

    info!(%url, "polling remote mute state");
    loop {
        if shutdown.load(Ordering::Relaxed) {
            info!("mute poller stopped");
            return Ok(());
        }

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<MuteState>().await {
                    Ok(state) => {
                        if state.is_muted != last {
                            last = state.is_muted;
                            muted.store(last, Ordering::Relaxed);
                            info!(is_muted = last, "mute state updated");
                        }
                    }
                    Err(err) => warn!(%url, %err, "invalid mute response"),
                }
            }
            Ok(response) => {
                warn!(%url, status = %response.status(), "mute request failed");
            }
            Err(err) => {
                warn!(%url, %err, "mute request failed");
            }
        }

        tokio::time::sleep(interval).await;
    }
}

async fn wait_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
