use std::collections::{HashMap, BTreeSet};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::fmt::{Display, Result, Formatter};

/// Wrapper for PathBuf holding its mtime as u64
#[derive(Debug, Serialize, Deserialize)]
pub struct PathBufx {
    pub file_path: PathBuf, 
    pub snapshot_path: PathBuf, // root path (no extension)
    pub mtime: u64,
}

impl PathBufx {
    pub fn new() -> Self {
        PathBufx {
            file_path: PathBuf::new(),
            snapshot_path: PathBuf::new(),
            mtime: u64::MIN,
        }
    }

    pub fn from(file_path: PathBuf, snapshot_path: PathBuf, mtime: u64) -> Self {
        PathBufx {
            file_path,
            snapshot_path,
            mtime,
        }
    }
}

/// Containg two pairing (equal) paths
/// the local path (destination) and it's equivelent remote path (source)
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct PathPair {
    pub source: PathBuf,
    pub destination: PathBuf,
}

impl PathPair {
    pub fn from(source: PathBuf, destination: PathBuf) -> Self {
        PathPair {
            source,
            destination
        }
    }
}

// Implementing PartialOrd and Ord for PathPair
impl Ord for PathPair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.source.cmp(&other.source)
    }
}

impl PartialOrd for PathPair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Entries containing the mtime of files.
/// Using the source path as key, we can get data.
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub entries: HashMap<PathBuf, PathBufx>,
    pub deleted_entries: BTreeSet<PathPair>,
}

impl Display for Snapshot {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "Snapshot: {{\n\tentries: {:?},\n\tdeleted_entries: {:?}\n\t\n}}", self.entries, self.deleted_entries)
    }
}

impl Snapshot {
    pub fn new(cap: usize) -> Self {
        Snapshot {
            entries: HashMap::with_capacity(cap),
            deleted_entries: BTreeSet::new(),
        }
    }

    pub fn add_entry(&mut self, pathpair: PathPair, snapshot_path: PathBuf, mtime: u64) {
        self.entries.insert(
            pathpair.source.clone(), 
            PathBufx::from(pathpair.destination.clone(), snapshot_path, mtime)
        );
        self.deleted_entries.remove(&pathpair);
    }

    pub fn mark_as_deleted(&mut self, pair: PathPair) {
        self.entries.remove(&pair.source);
        self.deleted_entries.insert(pair);
    }

    pub fn is_deleted(&self, pair: &PathPair) -> bool {
        self.deleted_entries.contains(pair)
    }

    pub fn undelete(&mut self, pair: &PathPair) {
        self.deleted_entries.remove(pair);
    }

    /// returns the mtime entry matching key
    pub fn mtime(&self, key: &PathBuf) -> Option<&u64> {
        self.entries.get(key).map(|entry| &entry.mtime)
    }

    pub fn path(&self, key: &PathBuf) -> Option<&PathBuf> {
        self.entries.get(key).map(|entry| &entry.file_path)
    }
}
