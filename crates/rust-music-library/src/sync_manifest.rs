//! Canonical v1 binary serialization for offline sync snapshot metadata.
//! No filesystem I/O or cryptographic verification happens here.
use super::sync_plan::{
    validate_snapshot, MediaEntry, PlanError, Snapshot, MAX_SYNC_PATH_BYTES, MAX_SYNC_TRACKS,
};

const MAGIC: &[u8; 8] = b"RMSYNC01";
const HEADER_LEN: usize = 8 + 8 + 4;
const ENTRY_FIXED: usize = 16 + 2 + 8 + 32;
pub const MAX_MANIFEST_BYTES: usize =
    HEADER_LEN + MAX_SYNC_TRACKS * (ENTRY_FIXED + MAX_SYNC_PATH_BYTES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    InvalidHeader,
    Truncated,
    TrailingData,
    NonCanonical,
    InvalidUtf8,
    InvalidSnapshot(PlanError),
    TooLarge,
}
impl From<PlanError> for ManifestError {
    fn from(error: PlanError) -> Self {
        Self::InvalidSnapshot(error)
    }
}

/// V1 is canonical: entries sorted by ID; LE lengths; no trailing bytes.
pub fn encode(snapshot: &Snapshot) -> Result<Vec<u8>, ManifestError> {
    validate_snapshot(snapshot)?;
    let mut entries = snapshot.entries.clone();
    entries.sort_by_key(|entry| entry.id);
    let capacity = HEADER_LEN
        + entries
            .iter()
            .map(|e| ENTRY_FIXED + e.relative_path.len())
            .sum::<usize>();
    if capacity > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&snapshot.generation.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&entry.id);
        out.extend_from_slice(&(entry.relative_path.len() as u16).to_le_bytes());
        out.extend_from_slice(entry.relative_path.as_bytes());
        out.extend_from_slice(&entry.size.to_le_bytes());
        out.extend_from_slice(&entry.sha256);
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Snapshot, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    if bytes.len() < HEADER_LEN {
        return Err(ManifestError::Truncated);
    }
    if &bytes[..8] != MAGIC {
        return Err(ManifestError::InvalidHeader);
    }
    let generation = u64::from_le_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| ManifestError::Truncated)?,
    );
    let count = u32::from_le_bytes(
        bytes[16..20]
            .try_into()
            .map_err(|_| ManifestError::Truncated)?,
    ) as usize;
    if count > MAX_SYNC_TRACKS {
        return Err(ManifestError::TooLarge);
    }
    let mut position = HEADER_LEN;
    let mut entries = Vec::with_capacity(count);
    let mut previous_id: Option<[u8; 16]> = None;
    for _ in 0..count {
        let fixed_start = position.checked_add(18).ok_or(ManifestError::TooLarge)?;
        if fixed_start > bytes.len() {
            return Err(ManifestError::Truncated);
        }
        let id: [u8; 16] = bytes[position..position + 16]
            .try_into()
            .map_err(|_| ManifestError::Truncated)?;
        if previous_id.is_some_and(|prev| id <= prev) {
            return Err(ManifestError::NonCanonical);
        }
        previous_id = Some(id);
        position += 16;
        let path_len = u16::from_le_bytes(
            bytes[position..position + 2]
                .try_into()
                .map_err(|_| ManifestError::Truncated)?,
        ) as usize;
        position += 2;
        if path_len == 0 || path_len > MAX_SYNC_PATH_BYTES {
            return Err(ManifestError::InvalidSnapshot(PlanError::InvalidPath));
        }
        let end = position
            .checked_add(path_len)
            .and_then(|p| p.checked_add(40))
            .ok_or(ManifestError::TooLarge)?;
        if end > bytes.len() {
            return Err(ManifestError::Truncated);
        }
        let relative_path = std::str::from_utf8(&bytes[position..position + path_len])
            .map_err(|_| ManifestError::InvalidUtf8)?
            .to_owned();
        position += path_len;
        let size = u64::from_le_bytes(
            bytes[position..position + 8]
                .try_into()
                .map_err(|_| ManifestError::Truncated)?,
        );
        position += 8;
        let sha256 = bytes[position..position + 32]
            .try_into()
            .map_err(|_| ManifestError::Truncated)?;
        position += 32;
        entries.push(MediaEntry {
            id,
            relative_path,
            size,
            sha256,
        });
    }
    if position != bytes.len() {
        return Err(ManifestError::TrailingData);
    }
    let snapshot = Snapshot {
        generation,
        entries,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: u8, name: &str) -> MediaEntry {
        MediaEntry {
            id: [id; 16],
            relative_path: name.into(),
            size: 90,
            sha256: [id; 32],
        }
    }
    #[test]
    fn canonical_roundtrip_and_empty_golden() {
        let empty = Snapshot {
            generation: 1,
            entries: vec![],
        };
        assert_eq!(
            encode(&empty).unwrap(),
            b"RMSYNC01\x01\0\0\0\0\0\0\0\0\0\0\0"
        );
        assert_eq!(decode(&encode(&empty).unwrap()).unwrap(), empty);
        let a = Snapshot {
            generation: 4,
            entries: vec![entry(2, "b.flac"), entry(1, "a.flac")],
        };
        let b = Snapshot {
            generation: 4,
            entries: vec![entry(1, "a.flac"), entry(2, "b.flac")],
        };
        assert_eq!(encode(&a).unwrap(), encode(&b).unwrap());
        assert_eq!(decode(&encode(&a).unwrap()).unwrap(), b);
    }
    #[test]
    fn rejects_bad_header_truncation_trailing_and_order() {
        let bytes = encode(&Snapshot {
            generation: 2,
            entries: vec![entry(1, "a.flac"), entry(2, "b.flac")],
        })
        .unwrap();
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert_eq!(decode(&bad), Err(ManifestError::InvalidHeader));
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n]).is_err());
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert_eq!(decode(&bad), Err(ManifestError::TrailingData));
        let mut bad = bytes.clone();
        bad[16..20].copy_from_slice(&(20_001u32).to_le_bytes());
        assert_eq!(decode(&bad), Err(ManifestError::TooLarge));
    }
    #[test]
    fn rejects_duplicate_and_unsafe_paths() {
        let duplicate = Snapshot {
            generation: 1,
            entries: vec![entry(1, "A.flac"), entry(2, "a.flac")],
        };
        assert_eq!(
            encode(&duplicate),
            Err(ManifestError::InvalidSnapshot(PlanError::DuplicatePath))
        );
        let traversal = Snapshot {
            generation: 1,
            entries: vec![entry(1, "../a.flac")],
        };
        assert_eq!(
            encode(&traversal),
            Err(ManifestError::InvalidSnapshot(PlanError::InvalidPath))
        );
    }
}
