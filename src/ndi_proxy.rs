use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use crate::config::{AudioMode, NdiConfig};
use crate::ndi_raw::{Captured, Finder, Recv, Sender, Source};

pub fn spawn(
    cfg: NdiConfig,
    audio_mode: AudioMode,
    muted: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ndi-proxy".to_string())
        .spawn(move || run(cfg, audio_mode, muted, shutdown))
        .expect("failed to spawn NDI thread")
}

pub fn list_sources() -> Result<()> {
    ndi::initialize().context("failed to initialize NDI")?;
    let finder = Finder::new()?;
    let deadline = Instant::now() + Duration::from_secs(2);

    loop {
        let sources = finder.sources();
        if !sources.is_empty() {
            for source in sources {
                println!("{}", source.name);
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            println!("no NDI sources found");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn run(cfg: NdiConfig, audio_mode: AudioMode, muted: Arc<AtomicBool>, shutdown: Arc<AtomicBool>) {
    if let Err(err) = ndi::initialize() {
        error!(%err, "failed to initialize NDI");
        return;
    }

    info!(
        source = %cfg.source_name,
        output = %cfg.output_name,
        "starting NDI proxy"
    );

    let sender = loop {
        if shutdown.load(Ordering::Relaxed) {
            return;
        }
        match build_sender(&cfg) {
            Ok(sender) => break sender,
            Err(err) => {
                warn!(%err, "failed to create NDI sender; retrying");
                thread::sleep(Duration::from_millis(cfg.reconnect_delay_ms.max(100)));
            }
        }
    };
    info!(output = %cfg.output_name, "NDI sender ready");

    let finder = loop {
        if shutdown.load(Ordering::Relaxed) {
            return;
        }
        match Finder::new() {
            Ok(finder) => break finder,
            Err(err) => {
                warn!(%err, "failed to create NDI finder; retrying");
                thread::sleep(Duration::from_millis(cfg.reconnect_delay_ms.max(100)));
            }
        }
    };

    let mut last_mute = muted.load(Ordering::Relaxed);
    let mut last_discovery_log = Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        let source = match find_source(&finder, &cfg, &mut last_discovery_log) {
            Some(source) => source,
            None => {
                thread::sleep(Duration::from_millis(cfg.reconnect_delay_ms.max(100)));
                continue;
            }
        };

        info!(source = %source.name, "OBS NDI source found");
        let receiver = match build_receiver(&cfg, &source) {
            Ok(receiver) => receiver,
            Err(err) => {
                warn!(%err, "failed to create NDI receiver; retrying");
                thread::sleep(Duration::from_millis(cfg.reconnect_delay_ms.max(100)));
                continue;
            }
        };

        info!("NDI receiver connected");
        forward_loop(
            &receiver,
            &sender,
            &cfg,
            audio_mode,
            &muted,
            &shutdown,
            &mut last_mute,
        );

        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        warn!("NDI source disconnected; reconnecting");
        thread::sleep(Duration::from_millis(cfg.reconnect_delay_ms.max(100)));
    }

    info!("NDI proxy stopped");
}

fn build_sender(cfg: &NdiConfig) -> Result<Sender> {
    Sender::new(
        &cfg.output_name,
        cfg.output_groups.as_deref(),
        cfg.clock_video,
        cfg.clock_audio,
    )
}

fn build_receiver(cfg: &NdiConfig, source: &Source) -> Result<Recv> {
    Recv::new(source, cfg.color_format, cfg.bandwidth)
}

fn find_source(finder: &Finder, cfg: &NdiConfig, last_log: &mut Instant) -> Option<Source> {
    let sources = finder.sources();
    let candidates: Vec<_> = sources
        .into_iter()
        .filter(|source| !is_own_source(&source.name, &cfg.output_name))
        .collect();

    if let Some(found) = candidates
        .iter()
        .find(|source| exact_match(&source.name, &cfg.source_name))
    {
        return Some(found.clone());
    }

    if let Some(found) = candidates
        .iter()
        .find(|source| contains_match(&source.name, &cfg.source_name))
    {
        return Some(found.clone());
    }

    if last_log.elapsed() >= Duration::from_secs(5) {
        *last_log = Instant::now();
        let names: Vec<_> = candidates
            .iter()
            .map(|source| source.name.clone())
            .collect();
        warn!(?names, wanted = %cfg.source_name, "no NDI source matched");
    }

    None
}

fn forward_loop(
    receiver: &Recv,
    sender: &Sender,
    cfg: &NdiConfig,
    audio_mode: AudioMode,
    muted: &Arc<AtomicBool>,
    shutdown: &Arc<AtomicBool>,
    last_mute: &mut bool,
) {
    let capture_timeout_ms = cfg.capture_timeout_ms.min(u32::MAX as u64) as u32;
    let status_interval = Duration::from_millis(cfg.status_interval_ms.max(1000));
    let mut last_status = Instant::now();
    let mut last_connections = sender.connections();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            return;
        }

        let current_mute = muted.load(Ordering::Relaxed);
        if current_mute != *last_mute {
            *last_mute = current_mute;
            if current_mute {
                info!("mute on: NDI audio is gated");
            } else {
                info!("mute off: NDI audio is forwarded");
            }
        }

        match receiver.capture(capture_timeout_ms) {
            Captured::Video(frame) => sender.send_video(frame.raw()),
            Captured::Audio(frame) => {
                if !current_mute {
                    sender.send_audio(frame.raw());
                } else if audio_mode == AudioMode::Silence {
                    sender.send_silence(frame.raw());
                }
            }
            Captured::None => {}
            Captured::Error => {
                warn!("NDI receiver reported an error");
                return;
            }
        }

        if last_status.elapsed() >= status_interval {
            last_status = Instant::now();
            let connections = sender.connections();
            if connections != last_connections {
                last_connections = connections;
                info!(connections, "NDI output receiver count changed");
            }
        }
    }
}

fn exact_match(name: &str, wanted: &str) -> bool {
    name == wanted || name.ends_with(&format!("({wanted})"))
}

fn contains_match(name: &str, wanted: &str) -> bool {
    !wanted.is_empty() && name.contains(wanted)
}

fn is_own_source(name: &str, output_name: &str) -> bool {
    name == output_name || name.ends_with(&format!("({output_name})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ndi_source_names() {
        assert!(exact_match("HOST (OBS-PGM)", "OBS-PGM"));
        assert!(exact_match("OBS-PGM", "OBS-PGM"));
        assert!(contains_match("HOST (OBS-PGM)", "OBS"));
        assert!(!contains_match("HOST (OBS-PGM)", ""));
        assert!(is_own_source("HOST (OTOME-OUT)", "OTOME-OUT"));
        assert!(!is_own_source("HOST (OBS-PGM)", "OTOME-OUT"));
    }
}
