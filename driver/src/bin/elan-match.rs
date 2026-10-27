// SPDX-License-Identifier: LGPL-2.1-or-later

//! Offline evaluation of the matcher on a dataset collected with elan-capture.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use elan_0c57::i18n::{localize, tr, trf};
use elan_0c57::image::Frame;
use elan_0c57::matcher::{Matcher, Prepared, THRESHOLD, Template};

#[derive(Parser)]
#[command(version, about = tr("Evaluate the fingerprint matcher on a dataset"))]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(about = tr(
        "Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py"
    ))]
    Score {
        #[arg(default_value = "dataset")]
        dataset: PathBuf,
        #[arg(long, default_value_t = 12)]
        enroll: usize,
    },
    #[command(about = tr("Time of one verification (one probe against a template), on one core"))]
    Bench {
        #[arg(default_value = "dataset")]
        dataset: PathBuf,
        #[arg(long, default_value_t = 12)]
        enroll: usize,
    },
    #[command(about = tr("Error rates over random splits of enrolled / verification touches"))]
    Eval {
        #[arg(default_value = "dataset")]
        dataset: PathBuf,
        #[arg(long, default_value_t = 12)]
        enroll: usize,
        #[arg(long, default_value_t = 5)]
        splits: usize,
        #[arg(long, help = tr("Also use partial touches (the driver would ask to repeat them)"))]
        keep_partial: bool,
    },
}

struct Finger {
    name: String,
    touches: Vec<Prepared>,
}

fn main() -> Result<()> {
    let matches = localize(Cli::command()).get_matches();
    match Cli::from_arg_matches(&matches)?.command {
        Cmd::Score { dataset, enroll } => score(&dataset, enroll),
        Cmd::Eval { dataset, enroll, splits, keep_partial } => eval(&dataset, enroll, splits, keep_partial),
        Cmd::Bench { dataset, enroll } => bench(&dataset, enroll),
    }
}

