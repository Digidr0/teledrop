use std::path::{Path, PathBuf};

pub fn sanitize_filename(input: &str) -> String {
    let clean: String = input
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']);
    let clean: String = clean.chars().take(150).collect();
    let stem = clean.split('.').next().unwrap_or("").to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|n| {
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
            })
        });
    if clean.is_empty() {
        "media".into()
    } else if reserved {
        format!("_{clean}")
    } else {
        clean.trim_end_matches(['.', ' ']).into()
    }
}

pub fn collision_path(directory: &Path, filename: &str, index: u32) -> PathBuf {
    let name = sanitize_filename(filename);
    if index == 0 {
        return directory.join(name);
    }
    let path = Path::new(&name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    match path.extension() {
        Some(ext) => directory.join(format!("{stem} ({index}).{}", ext.to_string_lossy())),
        None => directory.join(format!("{stem} ({index})")),
    }
}

pub async fn validate_directory(path: &str) -> anyhow::Result<()> {
    let dir = Path::new(path);
    if !dir.is_absolute() || !dir.is_dir() {
        anyhow::bail!("Выберите существующую абсолютную папку.");
    }
    let probe = dir.join(format!(".teledrop-check-{}", uuid::Uuid::new_v4()));
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .await
        .map_err(|_| anyhow::anyhow!("Нет разрешения на запись в папку."))?;
    drop(file);
    tokio::fs::remove_file(probe).await?;
    Ok(())
}

pub fn open(path: &Path, reveal: bool) -> anyhow::Result<()> {
    if !path.exists() {
        anyhow::bail!("Файл или папка больше не существует.");
    }
    let mut command = std::process::Command::new("explorer.exe");
    if reveal {
        command.arg("/select,");
    }
    command.arg(path);
    command.spawn()?;
    Ok(())
}

pub async fn sha256(path: &Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}

/// Same-volume Windows move without REPLACE_EXISTING or COPY_ALLOWED.
/// Supports removable filesystems without relying on hard-link support.
pub fn publish_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: owned NUL-terminated UTF-16 arrays remain valid throughout the call.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_names() {
        assert_eq!(sanitize_filename("CON.mp4"), "_CON.mp4");
        assert_eq!(sanitize_filename("LPT¹.txt"), "_LPT¹.txt");
        assert_eq!(sanitize_filename("../фото:1?.jpg "), ".._фото_1_.jpg");
        assert_eq!(sanitize_filename(". ."), "media");
        assert_eq!(sanitize_filename("видео 🎬.mp4"), "видео 🎬.mp4");
        assert!(!sanitize_filename("a\\b/c:stream").contains(['\\', '/', ':']));
    }
    #[test]
    fn collisions_keep_extension() {
        assert_eq!(
            collision_path(Path::new("C:/media"), "clip.mp4", 2),
            PathBuf::from("C:/media/clip (2).mp4")
        );
    }
}
