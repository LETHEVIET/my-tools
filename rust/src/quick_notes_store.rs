//! Persistent quick notes: ordinary UTF-8 text files, stored separately from
//! the reusable AI prompt draft. No clipboard or network access.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn folder() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|root| PathBuf::from(root).join("MyTools").join("quick-notes"))
        .ok_or_else(|| "Cannot find local Windows user profile".to_owned())
}

pub fn save(text: &str) -> Result<PathBuf, String> {
    save_to(&folder()?, text)
}

pub fn save_to(dir: &Path, text: &str) -> Result<PathBuf, String> {
    if text.trim().is_empty() {
        return Err("Type a note before saving.".into());
    }
    fs::create_dir_all(dir).map_err(|e| format!("Cannot create notes folder: {e}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    for i in 0..100 {
        let path = dir.join(format!("note-{stamp:020}-{i:02}.txt"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(text.as_bytes()) {
                    let _ = fs::remove_file(&path);
                    return Err(format!("Could not write quick note: {error}"));
                }
                file.sync_all()
                    .map_err(|e| format!("Could not save quick note: {e}"))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Could not create quick note: {e}")),
        }
    }
    Err("Could not generate a unique quick note filename".into())
}

pub fn recent(limit: usize) -> Vec<(PathBuf, String)> {
    recent_in(&folder().unwrap_or_default(), limit)
}

fn recent_in(dir: &Path, limit: usize) -> Vec<(PathBuf, String)> {
    let Ok(items) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut paths: Vec<PathBuf> = items
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.starts_with("note-") && name.ends_with(".txt"))
        })
        .collect();
    paths.sort_unstable_by(|a, b| b.cmp(a));
    paths
        .into_iter()
        .take(limit)
        .filter_map(|path| {
            let text = fs::read_to_string(&path).ok()?;
            let first_line = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("(empty note)");
            let preview = first_line.chars().take(58).collect::<String>();
            Some((path, preview))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_distinct_notes_and_lists_latest_first() {
        let dir = std::env::temp_dir().join(format!("quick-notes-test-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        let a = save_to(&dir, "First note\nline 2").unwrap();
        let b = save_to(&dir, "Second note 你好 🚀").unwrap();
        assert_ne!(a, b);
        assert_eq!(fs::read_to_string(&a).unwrap(), "First note\nline 2");
        let recent = recent_in(&dir, 2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].0, b);
        assert_eq!(recent[0].1, "Second note 你好 🚀");
        assert!(save_to(&dir, "  ").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
