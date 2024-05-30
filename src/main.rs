use clap::Parser;

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
        panic!("Cannot create target dir. Path: {}, Error: {}", p.display(), e);
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
        panic!("Cannot create target path. Path: {}, Error: {}", p.display(), e);
    }
}

async fn copy_object(entry: tokio::fs::DirEntry, tgt_folder: std::path::PathBuf) -> std::io::Result<()> {
    let src_path = entry.path();
    let tgt_path = tgt_folder.join(entry.file_name());
    dbg!(format!("Copying {} to {}", src_path.display(), tgt_path.display()));
    if entry.file_type().await?.is_dir() {
        tokio::fs::create_dir(tgt_path).await?;
    } else {
        tokio::fs::copy(src_path, tgt_path).await?;
    }
    Ok(())
}

/*
async fn copy_dir(src_path: &std::path::Path, tgt_path: &std::path::Path) -> std::io::Result<()> {
    let mut src_dir = tokio::fs::read_dir(src_path).await?;
    while let Some(entry) = src_dir.next_entry().await? {
        copy_object(entry, tgt_path.to_path_buf()).await?;
    }
    Ok(())
}
*/
async fn compare_and_copy_dir(src_path: &std::path::Path, tgt_path: &std::path::Path) -> std::io::Result<()> {
    let src_dir = tokio::fs::read_dir(src_path);
    let tgt_dir = tokio::fs::read_dir(tgt_path);

    let mut src_dir = src_dir.await?;
    let mut tgt_dir = tgt_dir.await?;

    let mut tgt_dir_index = std::collections::HashMap::new();

    while let Some(entry) = tgt_dir.next_entry().await? {
        tgt_dir_index.insert(entry.file_name(), entry);
    }

    while let Some(entry) = src_dir.next_entry().await? {
        if let Some(tgt_entry) = tgt_dir_index.get(&entry.file_name()) {
            dbg!(format!("Object already exists. Path: {}", tgt_entry.path().display()));
        } else {
            copy_object(entry, tgt_path.to_path_buf()).await?;
        }
    }

    Ok(())
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

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = compare_and_copy_dir(src_path, tgt_path);
    rt.block_on(result).expect("Failed to sync two directories");
}

