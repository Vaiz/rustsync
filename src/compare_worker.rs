use std::path::{Path, PathBuf};
use tokio::fs::DirEntry;
use crate::activity_tracker::ActivityTracker;
use crate::types::*;

pub(crate) struct CompareWorker {
    cmp_tx: CompareSender,
    cmp_rx: CompareReceiver,
    copy_tx: CopySender,
    dir_counter: ActivityTracker,
    recursive: bool,
}

impl CompareWorker {
    pub(crate) fn new(
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

    pub(crate) async fn worker_thread(&self) {
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

    async fn compare_and_copy_dir(&self, src_path: &Path, tgt_path: &Path) -> std::io::Result<()> {
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

    async fn queue_existing_dir(&self, src_folder: PathBuf, tgt_folder: PathBuf) {
        if self.recursive {
            self.dir_counter.push();
            self.cmp_tx.send((src_folder, tgt_folder))
                .await
                .expect("Failed to queue a directory");
        }
    }

    async fn queue_object_for_copy(&self, entry: DirEntry, tgt_folder: PathBuf) -> std::io::Result<()> {
        if self.recursive && entry.file_type().await?.is_dir() {
            self.dir_counter.push();
        }
        self.copy_tx.send((entry, tgt_folder))
            .await
            .expect("Failed to queue an object for copy");
        Ok(())
    }

    fn finish_folder(&self) {
        if self.dir_counter.pop() {
            dbg!("Closing channels ...");
            self.cmp_tx.close();
            self.copy_tx.close();
        }
    }
}
