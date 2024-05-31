use clap::Parser;

#[derive(Debug, Copy, Clone)]
pub(crate) struct TokioSettings {
    pub(crate) compare_workers_count: usize,
    pub(crate) copy_queue_size: usize,
    pub(crate) copy_workers_count: usize,
}

impl TokioSettings {
    pub(crate) fn new(recursive: bool) -> Self {
        Self {
            compare_workers_count: if recursive { 16 } else { 1 },
            copy_queue_size: 1024,
            copy_workers_count: 16,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
pub(crate) struct Args {
    // https://download.samba.org/pub/rsync/rsync.1
    #[arg(long, short, help = "recurse into directories")]
    pub(crate) recursive: bool,
    #[arg(long, help = "create destination's missing path components")]
    pub(crate) mkpath: bool,
    /*
    #[arg(long = "dry-run", short = 'n', help = "perform a trial run with no changes made")]
    pub(crate) dry_run: bool,
    */

    #[arg(required = true)]
    pub(crate) source: String,
    #[arg(required = true)]
    pub(crate) target: String,
}
