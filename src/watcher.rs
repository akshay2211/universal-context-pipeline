use anyhow::Result;
use std::path::Path;

pub struct FolderWatcher;

impl FolderWatcher {
    pub async fn watch(_path: &Path) -> Result<()> {
        // TODO Week 3: notify crate, debounced channel, on event:
        //   - Create/Modify: re-chunk the file, hash each chunk, look up cache,
        //     embed misses, upsert into VectorStore.
        //   - Remove: delete chunks where file_path = removed path.
        todo!("notify-based folder watcher")
    }
}
