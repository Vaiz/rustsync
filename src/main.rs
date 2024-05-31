use clap::Parser;
use tokio::fs::{DirEntry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

type CompareFolder = (PathBuf, PathBuf);
type CompareSender = async_channel::Sender<CompareFolder>;
type CompareReceiver = async_channel::Receiver<CompareFolder>;

type CopyItem = (DirEntry, PathBuf);
type CopySender = async_channel::Sender<CopyItem>;
type CopyReceiver = async_channel::Receiver<CopyItem>;

#[derive(Debug, Copy, Clone)]
struct TokioSettings {
    compare_workers_count: usize,
    copy_queue_size: usize,
    copy_workers_count: usize,
}

impl TokioSettings {
    fn new(recursive: bool) -> Self {
        Self {
            compare_workers_count: if recursive { 16 } else { 1 },
            copy_queue_size: 1024,
            copy_workers_count: 16,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
struct Args {
    // https://download.samba.org/pub/rsync/rsync.1
    #[arg(long, short, help = "recurse into directories")]
    recursive: bool,
    #[arg(long, help = "create destination's missing path components")]
    mkpath: bool,
    /*
    #[arg(long = "dry-run", short = 'n', help = "perform a trial run with no changes made")]
    dry_run: bool,
    */

    #[arg(required = true)]
    source: String,
    #[arg(required = true)]
    target: String,
}


#[derive(Clone)]
struct ActivityTracker {
    processing_dirs: Arc<std::sync::atomic::AtomicI64>,
}

impl ActivityTracker {
    fn new(val: i64) -> Self {
        Self {
            processing_dirs: Arc::new(std::sync::atomic::AtomicI64::new(val))
        }
    }

    fn push(&mut self) {
        self.processing_dirs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn pop(&mut self) -> bool {
        let prev = self.processing_dirs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        prev == 1
    }
}

struct CopyWorker {
    rx: CopyReceiver,
    tx: CompareSender,
    recursive: bool,
}

impl CopyWorker {
    fn new(rx: CopyReceiver, tx: CompareSender, recursive: bool) -> Self {
        Self {
            rx,
            tx,
            recursive,
        }
    }

    async fn worker_thread(&mut self) {
        dbg!("Copy worker has started");
        loop {
            let next_msg = self.rx.recv().await;
            if let Err(_) = next_msg {
                //eprintln!("Failed to receive next copy message. Error: {}", e);
                break;
            }

            let (src_dir_entry, tgt_path) = next_msg.unwrap();
            if let Err(e) = self.copy_object(&src_dir_entry, tgt_path).await {
                eprintln!("Error copying object. Source path: {}, Error: {}", src_dir_entry.path().display(), e);
            }
        }
        dbg!("Copy worker has finished");
    }
    async fn copy_object(&mut self, entry: &DirEntry, tgt_folder: PathBuf) -> std::io::Result<()> {
        let src_path = entry.path();
        let tgt_path = tgt_folder.join(entry.file_name());
        dbg!(format!("Copying {} to {}", src_path.display(), tgt_path.display()));
        if entry.file_type().await?.is_dir() {
            tokio::fs::create_dir(tgt_path.clone()).await?;
            self.queue_dir(src_path, tgt_path).await;
        } else {
            tokio::fs::copy(src_path, tgt_path).await?;
        }
        Ok(())
    }

    async fn queue_dir(&mut self, src_folder: PathBuf, tgt_folder: PathBuf) {
        if self.recursive {
            self.tx.send((src_folder, tgt_folder))
                .await
                .expect("Failed to queue a directory");
        }
    }
}

struct CompareWorker {
    cmp_tx: CompareSender,
    cmp_rx: CompareReceiver,
    copy_tx: CopySender,
    dir_counter: ActivityTracker,
    recursive: bool,
}

impl CompareWorker {
    fn new(
        cmp_tx: CompareSender,
        cmp_rx: CompareReceiver,
        copy_tx: CopySender,
        dir_counter: ActivityTracker,
        recursive: bool) -> Self {
        Self {
            cmp_tx,
            cmp_rx,
            copy_tx,
            dir_counter,
            recursive,
        }
    }

    async fn worker_thread(&mut self) {
        dbg!("Compare worker has started");
        loop {
            let next_msg = self.cmp_rx.recv().await;
            if let Err(_) = next_msg {
                //eprintln!("Failed to receive next compare message. Error: {}", e);
                break;
            }

            let (src_folder, tgt_folder) = next_msg.unwrap();
            if let Err(e) = self.compare_and_copy_dir(src_folder.as_path(), tgt_folder.as_path()).await {
                eprintln!("Error comparing folders. Source folder: {}, Target folder: {}, Error: {}",
                          src_folder.display(), tgt_folder.display(), e);
            }
        }
        dbg!("Compare worker has finished");
    }

    async fn compare_and_copy_dir(&mut self, src_path: &Path, tgt_path: &Path) -> std::io::Result<()> {
        let src_dir = tokio::fs::read_dir(src_path);
        let tgt_dir = tokio::fs::read_dir(tgt_path);

        let mut src_dir = src_dir.await
            .expect(format!("Failed to list source folder. Path: {}", src_path.display()).as_str());
        let mut tgt_dir = tgt_dir.await
            .expect(format!("Failed to list target folder. Path: {}", tgt_path.display()).as_str());

        let mut tgt_dir_index = std::collections::HashMap::new();

        while let Some(entry) = tgt_dir.next_entry().await? {
            tgt_dir_index.insert(entry.file_name(), entry);
        }

        while let Some(entry) = src_dir.next_entry().await? {
            if let Some(tgt_entry) = tgt_dir_index.get(&entry.file_name()) {
                dbg!(format!("Object already exists. Path: {}", tgt_entry.path().display()));
                if entry.file_type().await?.is_dir() {
                    if tgt_entry.file_type().await?.is_dir() {
                        self.queue_existing_dir(entry.path(), tgt_entry.path()).await;
                    } else {
                        eprintln!("Cannot copy directory because there is a file with the same name.\
                        Directory: {}, File: {}", entry.path().display(), tgt_entry.path().display());
                    }
                }
            } else {
                self.queue_object_for_copy(entry, tgt_path.to_path_buf()).await?;
            }
        }

        self.finish_folder();
        Ok(())
    }

    async fn queue_existing_dir(&mut self, src_folder: PathBuf, tgt_folder: PathBuf) {
        if self.recursive {
            self.dir_counter.push();
            self.cmp_tx.send((src_folder, tgt_folder))
                .await
                .expect("Failed to queue a directory");
        }
    }

    async fn queue_object_for_copy(&mut self, entry: DirEntry, tgt_folder: PathBuf) -> std::io::Result<()> {
        if self.recursive && entry.file_type().await?.is_dir() {
            self.dir_counter.push();
        }
        self.copy_tx.send((entry, tgt_folder))
            .await
            .expect("Failed to queue an object for copy");
        Ok(())
    }

    fn finish_folder(&mut self) {
        if self.dir_counter.pop() {
            dbg!("Closing channels ...");
            self.cmp_tx.close();
            self.copy_tx.close();
        }
    }
}

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
        let mut worker = CompareWorker::new(
            cmp_tx.clone(), cmp_rx.clone(), copy_tx.clone(), dir_counter.clone(), args.recursive);
        handles.push(tokio::spawn(async move {
            worker.worker_thread().await;
        }));
    }
    for _ in 0..tokio_settings.copy_workers_count {
        let mut worker = CopyWorker::new(copy_rx.clone(), cmp_tx.clone(), args.recursive);
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

