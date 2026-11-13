//! `media_key` (ARCHITECTURE §9.2): identity independent of the path, so resume, per-file
//! memory, bookmarks, and presets survive renames and moves.
//!
//! - Local file: `"f:"` + the 64-bit OpenSubtitles hash (file size plus the little-endian u64
//!   sum of the first and last 64 KiB). Two small reads, no matter how large the file.
//! - URL: `"u:"` + the first 16 hex digits of SHA-256 of the normalised URL.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

const CHUNK: u64 = 64 * 1024;

/// The key for a source (path or URL). Fails only when a local file cannot be read.
pub fn media_key(source: &str) -> std::io::Result<String> {
    if crate::cli::is_url(source) {
        Ok(url_key(source))
    } else {
        Ok(format!("f:{:016x}", oshash(Path::new(source))?))
    }
}

pub fn oshash(path: &Path) -> std::io::Result<u64> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hash = size;
    let mut add_chunk = |file: &mut File, from: u64| -> std::io::Result<()> {
        file.seek(SeekFrom::Start(from))?;
        let mut buf = Vec::with_capacity(CHUNK as usize);
        file.take(CHUNK).read_to_end(&mut buf)?;
        // A short final word is zero-padded (only possible for tiny files).
        buf.resize(buf.len().div_ceil(8) * 8, 0);
        for word in buf.chunks_exact(8) {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(word);
            hash = hash.wrapping_add(u64::from_le_bytes(bytes));
        }
        Ok(())
    };
    add_chunk(&mut file, 0)?;
    add_chunk(&mut file, size.saturating_sub(CHUNK))?;
    Ok(hash)
}

/// Trims, drops the fragment, lower-cases scheme and host, and drops a default port.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();
    let url = url.split('#').next().unwrap_or(url);
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let scheme = scheme.to_ascii_lowercase();
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    // Keep any user-info as written; lower-case only the host.
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((user, host)) => (format!("{user}@"), host),
        None => (String::new(), authority),
    };
    let mut host = hostport.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "http" => Some(":80"),
        "https" => Some(":443"),
        _ => None,
    };
    if let Some(port) = default_port {
        if let Some(stripped) = host.strip_suffix(port) {
            host = stripped.to_string();
        }
    }
    format!("{scheme}://{userinfo}{host}{path}")
}

pub fn url_key(url: &str) -> String {
    let digest = Sha256::digest(normalize_url(url).as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("u:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vihpluh-identity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    // Expected values computed independently (Python reference implementation).
    #[test]
    fn oshash_matches_the_reference() {
        let big: Vec<u8> = (0..200_000u32).map(|i| ((i * 7 + 3) % 256) as u8).collect();
        let small: Vec<u8> = (0..1000u32).map(|i| ((i * 13 + 1) % 256) as u8).collect();
        let big = temp_file("big.bin", &big);
        let small = temp_file("small.bin", &small);
        assert_eq!(format!("{:016x}", oshash(&big).unwrap()), "60a0df1f5fa2cd40");
        assert_eq!(format!("{:016x}", oshash(&small).unwrap()), "3881d11a6bbd09c2");
        assert_eq!(media_key(&big.display().to_string()).unwrap(), "f:60a0df1f5fa2cd40");
    }

    #[test]
    fn a_renamed_file_keeps_its_key() {
        let bytes: Vec<u8> = (0..150_000u32).map(|i| (i % 251) as u8).collect();
        let a = temp_file("episode.mkv", &bytes);
        let b = temp_file("renamed copy.mkv", &bytes);
        assert_eq!(
            media_key(&a.display().to_string()).unwrap(),
            media_key(&b.display().to_string()).unwrap()
        );
    }

    #[test]
    fn url_keys() {
        assert_eq!(url_key("https://example.com/stream.m3u8"), "u:9e742e9ad8b5c6dd");
        assert_eq!(url_key("  http://Example.COM:80/a/b?x=1#frag "), "u:3153e1a7deffd9c0");
        assert_eq!(url_key("http://example.com/a/b?x=1"), "u:3153e1a7deffd9c0");
        assert_eq!(media_key("https://example.com/stream.m3u8").unwrap(), "u:9e742e9ad8b5c6dd");
    }

    #[test]
    fn normalization_keeps_what_matters() {
        assert_eq!(normalize_url("HTTPS://Host.TV:443/Path?Q=1"), "https://host.tv/Path?Q=1");
        assert_eq!(normalize_url("rtsp://cam:554/live"), "rtsp://cam:554/live");
        assert_eq!(normalize_url("http://User:Pw@Host:8080/x"), "http://User:Pw@host:8080/x");
    }

    #[test]
    fn a_missing_file_is_an_error() {
        assert!(media_key("Z:/definitely/missing.mkv").is_err());
    }
}
