use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

pub fn safe_name(input: &str) -> String {
    let mut s: String = input
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(160)
        .collect();
    s = s.trim_matches([' ', '.']).to_owned();
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if s.is_empty()
        || [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
    {
        s = format!("file_{s}");
    }
    s
}
pub fn hash(path: &Path) -> Result<(u64, String)> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut size = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        size += n as u64;
    }
    Ok((size, hex::encode(h.finalize())))
}
// The partial stays in a private staging directory; hard_link atomically publishes
// the verified inode without ever replacing an existing destination.
pub fn publish(part: &Path, dir: &Path, name: &str) -> Result<PathBuf> {
    let name = safe_name(name);
    for i in 0..10000 {
        let dest = dir.join(if i == 0 {
            name.clone()
        } else {
            format!("{}_{}", i, name)
        });
        match std::fs::hard_link(part, &dest) {
            Ok(()) => {
                std::fs::remove_file(part)?;
                return Ok(dest);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    bail!("Too many filename conflicts")
}
pub struct Partial(pub PathBuf);
impl Partial {
    pub fn keep(mut self) -> PathBuf {
        std::mem::take(&mut self.0)
    }
}
impl Drop for Partial {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn create_partial(dir: &Path) -> Result<(Partial, File)> {
    let path = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
    let f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    Ok((Partial(path), f))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_names() {
        for s in ["../CON.txt", "a/b\\c", "NUL", "..", "COM1.jpg", "LPT9"] {
            let n = safe_name(s);
            assert!(!n.contains(['/', '\\', ':']));
            assert!(!n.ends_with('.'));
            assert!(!n.is_empty());
        }
        assert_eq!(safe_name("CON.txt"), "file_CON.txt");
    }
    #[test]
    fn verified_publish_preserves_conflict() {
        let d = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("a.txt"), b"old").unwrap();
        let (p, mut f) = create_partial(&d).unwrap();
        std::io::Write::write_all(&mut f, b"new").unwrap();
        drop(f);
        let out = publish(&p.0, &d, "a.txt").unwrap();
        assert_eq!(std::fs::read(d.join("a.txt")).unwrap(), b"old");
        assert_eq!(std::fs::read(out).unwrap(), b"new");
        std::fs::remove_dir_all(d).unwrap();
    }
}
