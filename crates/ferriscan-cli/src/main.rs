//! Headless `FerriScan` scanner.
//!
//! Exists so the engine can be exercised and benchmarked before the UI is ready,
//! and so CI can run a real-filesystem smoke test. Deliberately dependency-free:
//! argument parsing is a dozen lines.

use std::path::PathBuf;
use std::process::ExitCode;

use ferriscan_core::model::{NodeId, SizeBasis};
use ferriscan_core::scan::controller::CancelToken;
use ferriscan_core::scan::progress::Progress;
use ferriscan_core::scan::{ScanOptions, scan};
use ferriscan_core::util::{ByteUnits, format_bytes};
use ferriscan_platform::PlatformDirReader;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return ExitCode::SUCCESS;
    }

    match args[0].as_str() {
        "scan" => run_scan(&args[1..]),
        "volumes" => {
            for volume in ferriscan_platform::list_volumes() {
                println!(
                    "{:<12} {:<10} {:<8} {:>10} / {:>10} free  {}",
                    volume.mount_point.display(),
                    volume.filesystem,
                    volume.kind.as_str(),
                    format_bytes(volume.available_bytes, ByteUnits::Binary),
                    format_bytes(volume.total_bytes, ByteUnits::Binary),
                    volume.label,
                );
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn run_scan(args: &[String]) -> ExitCode {
    let mut path: Option<PathBuf> = None;
    let mut top = 20usize;
    let mut include_hidden = true;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--top" => {
                index += 1;
                top = args.get(index).and_then(|v| v.parse().ok()).unwrap_or(20);
            }
            "--no-hidden" => include_hidden = false,
            flag if flag.starts_with('-') => {
                eprintln!("unknown option: {flag}");
                return ExitCode::from(2);
            }
            value => path = Some(PathBuf::from(value)),
        }
        index += 1;
    }

    let Some(path) = path else {
        eprintln!("scan requires a path");
        return ExitCode::from(2);
    };

    let reader = PlatformDirReader::new();
    let options = ScanOptions {
        include_hidden,
        ..ScanOptions::default()
    };
    let progress = Progress::new();
    let cancel = CancelToken::new();

    let started = std::time::Instant::now();
    let snapshot = match scan(&reader, &path, &options, &progress, &cancel) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("scan failed: {err}");
            return ExitCode::from(4);
        }
    };

    println!("root      {}", snapshot.meta.root_path.display());
    println!(
        "size      {} ({} logical)",
        format_bytes(snapshot.meta.allocated_bytes, ByteUnits::Binary),
        format_bytes(snapshot.meta.logical_bytes, ByteUnits::Binary),
    );
    println!(
        "entries   {} files, {} directories, {} errors",
        snapshot.meta.file_count, snapshot.meta.dir_count, snapshot.meta.error_count
    );
    println!(
        "duration  {:.2?}  ({:.0} entries/s)",
        started.elapsed(),
        snapshot.entry_count() as f64 / started.elapsed().as_secs_f64().max(0.001)
    );

    let mut children: Vec<(NodeId, u64)> = snapshot
        .store
        .children(snapshot.root)
        .map(|id| (id, snapshot.store.get(id).allocated))
        .collect();
    children.sort_by_key(|a| std::cmp::Reverse(a.1));

    println!("\ntop {top}:");
    for (id, size) in children.into_iter().take(top) {
        println!(
            "  {:>10}  {}{}",
            format_bytes(size, ByteUnits::Binary),
            snapshot.store.name(id),
            if snapshot.store.get(id).flags().is_dir() {
                "/"
            } else {
                ""
            }
        );
    }

    if snapshot.store.get(snapshot.root).size == 0 {
        return ExitCode::from(3);
    }
    let _ = SizeBasis::Allocated;
    ExitCode::SUCCESS
}

fn print_help() {
    println!(
        "ferriscan — headless disk scanner\n\n\
         Usage:\n\
         \x20 ferriscan scan <path> [--top N] [--no-hidden]\n\
         \x20 ferriscan volumes\n\n\
         Exit codes: 0 ok, 2 usage, 3 empty result, 4 failure."
    );
}
