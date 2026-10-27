// SPDX-License-Identifier: LGPL-2.1-or-later

//! Talks to the ELAN 0c57 reader and collects fingerprint datasets.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use elan_0c57::device::{Error, Sensor, Touch};
use elan_0c57::i18n::{localize, tr, trf};
use elan_0c57::matcher::Prepared;

#[derive(Parser)]
#[command(version, about = tr("Open source tool for the ELAN 04f3:0c57 fingerprint reader"))]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(about = tr("Show firmware, sensor size and calibration values"))]
    Info,
    #[command(about = tr("Switch the LED on for a moment, to check that commands reach the reader"))]
    Led {
        #[arg(short, long, default_value_t = 2.0, help = tr("How long to keep it on, in seconds"))]
        seconds: f64,
    },
    #[command(about = tr("Capture touches of one finger into a dataset folder"))]
    Capture {
        #[arg(short, long, help = tr("Finger label, used as sub-folder (e.g. right-index)"))]
        finger: String,
        #[arg(short = 'n', long, default_value_t = 1, help = tr("Number of touches to capture"))]
        count: usize,
        #[arg(short, long, default_value = "dataset", help = tr("Dataset folder"))]
        dir: PathBuf,
        #[arg(short, long, default_value_t = 15.0, help = tr("Seconds to wait for each touch"))]
        timeout: f64,
        #[arg(long, help = tr("Also save touches that cover only part of the sensor"))]
        keep_partial: bool,
    },
}

fn main() -> Result<()> {
    let matches = localize(Cli::command()).get_matches();
    match Cli::from_arg_matches(&matches)?.command {
        Cmd::Info => info(),
        Cmd::Led { seconds } => led(seconds),
        Cmd::Capture { finger, count, dir, timeout, keep_partial } => {
            capture(&finger, count, &dir, timeout, keep_partial)
        }
    }
}

fn info() -> Result<()> {
    let mut sensor = Sensor::open_usb()?;
    println!("{:24} 0x{:04x}", tr("firmware:"), sensor.firmware);
    println!("{:24} {}x{}", tr("sensor size:"), sensor.dims.width, sensor.dims.height);
    println!("{:24} {}", tr("calibration mean:"), sensor.calibration_mean()?);
    let background = sensor.calibrate()?;
    println!("{:24} {} {}", tr("background mean:"), background.mean(), tr("(after calibration)"));
    sensor.stop()?;
    Ok(())
}

fn led(seconds: f64) -> Result<()> {
    let mut sensor = Sensor::open_usb()?;
    sensor.led_on()?;
    std::thread::sleep(Duration::from_secs_f64(seconds));
    sensor.stop()?;
    Ok(())
}

fn capture(finger: &str, count: usize, dir: &Path, timeout: f64, keep_partial: bool) -> Result<()> {
    anyhow::ensure!(
        !finger.is_empty() && finger.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        tr("use letters, digits, '-' or '_' for the finger name")
    );
    let folder = dir.join(finger);
    fs::create_dir_all(&folder).with_context(|| format!("creating {}", folder.display()))?;
    let mut sensor = Sensor::open_usb()?;
    let timeout = Duration::from_secs_f64(timeout);
    let mut done = 0;
    while done < count {
        println!("[{}/{count}] {}", done + 1, trf("touch the sensor with {finger}…", &[("finger", &finger)]));
        match sensor.capture(timeout) {
            Ok(touch) => {
                let prepared = Prepared::new(&touch.print);
                if prepared.is_partial() && !keep_partial {
                    let pct = format!("{:.0}", prepared.coverage() * 100.0);
                    println!(
                        "        {}",
                        trf("partial touch ({pct}% of the sensor): press the whole fingertip, try again", &[("pct", &pct)])
                    );
                } else {
                    let n = next_index(&folder)?;
                    save(&folder, n, &touch)?;
                    log(dir, finger, n, &touch)?;
                    let path = folder.join(format!("{n:03}.png"));
                    println!("        {}", trf("saved {path}", &[("path", &path.display())]));
                    done += 1;
                }
            }
            Err(Error::Timeout) => {
                println!("        {}", tr("no finger, stopping"));
                break;
            }
            Err(e @ (Error::FingerDuringCalibration | Error::Calibration)) => {
                println!("        {}", trf("{error} — lift the finger and try again", &[("error", &e)]));
            }
            Err(e) => return Err(e.into()),
        }
        println!("        {}", tr("lift the finger"));
        // Not every firmware supports it; a short pause is enough otherwise.
        if sensor.wait_finger_off(Duration::from_secs(10)).is_err() {
            std::thread::sleep(Duration::from_millis(500));
        }
        sensor.stop()?;
    }
    println!("{}", trf("{n} touch(es) saved in {path}", &[("n", &done), ("path", &folder.display())]));
    Ok(())
}

/// First free number in the folder, so new sessions add to old ones.
fn next_index(folder: &Path) -> Result<u32> {
    let mut max = 0;
    for entry in fs::read_dir(folder)? {
        let name = entry?.file_name();
        if let Some(n) = name.to_str().and_then(|s| s.strip_suffix("-raw.png")).and_then(|s| s.parse().ok()) {
            max = max.max(n);
        }
    }
    Ok(max + 1)
}

/// Per touch: the fingerprint (16 bit, background removed), the raw frame,
/// the background, and an enlarged 8 bit preview.
fn save(folder: &Path, n: u32, touch: &Touch) -> Result<()> {
    touch.print.save_png(&folder.join(format!("{n:03}-print.png")))?;
    touch.frame.save_png(&folder.join(format!("{n:03}-raw.png")))?;
    touch.background.save_png(&folder.join(format!("{n:03}-bg.png")))?;
    touch.print.normalize_percentile(2.0, 98.0).upscale(4).save_png(&folder.join(format!("{n:03}.png")))
}

fn log(dir: &Path, finger: &str, n: u32, touch: &Touch) -> Result<()> {
    let path = dir.join("index.csv");
    let new = !path.exists();
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    if new {
        writeln!(file, "finger,index,time,background_mean,print_mean")?;
    }
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    writeln!(file, "{finger},{n},{time},{},{}", touch.background.mean(), touch.print.mean())?;
    Ok(())
}
