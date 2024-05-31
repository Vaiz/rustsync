use std::path::PathBuf;
use tokio::fs::DirEntry;
use crate::types::{CompareSender, CopyReceiver};

pub(crate) struct CopyWorker {
    rx: CopyReceiver,
    tx: Option<CompareSender>,
}

impl CopyWorker {
    pub(crate) fn new(rx: CopyReceiver, tx: Option<CompareSender>) -> Self {
        Self {
            rx,
            tx,
        }
    }

    pub(crate) async fn worker_thread(&self) {
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
    async fn copy_object(&self, entry: &DirEntry, tgt_folder: PathBuf) -> std::io::Result<()> {
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

    async fn queue_dir(&self, src_folder: PathBuf, tgt_folder: PathBuf) {
        if let Some(tx) = &self.tx {
            tx.send((src_folder, tgt_folder))
                .await
                .expect("Failed to queue a directory");
        }
    }
}
