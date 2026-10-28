mod config;
mod mute;
mod ndi_proxy;
mod ndi_raw;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "otome",
    version,
    about = "NDI audio gate controlled by ultradonguri mute"
)]
struct Cli {
    #[arg(short, long, default_value = "otome.toml")]
    config: PathBuf,

    #[arg(long)]
    source_name: Option<String>,

    #[arg(long)]
    output_name: Option<String>,

    #[arg(long)]
    mute_listen: Option<String>,

    #[arg(long)]
    list_sources: bool,

    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = config::load(&cli.config).context("failed to load config")?;

    if let Some(value) = cli.source_name {
        cfg.ndi.source_name = value;
    }
    if let Some(value) = cli.output_name {
        cfg.ndi.output_name = value;
    }
    if let Some(value) = cli.mute_listen {
        cfg.mute.listen = value;
    }

    cfg.validate()?;
    init_tracing(&cfg.log.level);

    if cli.list_sources {
        ndi_proxy::list_sources()?;
        return Ok(());
    }

    info!(
        config = %cli.config.display(),
        dry_run = cli.dry_run,
        "otome starting"
    );

    let muted = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));

    let ndi_handle = if cli.dry_run {
        info!("dry-run: NDI proxy disabled");
        None
    } else {
        Some(ndi_proxy::spawn(
            cfg.ndi.clone(),
            cfg.mute.audio_mode,
            muted.clone(),
            shutdown.clone(),
        ))
    };

    let ctrl_c = tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("shutdown requested");
                shutdown.store(true, Ordering::Relaxed);
            }
        }
    });

    let result = mute::run(&cfg.mute, muted.clone(), shutdown.clone()).await;

    shutdown.store(true, Ordering::Relaxed);
    ctrl_c.abort();

    if let Some(handle) = ndi_handle {
        let _ = handle.join();
    }

    if let Err(err) = result {
        error!(%err, "mute service failed");
        return Err(err);
    }

    info!("otome stopped");
    Ok(())
}

fn init_tracing(level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
