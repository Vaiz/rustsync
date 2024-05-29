use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
struct Args {
    // https://download.samba.org/pub/rsync/rsync.1
    #[arg(long, short, help = "recurse into directories")]
    recursive: bool,
    #[arg(long, help = "create destination's missing path components")]
    mkpath: bool,
    #[arg(long = "dry-run", short = 'n', help = "perform a trial run with no changes made")]
    dry_run: bool,

    #[arg(required = true)]
    source: String,
    #[arg(required = true)]
    target: String,
}

fn mk_target_dir(p: &std::path::Path) {
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
        panic!("Cannot create target dir. Path: {}, Error: {e}", p.display());
    }
}
fn mkpath(p: &std::path::Path) {
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
        panic!("Cannot create target path. Path: {}, Error: {e}", p.display());
    }
}

fn main() {
    let args = Args::parse();
    let src_path = std::path::Path::new(&args.source);
    let tgt_path = std::path::Path::new(&args.target);

    println!("{args:?}");

    if !src_path.exists() {
        panic!("Source path doesn't exist. Path: {}", src_path.display());
    }

    if args.mkpath {
        mkpath(tgt_path);
    } else {
        mk_target_dir(tgt_path);
    }

    // TODO
}

