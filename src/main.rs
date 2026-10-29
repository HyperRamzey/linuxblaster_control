#![allow(unused)]

use std::path::PathBuf;
use std::sync::LazyLock;

use eframe::egui::{self, Vec2, ahash::HashMap, mutex::Mutex};
use linuxblaster_control::BlasterXG6;
use tracing::{Level, warn};

mod app;
use app::Launcher;

fn main() -> eframe::Result<()> {
    // RUST_LOG overrides this, e.g. RUST_LOG=debug linuxblaster_control
    let default_level = if cfg!(debug_assertions) {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(default_level.into())
                .from_env_lossy(),
        )
        .init();

    let startup_profile = match parse_args(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            print!("{}", USAGE);
            return Ok(());
        }
        Ok(Command::Run(None)) => None,
        Ok(Command::Run(Some(StartupProfile::File(path)))) => Some(path),
        Ok(Command::Run(Some(StartupProfile::Last))) => {
            let last = linuxblaster_control::last_profile();
            if last.is_none() {
                warn!(
                    "--apply-last: no profile has been loaded or saved yet, \
                     nothing to apply"
                );
            }
            last
        }
        Err(message) => {
            eprintln!("{}\n\n{}", message, USAGE);
            std::process::exit(2);
        }
    };

    let app = Launcher::new(startup_profile);

    let icon = eframe::icon_data::from_png_bytes(include_bytes!(
        "../packaging/linuxblaster-control.png"
    ))
    .expect("bundled icon is a valid PNG");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Linuxblaster Control")
            // matches the .desktop file, so the taskbar shows the right icon
            .with_app_id("linuxblaster-control")
            .with_icon(icon)
            .with_resizable(true)
            .with_inner_size(Vec2::new(1050.0, 600.0))
            // the two-column panes need at least this much room
            .with_min_inner_size(Vec2::new(1050.0, 600.0)),
        // .with_inner_size()
        ..Default::default()
    };

    eframe::run_native(
        "Linuxblaster Control",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            cc.egui_ctx.set_pixels_per_point(1.5);

            #[cfg(debug_assertions)]
            {
                cc.egui_ctx.debug_painter();
                cc.egui_ctx.set_debug_on_hover(false);
            }

            Ok(Box::new(app))
        }),
    )
}

const USAGE: &str = "\
Usage: linuxblaster_control [OPTIONS]

Options:
  --apply-last            Apply the profile that was loaded or saved last
  --apply-profile <FILE>  Apply this profile
  -h, --help              Print this help

The profile is applied to the active output once the G6 is connected.
Only settings that differ from the device's current state are written.
";

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    Run(Option<StartupProfile>),
}

#[derive(Debug, PartialEq)]
enum StartupProfile {
    Last,
    File(PathBuf),
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Command, String> {
    let mut profile = None;
    while let Some(arg) = args.next() {
        let next = match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--apply-last" => StartupProfile::Last,
            "--apply-profile" => {
                let path = args
                    .next()
                    .ok_or("--apply-profile needs a profile file")?;
                StartupProfile::File(PathBuf::from(path))
            }
            other => match other.strip_prefix("--apply-profile=") {
                Some(path) if !path.is_empty() => {
                    StartupProfile::File(PathBuf::from(path))
                }
                Some(_) => return Err("--apply-profile needs a profile file".into()),
                None => return Err(format!("Unknown argument: {}", other)),
            },
        };
        if profile.is_some() {
            return Err(
                "Use only one of --apply-last and --apply-profile".into()
            );
        }
        profile = Some(next);
    }
    Ok(Command::Run(profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn startup_profile_arguments() {
        assert_eq!(parse(&[]), Ok(Command::Run(None)));
        assert_eq!(
            parse(&["--apply-last"]),
            Ok(Command::Run(Some(StartupProfile::Last)))
        );
        let file = Ok(Command::Run(Some(StartupProfile::File(
            PathBuf::from("a b.json"),
        ))));
        assert_eq!(parse(&["--apply-profile", "a b.json"]), file);
        assert_eq!(parse(&["--apply-profile=a b.json"]), file);
        assert_eq!(parse(&["--help", "--bogus"]), Ok(Command::Help));
        assert!(parse(&["--apply-profile"]).is_err());
        assert!(parse(&["--apply-profile="]).is_err());
        assert!(parse(&["--apply-last", "--apply-profile", "x"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }
}

/// Results from "<headphone_name> FixedBandEQ.txt"
/// mapped to ten bands (31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz)
#[derive(Debug, Clone)]
pub struct HeadphoneResult {
    pub tester: &'static str,
    pub variant: Option<&'static str>,
    pub test_device: Option<&'static str>,
    pub preamp: f32,
    pub ten_band_eq: [f32; 10],
}

struct AutoEqDb {
    // HashMap
    // Key: Name (DT 990 Pro (250 Ohm))
    // Value: Vec<HeadphoneResult>
    results:
        Option<&'static phf::Map<&'static str, &'static [HeadphoneResult]>>,
}

include!(concat!(env!("OUT_DIR"), "/autoeq_db.rs"));
