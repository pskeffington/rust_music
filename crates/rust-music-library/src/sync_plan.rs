//! Pure, bounded desktop-to-Pip-Boy synchronization planning.
//! This module performs NO filesystem writes, scans, hashing or playback.
//! Callers must independently calculate and verify SHA-256 before publication.

use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SYNC_TRACKS: usize = 20_000;
pub const MAX_SYNC_PATH_BYTES: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaEntry {
    /// Portable logical entry identity, independent of the desktop root path.
    pub id: [u8; 16],
    /// Relative UTF-8 media path inside the managed target directory.
    pub relative_path: String,
    pub size: u64,
    /// Caller-provided SHA-256. This planner does not authenticate its value.
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: u64,
    pub entries: Vec<MediaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    pub from_generation: Option<u64>,
    pub to_generation: u64,
    /// Files to copy and verify before publishing a new manifest.
    pub transfer: Vec<MediaEntry>,
    /// Only previously managed files; recommendations, NOT delete operations.
    pub stale_managed_paths: Vec<String>,
    pub unchanged: usize,
    pub transfer_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    TooManyTracks,
    InvalidPath,
    DuplicateId,
    DuplicatePath,
    GenerationRollback,
    SizeOverflow,
}

fn valid_path(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_SYNC_PATH_BYTES || value.contains('\\')
        || value.chars().any(|ch| ch.is_control() || ch == ':')
        || value.starts_with('/')
    {
        return false;
    }
    value.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && !component.ends_with([' ', '.'])
            && !matches!(component.to_ascii_lowercase().as_str(),
                "con" | "prn" | "aux" | "nul"
                | "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9"
                | "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9")
    })
}

/// Validate the structural contract, not whether bytes on disk match their digest.
pub fn validate_snapshot(snapshot: &Snapshot) -> Result<(), PlanError> {
    if snapshot.entries.len() > MAX_SYNC_TRACKS {
        return Err(PlanError::TooManyTracks);
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in &snapshot.entries {
        if !valid_path(&entry.relative_path) {
            return Err(PlanError::InvalidPath);
        }
        if !ids.insert(entry.id) {
            return Err(PlanError::DuplicateId);
        }
        // Conservative ASCII case-folding prevents common FAT/exFAT conflicts.
        // Full Unicode normalization/case-folding is a separate import gate.
        if !paths.insert(entry.relative_path.to_ascii_lowercase()) {
            return Err(PlanError::DuplicatePath);
        }
    }
    Ok(())
}

/// Compute a deterministic delta without mutating either catalog or the target.
/// File transfer must use a separate safe-open, digest-checked atomic publisher.
pub fn plan_sync(
    target: Option<&Snapshot>,
    desired: &Snapshot,
) -> Result<SyncPlan, PlanError> {
    validate_snapshot(desired)?;
    if let Some(old) = target {
        validate_snapshot(old)?;
        if desired.generation <= old.generation {
            return Err(PlanError::GenerationRollback);
        }
    }
    let old_by_id: BTreeMap<_, _> = target
        .into_iter()
        .flat_map(|snapshot| &snapshot.entries)
        .map(|entry| (entry.id, entry))
        .collect();
    let desired_paths: BTreeSet<_> = desired
        .entries
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect();
    let mut transfer = Vec::new();
    let mut unchanged = 0;
    let mut transfer_bytes = 0u64;
    for entry in &desired.entries {
        if old_by_id.get(&entry.id).is_some_and(|old| {
            old.relative_path == entry.relative_path
                && old.size == entry.size
                && old.sha256 == entry.sha256
        }) {
            unchanged += 1;
        } else {
            transfer_bytes = transfer_bytes
                .checked_add(entry.size)
                .ok_or(PlanError::SizeOverflow)?;
            transfer.push(entry.clone());
        }
    }
    transfer.sort_by(|a, b| a.relative_path.cmp(&b.relative_path).then(a.id.cmp(&b.id)));
    let mut stale_managed_paths: Vec<_> = old_by_id
        .values()
        .filter(|old| {
            !desired.entries.iter().any(|new| {
                new.id == old.id && new.relative_path == old.relative_path
            }) && !desired_paths.contains(old.relative_path.as_str())
        })
        .map(|old| old.relative_path.clone())
        .collect();
    stale_managed_paths.sort();
    Ok(SyncPlan {
        from_generation: target.map(|snapshot| snapshot.generation),
        to_generation: desired.generation,
        transfer,
        stale_managed_paths,
        unchanged,
        transfer_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u8, name: &str, size: u64, digest: u8) -> MediaEntry {
        MediaEntry { id: [id; 16], relative_path: name.into(), size, sha256: [digest; 32] }
    }
    fn snapshot(generation: u64, entries: Vec<MediaEntry>) -> Snapshot {
        Snapshot { generation, entries }
    }
    #[test]
    fn empty_and_incremental_plans_are_deterministic() {
        let old = snapshot(1, vec![entry(1, "a.flac", 100, 9), entry(2, "b.flac", 200, 8)]);
        let new = snapshot(2, vec![entry(2, "b.flac", 200, 8), entry(3, "c.flac", 30, 7)]);
        let plan = plan_sync(Some(&old), &new).unwrap();
        assert_eq!(plan.unchanged, 1);
        assert_eq!(plan.transfer_bytes, 30);
        assert_eq!(plan.transfer, vec![entry(3, "c.flac", 30, 7)]);
        assert_eq!(plan.stale_managed_paths, vec!["a.flac"]);
        assert_eq!(plan, plan_sync(Some(&old), &new).unwrap());
        assert_eq!(plan_sync(None, &snapshot(1, vec![])).unwrap().transfer_bytes, 0);
    }
    #[test]
    fn mutation_or_rename_requires_copy() {
        let old = snapshot(1, vec![entry(1, "a.flac", 100, 1)]);
        let new = snapshot(2, vec![entry(1, "renamed.flac", 100, 1)]);
        let plan = plan_sync(Some(&old), &new).unwrap();
        assert_eq!(plan.transfer.len(), 1);
        assert_eq!(plan.stale_managed_paths, vec!["a.flac"]);
    }
    #[test]
    fn reject_traversal_collisions_and_rollback() {
        for path in ["../bad.flac", "/root.flac", "a//b.flac", "a\\b.flac", "CON", "x.", "a/./b", "C:/bad"] {
            assert_eq!(validate_snapshot(&snapshot(1, vec![entry(1, path, 1, 1)])), Err(PlanError::InvalidPath));
        }
        assert_eq!(validate_snapshot(&snapshot(1, vec![
            entry(1, "A.flac", 1, 1), entry(2, "a.flac", 1, 2)
        ])), Err(PlanError::DuplicatePath));
        assert_eq!(validate_snapshot(&snapshot(1, vec![
            entry(1, "a.flac", 1, 1), entry(1, "b.flac", 1, 2)
        ])), Err(PlanError::DuplicateId));
        assert_eq!(plan_sync(Some(&snapshot(3, vec![])), &snapshot(2, vec![])), Err(PlanError::GenerationRollback));
    }
    #[test]
    fn overflow_is_rejected() {
        let desired = snapshot(2, vec![entry(1, "a.flac", u64::MAX, 1), entry(2, "b.flac", 1, 1)]);
        assert_eq!(plan_sync(None, &desired), Err(PlanError::SizeOverflow));
    }
}
