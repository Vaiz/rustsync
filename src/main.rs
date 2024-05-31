use clap::Parser;
use std::path::Path;

mod types;
mod settings;
mod activity_tracker;
mod copy_worker;
mod compare_worker;

use settings::*;
use activity_tracker::ActivityTracker;

fn mk_target_dir(p: &Path) {
    if p.exists() {
        if p.is_file() {
            panic!("Cannot create target dir. Part of the path is a file. File path: {}", p.display());
        }
    }
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            panic!("Cannot create target dir. Part of the path doesn't exist. Path: {}", parent.display());
        }
        if parent.is_file() {
            panic!("Cannot create target dir. Part of the path is a file. File path: {}", parent.display());
        }
    }

    if let Err(e) = std::fs::create_dir(p) {
        panic!("Cannot create target dir. Path: {}, Error: {}", p.display(), e);
    }
}

fn mkpath(p: &Path) {
    if p.exists() {
        if p.is_file() {
            panic!("Cannot create target path. Part of the path is a file. File path: {}", p.display());
        }
        return;
    }
    if let Some(parent) = p.parent() {
        mkpath(parent);
    }
    if let Err(e) = std::fs::create_dir(p) {
        panic!("Cannot create target path. Path: {}, Error: {}", p.display(), e);
    }
}


async fn start(args: &Args, tokio_settings: &TokioSettings) -> std::io::Result<()> {
    let src_path = Path::new(&args.source);
    let tgt_path = Path::new(&args.target);

    let mut handles = Vec::new();
    let (cmp_tx, cmp_rx) = async_channel::unbounded();
    let (copy_tx, copy_rx) = async_channel::bounded(tokio_settings.copy_queue_size);
    let dir_counter = ActivityTracker::new(1);
    for _ in 0..tokio_settings.compare_workers_count {
        let worker = compare_worker::CompareWorker::new(
            cmp_tx.clone(), cmp_rx.clone(), copy_tx.clone(), dir_counter.clone(), args.recursive);
        handles.push(tokio::spawn(async move {
            worker.worker_thread().await;
        }));
    }
    for _ in 0..tokio_settings.copy_workers_count {
        let cmp_tx = if args.recursive { Some(cmp_tx.clone()) } else { None };
        let worker = copy_worker::CopyWorker::new(copy_rx.clone(), cmp_tx);
        handles.push(tokio::spawn(async move {
            worker.worker_thread().await;
        }));
    }

    cmp_tx.send((src_path.to_path_buf(), tgt_path.to_path_buf()))
        .await
        .expect("Failed to schedule the first folder");

    dbg!("Initialization has been completed");

    for h in handles {
        h.await?;
    }

    Ok(())
}

fn main() {
    let args = Args::parse();
    let tokio_settings = TokioSettings::new(args.recursive);
    let src_path = Path::new(&args.source);
    let tgt_path = Path::new(&args.target);

    println!("{args:?}");

    if !src_path.exists() {
        panic!("Source path doesn't exist. Path: {}", src_path.display());
    }

    if args.mkpath {
        mkpath(tgt_path);
    } else {
        mk_target_dir(tgt_path);
    }

    let future = start(&args, &tokio_settings);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(future).expect("Failed to complete folders synchronisation.\n");
}

