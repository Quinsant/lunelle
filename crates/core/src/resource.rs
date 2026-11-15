use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::{Runtime, Task};

pub type ResourceKey = PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Image,
    Css,
    Font,
    Static,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub path: ResourceKey,
    pub kind: ResourceKind,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceTable {
    resources: Vec<Resource>,
}

impl ResourceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(
        &mut self,
        path: impl Into<PathBuf>,
        kind: ResourceKind,
        bytes: impl Into<Arc<[u8]>>,
    ) {
        self.resources.push(Resource {
            path: path.into(),
            kind,
            bytes: bytes.into(),
        });
    }

    /// Loads resources at runtime, typically from `main`.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, crate::ResourceError> {
        Self::load_directory(root)
    }

    /// Loads resources from a build script before passing the resulting table on.
    pub fn load_for_build(root: impl AsRef<Path>) -> Result<Self, crate::ResourceError> {
        Self::load_directory(root)
    }

    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Resource> {
        self.resources
            .iter()
            .find(|resource| resource.path == path.as_ref())
    }

    fn load_directory(root: impl AsRef<Path>) -> Result<Self, crate::ResourceError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(crate::ResourceError::InvalidPath);
        }
        let mut resources = Vec::new();
        collect_resources(root, root, &mut resources)?;
        resources.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(Self { resources })
    }
}

fn collect_resources(
    root: &Path,
    directory: &Path,
    resources: &mut Vec<Resource>,
) -> Result<(), crate::ResourceError> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_resources(root, &path, resources)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| crate::ResourceError::InvalidPath)?
            .to_path_buf();
        let bytes = std::fs::read(&path).map_err(|source| crate::ResourceError::IoAt {
            path: relative.clone(),
            source,
        })?;
        resources.push(Resource {
            path: relative.clone(),
            kind: ResourceKind::from_path(&relative),
            bytes: Arc::from(bytes),
        });
    }
    Ok(())
}

impl ResourceKind {
    fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("css") => Self::Css,
            Some("ttf" | "otf" | "woff" | "woff2") => Self::Font,
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg") => Self::Image,
            _ => Self::Static,
        }
    }
}

/// Canonical filesystem root for resources, with traversal and symlink escape protection.
#[derive(Clone, Debug)]
pub struct ResourceRoot {
    path: Arc<Path>,
}

impl ResourceRoot {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, crate::ResourceError> {
        let path = std::fs::canonicalize(path)?;
        if !path.is_dir() {
            return Err(crate::ResourceError::InvalidPath);
        }
        Ok(Self {
            path: Arc::from(path),
        })
    }

    pub fn validate_relative_path(path: &Path) -> Result<PathBuf, crate::ResourceError> {
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(crate::ResourceError::InvalidPath);
        }
        Ok(path.to_path_buf())
    }

    pub fn resolve(&self, relative_path: &Path) -> Result<PathBuf, crate::ResourceError> {
        let relative_path = Self::validate_relative_path(relative_path)?;
        let path = std::fs::canonicalize(self.path.join(&relative_path)).map_err(|source| {
            crate::ResourceError::IoAt {
                path: relative_path.clone(),
                source,
            }
        })?;
        if !path.starts_with(self.path.as_ref()) {
            return Err(crate::ResourceError::OutsideResourceRoot(relative_path));
        }
        Ok(path)
    }
}

/// Size-bounded least-recently-used cache for resource values.
///
/// The cache itself is single-owner; callers that share it across threads must provide
/// synchronization appropriate to their workload.
pub struct ResourceCache<K, V> {
    entries: HashMap<K, V>,
    least_recently_used: VecDeque<K>,
    bytes: usize,
    limit: usize,
    size_of: fn(&V) -> usize,
}

impl<K, V> ResourceCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub fn new(limit: usize, size_of: fn(&V) -> usize) -> Self {
        Self {
            entries: HashMap::new(),
            least_recently_used: VecDeque::new(),
            bytes: 0,
            limit,
            size_of,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let value = self.entries.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    pub fn insert(&mut self, key: K, value: V) -> V {
        if let Some(existing) = self.entries.get(&key).cloned() {
            self.touch(&key);
            return existing;
        }

        let value_size = (self.size_of)(&value);
        if value_size > self.limit {
            return value;
        }

        while self.bytes.saturating_add(value_size) > self.limit {
            let Some(oldest_key) = self.least_recently_used.pop_front() else {
                break;
            };
            if let Some(oldest) = self.entries.remove(&oldest_key) {
                self.bytes = self.bytes.saturating_sub((self.size_of)(&oldest));
            }
        }
        self.bytes = self.bytes.saturating_add(value_size);
        self.least_recently_used.push_back(key.clone());
        self.entries.insert(key, value.clone());
        value
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        while self.bytes > self.limit {
            let Some(oldest_key) = self.least_recently_used.pop_front() else {
                break;
            };
            if let Some(oldest) = self.entries.remove(&oldest_key) {
                self.bytes = self.bytes.saturating_sub((self.size_of)(&oldest));
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.least_recently_used.clear();
        self.bytes = 0;
    }

    fn touch(&mut self, key: &K) {
        self.least_recently_used.retain(|entry| entry != key);
        self.least_recently_used.push_back(key.clone());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

/// Backend-independent asynchronous resource loading contract.
///
/// Implementations are shareable across threads. `load` takes ownership of its key,
/// borrows the runtime only for scheduling, and returns an owned task handle. It should
/// enqueue potentially blocking work rather than block the caller, which may be the UI
/// thread.
pub trait ResourceLoader<T>: Send + Sync {
    type Error;

    /// Starts loading a resource without transferring ownership of the runtime.
    fn load(&self, runtime: &Runtime, key: ResourceKey) -> Result<Task<T>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{ResourceCache, ResourceKind};

    #[test]
    fn classifies_resource_extensions() {
        assert_eq!(
            ResourceKind::from_path(std::path::Path::new("site.css")),
            ResourceKind::Css
        );
        assert_eq!(
            ResourceKind::from_path(std::path::Path::new("font.woff2")),
            ResourceKind::Font
        );
        assert_eq!(
            ResourceKind::from_path(std::path::Path::new("logo.png")),
            ResourceKind::Image
        );
    }

    #[test]
    fn evicts_least_recently_used_entry() {
        let mut cache = ResourceCache::new(4, String::len);
        cache.insert("a", "aa".to_owned());
        cache.insert("b", "bb".to_owned());

        assert_eq!(cache.get(&"a"), Some("aa".to_owned()));
        cache.insert("c", "cc".to_owned());

        assert_eq!(cache.get(&"a"), Some("aa".to_owned()));
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some("cc".to_owned()));
        assert_eq!(cache.bytes(), 4);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn does_not_cache_oversized_values() {
        let mut cache = ResourceCache::new(4, String::len);

        assert_eq!(cache.insert("large", "oversized".to_owned()), "oversized");
        assert!(cache.is_empty());
        assert_eq!(cache.bytes(), 0);
    }
}
