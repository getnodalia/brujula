//! The library: what the browser has left on the disk that the user may want again, in one
//! place next to the history. Downloads and screenshots are the files themselves (read from their
//! folders, so they are there after a restart too); this module only picks and orders them.

use std::path::{Path, PathBuf};

/// How many directory entries are looked at, so a folder with a million files cannot stall the
/// window.
const MAX_SCANNED: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibFile {
    pub name: String,
    pub path: PathBuf,
    /// Last change, in milliseconds since 1970 (0 when unknown).
    pub modified: u64,
}

/// A finished download: not hidden, and not the half-written copy Brújula keeps while it scans
/// the file (`.name.brujula-part`).
pub fn is_download(name: &str) -> bool {
    !name.starts_with('.') && !name.ends_with(".brujula-part") && !name.ends_with(".part")
}

/// A screenshot taken with Ctrl+Mayús+S: `Brujula-<date>-<time>.png`.
pub fn is_capture(name: &str) -> bool {
    name.starts_with("Brujula-") && name.ends_with(".png")
}

/// The newest regular files of `dir` whose name `keep` accepts, newest first. Symbolic links,
/// folders and anything unreadable are skipped.
pub fn recent_files(dir: &Path, keep: impl Fn(&str) -> bool, limit: usize) -> Vec<LibFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<LibFile> = entries
        .flatten()
        .take(MAX_SCANNED)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if !keep(&name) || !e.file_type().ok()?.is_file() {
                return None;
            }
            let modified = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0));
            Some(LibFile {
                name,
                path: e.path(),
                modified,
            })
        })
        .collect();
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    files.truncate(limit);
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("brujula-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn what_counts_as_a_download_or_a_capture() {
        assert!(is_download("informe.pdf"));
        assert!(!is_download(".informe.pdf.brujula-part"));
        assert!(!is_download(".oculto"));
        assert!(!is_download("video.mp4.part"));
        assert!(is_capture("Brujula-20261115-163000.png"));
        assert!(!is_capture("foto.png"));
        assert!(!is_capture("Brujula-notas.txt"));
    }

    #[test]
    fn the_newest_files_come_first_and_the_list_is_bounded() {
        let dir = scratch("order");
        for (i, name) in ["a.txt", "b.txt", "c.txt"].iter().enumerate() {
            let path = dir.join(name);
            std::fs::write(&path, "x").unwrap();
            let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000 + i as u64);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        std::fs::write(dir.join(".hidden"), "x").unwrap();
        std::fs::create_dir(dir.join("carpeta")).unwrap();
        let names: Vec<String> = recent_files(&dir, is_download, 10)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["c.txt", "b.txt", "a.txt"]);
        assert_eq!(recent_files(&dir, is_download, 2).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn links_and_missing_folders_are_left_out() {
        let dir = scratch("links");
        std::fs::write(dir.join("real.txt"), "x").unwrap();
        std::os::unix::fs::symlink("/etc/hostname", dir.join("enlace.txt")).unwrap();
        let names: Vec<String> = recent_files(&dir, is_download, 10)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["real.txt"]);
        assert!(recent_files(&dir.join("no-existe"), is_download, 10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