fn load(dataset: &Path) -> Result<Vec<Finger>> {
    let mut folders: Vec<_> = std::fs::read_dir(dataset)
        .with_context(|| format!("reading {}", dataset.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();
    folders
        .into_iter()
        .map(|folder| {
            let mut files: Vec<_> = std::fs::read_dir(&folder)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.to_string_lossy().ends_with("-print.png"))
                .collect();
            files.sort();
            let touches = files.iter().map(|f| Frame::load_png(f).map(|fr| Prepared::new(&fr))).collect::<Result<_>>()?;
            Ok(Finger { name: folder.file_name().unwrap().to_string_lossy().into_owned(), touches })
        })
        .collect()
}

/// Scores of every probe against every template, computed on all CPU cores.
/// Returns (probe finger, probe index, template finger, score).
fn all_scores(fingers: &[Finger], enrolled: &[Vec<usize>], probes: &[Vec<usize>]) -> Vec<(usize, usize, usize, f64)> {
    // One comparison per thread here, so each comparison runs on one core.
    let matcher = Matcher::with_threads(1);
    let templates: Vec<Template> = fingers
        .iter()
        .zip(enrolled)
        .map(|(f, idx)| matcher.template(&idx.iter().map(|&i| f.touches[i].clone()).collect::<Vec<_>>()))
        .collect();
    let jobs: Vec<(usize, usize, usize)> = probes
        .iter()
        .enumerate()
        .flat_map(|(f, idx)| idx.iter().flat_map(move |&i| (0..fingers.len()).map(move |g| (f, i, g))))
        .collect();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut out = vec![0.0; jobs.len()];
    std::thread::scope(|s| {
        for (chunk_jobs, chunk_out) in jobs.chunks(jobs.len().div_ceil(threads)).zip(out.chunks_mut(jobs.len().div_ceil(threads))) {
            let (matcher, templates) = (&matcher, &templates);
            s.spawn(move || {
                for (&(f, i, g), o) in chunk_jobs.iter().zip(chunk_out) {
                    *o = matcher.score(&templates[g], &fingers[f].touches[i]);
                }
            });
        }
    });
    jobs.into_iter().zip(out).map(|((f, i, g), s)| (f, i, g, s)).collect()
}

fn score(dataset: &Path, enroll: usize) -> Result<()> {
    let fingers = load(dataset)?;
    let enrolled: Vec<Vec<usize>> = fingers.iter().map(|f| (0..enroll.min(f.touches.len())).collect()).collect();
    let probes: Vec<Vec<usize>> = fingers.iter().map(|f| (enroll..f.touches.len()).collect()).collect();
    println!("probe_finger,probe,template_finger,score");
    for (f, i, g, s) in all_scores(&fingers, &enrolled, &probes) {
        println!("{},{},{},{s:.6}", fingers[f].name, i + 1, fingers[g].name);
    }
    Ok(())
}

fn bench(dataset: &Path, enroll: usize) -> Result<()> {
    let fingers = load(dataset)?;
    let matcher = Matcher::default();
    let finger = &fingers[0];
    let start = Instant::now();
    let template = matcher.template(&finger.touches[..enroll]);
    let enroll_time = start.elapsed();
    let probes = &finger.touches[enroll..];
    let time = |matcher: &Matcher| {
        let start = Instant::now();
        for probe in probes {
            std::hint::black_box(matcher.score(&template, probe));
        }
        (start.elapsed() / probes.len() as u32).as_secs_f64() * 1e3
    };
    let single = Matcher::with_threads(1);
    println!(
        "enroll ({enroll} touches): {:.1} ms   verify: {:.1} ms on {} threads, {:.1} ms on one core",
        enroll_time.as_secs_f64() * 1e3,
        time(&matcher),
        matcher.threads,
        time(&single)
    );
    Ok(())
}

fn eval(dataset: &Path, enroll: usize, splits: usize, keep_partial: bool) -> Result<()> {
    let mut fingers = load(dataset)?;
    if !keep_partial {
        for f in &mut fingers {
            let before = f.touches.len();
            f.touches.retain(|t| !t.is_partial());
            if f.touches.len() < before {
                let n = before - f.touches.len();
                eprintln!("{}", trf("{finger}: {n} partial touch(es) left out", &[("finger", &f.name), ("n", &n)]));
            }
        }
    }
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    let (mut eers, mut accepted) = (Vec::new(), Vec::new());
    let mut per_finger = vec![Vec::new(); fingers.len()];
    let (mut all_genuine, mut all_impostor) = (Vec::new(), Vec::new());
    for split in 0..splits {
        let start = Instant::now();
        let orders: Vec<Vec<usize>> = fingers
            .iter()
            .map(|f| {
                let mut v: Vec<usize> = (0..f.touches.len()).collect();
                if split > 0 {
                    shuffle(&mut v, &mut rng);
                }
                v
            })
            .collect();
        let enrolled: Vec<Vec<usize>> = orders.iter().map(|o| o[..enroll.min(o.len())].to_vec()).collect();
        let probes: Vec<Vec<usize>> = orders.iter().map(|o| o[enroll.min(o.len())..].to_vec()).collect();
        let scores = all_scores(&fingers, &enrolled, &probes);
        let genuine: Vec<f64> = scores.iter().filter(|s| s.0 == s.2).map(|s| s.3).collect();
        let impostor: Vec<f64> = scores.iter().filter(|s| s.0 != s.2).map(|s| s.3).collect();
        let strict = impostor.iter().copied().fold(f64::MIN, f64::max);
        let eer = equal_error_rate(&genuine, &impostor);
        let acc = genuine.iter().filter(|&&g| g > strict).count() as f64 / genuine.len() as f64;
        for (f, per) in per_finger.iter_mut().enumerate() {
            let own: Vec<f64> = scores.iter().filter(|s| s.0 == f && s.2 == f).map(|s| s.3).collect();
            per.push(own.iter().filter(|&&g| g > strict).count() as f64 / own.len() as f64);
        }
        eprintln!(
            "split {split}: EER {:5.1}%  genuine accepted with 0 impostors {:5.1}%  strict threshold {strict:.3}  ({:.1}s)",
            eer * 100.0,
            acc * 100.0,
            start.elapsed().as_secs_f64()
        );
        eers.push(eer);
        accepted.push(acc);
        all_genuine.extend(genuine);
        all_impostor.extend(impostor);
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!("EER {:.1}%   accepted* {:.1}%", mean(&eers) * 100.0, mean(&accepted) * 100.0);
    for (f, per) in fingers.iter().zip(&per_finger) {
        println!("  {:14} {:5.0}%", f.name, mean(per) * 100.0);
    }
    println!("* {}", tr("genuine attempts accepted at the strictest threshold that rejects every impostor"));
    let rate = |v: &[f64], t: f64| v.iter().filter(|&&s| s >= t).count() as f64 / v.len() as f64;
    let worst = all_impostor.iter().copied().fold(f64::MIN, f64::max);
    println!(
        "\n{}",
        trf(
            "fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
            &[
                ("t", &format!("{THRESHOLD:.2}")),
                ("g", &format!("{:.1}", rate(&all_genuine, THRESHOLD) * 100.0)),
                ("i", &format!("{:.2}", rate(&all_impostor, THRESHOLD) * 100.0)),
                ("w", &format!("{worst:.3}")),
            ]
        )
    );
    for t in [0.82, 0.84, 0.86, 0.88, 0.90] {
        println!("  at {t:.2}: genuine {:5.1}%  impostors {:.2}%", rate(&all_genuine, t) * 100.0, rate(&all_impostor, t) * 100.0);
    }
    Ok(())
}

fn equal_error_rate(genuine: &[f64], impostor: &[f64]) -> f64 {
    let mut thresholds: Vec<f64> = genuine.iter().chain(impostor).copied().collect();
    thresholds.sort_by(f64::total_cmp);
    let rate = |v: &[f64], t: f64, below: bool| v.iter().filter(|&&s| if below { s < t } else { s >= t }).count() as f64 / v.len() as f64;
    let (mut eer, mut gap) = (1.0, 2.0);
    for t in thresholds {
        let (frr, far) = (rate(genuine, t, true), rate(impostor, t, false));
        if (frr - far).abs() < gap {
            (gap, eer) = ((frr - far).abs(), (frr + far) / 2.0);
        }
    }
    eer
}

/// Fisher-Yates with xorshift: reproducible splits without extra crates.
fn shuffle(v: &mut [usize], state: &mut u64) {
    for i in (1..v.len()).rev() {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        v.swap(i, (*state % (i as u64 + 1)) as usize);
    }
}
