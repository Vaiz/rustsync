use std::path::PathBuf;
use tokio::fs::DirEntry;

pub(crate) type CompareFolderItem = (PathBuf, PathBuf);
pub(crate) type CompareSender = async_channel::Sender<CompareFolderItem>;
pub(crate) type CompareReceiver = async_channel::Receiver<CompareFolderItem>;

pub(crate) type CopyItem = (DirEntry, PathBuf);
pub(crate) type CopySender = async_channel::Sender<CopyItem>;
pub(crate) type CopyReceiver = async_channel::Receiver<CopyItem>;