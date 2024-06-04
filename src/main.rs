#[allow(dead_code)]
mod backend;
mod config;
mod config_io;
mod graphics;
mod gui;
mod hid;
mod overlays;
mod shaders;
mod state;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use clap::Parser;
use flexi_logger::{Duplicate, FileSpec, LogSpecification};

/// The lightweight desktop overlay for OpenVR and OpenXR
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[cfg(feature = "openvr")]
    /// Force OpenVR backend
    #[arg(long)]
    openvr: bool,

    #[cfg(feature = "openxr")]
    /// Force OpenXR backend
    #[arg(long)]
    openxr: bool,

    /// Uninstall OpenVR manifest and exit
    #[arg(long)]
    uninstall: bool,

    /// Path to write logs to
    #[arg(short, long, value_name = "FILE_PATH")]
    log_to: Option<String>,

    #[cfg(feature = "uidev")]
    /// Show a desktop window of a UI panel for development
    #[arg(short, long, value_name = "UI_NAME")]
    uidev: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_BACKTRACE", "full");

    let mut args = Args::parse();
    logging_init(&mut args)?;

    log::info!(
        "Welcome to {} version {}!",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    log::info!("It is {}.", chrono::Local::now().format("%c"));

    #[cfg(feature = "openvr")]
    if args.uninstall {
        crate::backend::openvr::openvr_uninstall();
        return Ok(());
    }

    #[cfg(feature = "uidev")]
    if let Some(panel_name) = args.uidev.as_ref() {
        crate::backend::uidev::uidev_run(panel_name.as_str())?;
        return Ok(());
    }

    let running = Arc::new(AtomicBool::new(true));
    let _ = ctrlc::set_handler({
        let running = running.clone();
        move || {
            running.store(false, Ordering::Relaxed);
        }
    });

    auto_run(running, args);

    Ok(())
}

fn auto_run(running: Arc<AtomicBool>, args: Args) {
    use backend::common::BackendError;

    #[cfg(feature = "openxr")]
    if !args_get_openvr(&args) {
        use crate::backend::openxr::openxr_run;
        match openxr_run(running.clone()) {
            Ok(()) => return,
            Err(BackendError::NotSupported) => (),
            Err(e) => {
                log::error!("{}", e.to_string());
                return;
            }
        };
    }

    #[cfg(feature = "openvr")]
    if !args_get_openxr(&args) {
        use crate::backend::openvr::openvr_run;
        match openvr_run(running.clone()) {
            Ok(()) => return,
            Err(BackendError::NotSupported) => (),
            Err(e) => {
                log::error!("{}", e.to_string());
                return;
            }
        };
    }

    log::error!("No more backends to try");

    #[cfg(not(any(feature = "openvr", feature = "openxr")))]
    compile_error!("No VR support! Enable either openvr or openxr features!");

    #[cfg(not(any(feature = "wayland", feature = "x11")))]
    compile_error!("No desktop support! Enable either wayland or x11 features!");
}

#[allow(dead_code)]
fn args_get_openvr(_args: &Args) -> bool {
    #[cfg(feature = "openvr")]
    let ret = _args.openvr;

    #[cfg(not(feature = "openvr"))]
    let ret = false;

    ret
}

#[allow(dead_code)]
fn args_get_openxr(_args: &Args) -> bool {
    #[cfg(feature = "openxr")]
    let ret = _args.openxr;

    #[cfg(not(feature = "openxr"))]
    let ret = false;

    ret
}

fn logging_init(args: &mut Args) -> anyhow::Result<()> {
    let log_file = args
        .log_to
        .take()
        .or_else(|| std::env::var("WLX_LOGFILE").ok())
        .or_else(|| Some("/tmp/wlx.log".to_string()));

    if let Some(log_to) = log_file.filter(|s| !s.is_empty()) {
        if let Err(e) = file_logging_init(&log_to) {
            log::error!("Failed to initialize file logging: {}", e);
            flexi_logger::Logger::try_with_env_or_str("info")?.start()?;
        }
    } else {
        flexi_logger::Logger::try_with_env_or_str("info")?.start()?;
    }

    log_panics::init();
    Ok(())
}

fn file_logging_init(log_to: &str) -> anyhow::Result<()> {
    let file_spec = FileSpec::try_from(PathBuf::from(log_to))?;
    let log_spec = LogSpecification::env_or_parse("info")?;

    let duplicate = log_spec
        .module_filters()
        .iter()
        .find(|m| m.module_name.is_none())
        .map(|m| match m.level_filter {
            log::LevelFilter::Trace => Duplicate::Trace,
            log::LevelFilter::Debug => Duplicate::Debug,
            log::LevelFilter::Info => Duplicate::Info,
            log::LevelFilter::Warn => Duplicate::Warn,
            _ => Duplicate::Error,
        });

    flexi_logger::Logger::with(log_spec)
        .log_to_file(file_spec)
        .duplicate_to_stderr(duplicate.unwrap_or(Duplicate::Error))
        .start()?;
    println!("Logging to: {}", log_to);
    Ok(())
}
