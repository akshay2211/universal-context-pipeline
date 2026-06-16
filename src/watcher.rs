use crate::embeddings::Embedder;
use crate::indexer::{self, is_supported, IndexOptions, IndexStats};
use crate::storage::VectorStore;
use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEBOUNCE_MS: u64 = 500;

/// Watch `root` recursively; re-index supported files as they change. Runs
/// until the underlying watcher is dropped or the channel closes.
pub async fn watch_folder<E: Embedder>(
    root: &Path,
    store: &mut VectorStore,
    embedder: &E,
    opts: &IndexOptions,
) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(ev) = res {
            let _ = tx.send(ev);
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    tracing::info!(path = %root.display(), "watching folder");

    while let Some(first) = rx.recv().await {
        let mut events = vec![first];
        let deadline = tokio::time::Instant::now() + Duration::from_millis(DEBOUNCE_MS);
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(ev)) => events.push(ev),
                _ => break,
            }
        }
        let part = partition_events(events);
        apply_changes(part, store, embedder, opts).await?;
    }
    Ok(())
}

#[derive(Default, Debug)]
pub struct PartitionedEvents {
    pub changed: HashSet<PathBuf>,
    pub removed: HashSet<PathBuf>,
}

pub fn partition_events(events: Vec<Event>) -> PartitionedEvents {
    let mut out = PartitionedEvents::default();
    for ev in events {
        match ev.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for p in ev.paths {
                    out.removed.remove(&p);
                    out.changed.insert(p);
                }
            }
            EventKind::Remove(_) => {
                for p in ev.paths {
                    out.changed.remove(&p);
                    out.removed.insert(p);
                }
            }
            _ => {}
        }
    }
    out
}

async fn apply_changes<E: Embedder>(
    part: PartitionedEvents,
    store: &mut VectorStore,
    embedder: &E,
    opts: &IndexOptions,
) -> Result<()> {
    for path in &part.removed {
        match store.delete_chunks_for_path(path) {
            Ok(n) if n > 0 => {
                tracing::info!(path = %path.display(), chunks = n, "removed");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "delete failed"),
        }
    }
    for path in &part.changed {
        if !path.is_file() || !is_supported(path) {
            continue;
        }
        let mut stats = IndexStats::default();
        match indexer::index_one_file(path, store, embedder, opts, &mut stats).await {
            Ok(()) => tracing::info!(path = %path.display(), chunks = stats.chunks_inserted, "re-indexed"),
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "re-index failed"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};

    fn ev(kind: EventKind, paths: Vec<&str>) -> Event {
        Event {
            kind,
            paths: paths.into_iter().map(PathBuf::from).collect(),
            attrs: Default::default(),
        }
    }

    #[test]
    fn create_and_modify_both_land_in_changed() {
        let events = vec![
            ev(EventKind::Create(CreateKind::File), vec!["/a.md"]),
            ev(EventKind::Modify(ModifyKind::Any), vec!["/b.md"]),
        ];
        let p = partition_events(events);
        assert!(p.changed.contains(Path::new("/a.md")));
        assert!(p.changed.contains(Path::new("/b.md")));
        assert!(p.removed.is_empty());
    }

    #[test]
    fn remove_lands_in_removed() {
        let events = vec![ev(EventKind::Remove(RemoveKind::File), vec!["/gone.md"])];
        let p = partition_events(events);
        assert!(p.removed.contains(Path::new("/gone.md")));
        assert!(p.changed.is_empty());
    }

    #[test]
    fn later_remove_supersedes_earlier_modify() {
        let events = vec![
            ev(EventKind::Modify(ModifyKind::Any), vec!["/x.md"]),
            ev(EventKind::Remove(RemoveKind::File), vec!["/x.md"]),
        ];
        let p = partition_events(events);
        assert!(p.removed.contains(Path::new("/x.md")));
        assert!(!p.changed.contains(Path::new("/x.md")));
    }

    #[test]
    fn later_create_supersedes_earlier_remove() {
        // editors often save as: remove, then create
        let events = vec![
            ev(EventKind::Remove(RemoveKind::File), vec!["/x.md"]),
            ev(EventKind::Create(CreateKind::File), vec!["/x.md"]),
        ];
        let p = partition_events(events);
        assert!(p.changed.contains(Path::new("/x.md")));
        assert!(!p.removed.contains(Path::new("/x.md")));
    }

    #[test]
    fn unrelated_kinds_are_ignored() {
        let events = vec![ev(EventKind::Access(notify::event::AccessKind::Read), vec!["/x.md"])];
        let p = partition_events(events);
        assert!(p.changed.is_empty());
        assert!(p.removed.is_empty());
    }

    #[test]
    fn duplicate_paths_are_deduplicated() {
        let events = vec![
            ev(EventKind::Modify(ModifyKind::Any), vec!["/x.md"]),
            ev(EventKind::Modify(ModifyKind::Any), vec!["/x.md"]),
            ev(EventKind::Modify(ModifyKind::Any), vec!["/x.md"]),
        ];
        let p = partition_events(events);
        assert_eq!(p.changed.len(), 1);
    }
}
