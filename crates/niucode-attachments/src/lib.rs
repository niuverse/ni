//! Content-addressed image and text attachments.
//!
//! The event log stores references (`sha256`, media type, size, kind), never
//! base64. Files are written atomically with restrictive permissions and are
//! deduplicated by content hash. Budget checks are deliberately fail-closed:
//! oversized or unsupported attachments are rejected before a request is sent.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_FILES_PER_TURN: usize = 4;
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_TEXT_CHARS: usize = 20_000;
pub const MAX_TEXT_TOKENS: usize = 4_000;
pub const IMAGE_TOKEN_ESTIMATE: usize = 1_024;
pub const ATTACHMENT_CONTEXT_FRACTION: usize = 4;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub use niucode_core::{AttachmentKind, AttachmentRef};

#[derive(Debug, Clone)]
pub struct AttachmentStore {
    root: PathBuf,
}

impl AttachmentStore {
    /// Create/open a store. The directory is private (`0700` on Unix).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AttachmentError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        set_private_dir(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ingest bytes, classify them, enforce per-file limits, and persist by
    /// SHA-256. The original name is metadata only; it never becomes a path.
    pub fn put(
        &self,
        bytes: &[u8],
        name: Option<&str>,
        alt: Option<&str>,
    ) -> Result<AttachmentRef, AttachmentError> {
        let (kind, media_type, _extension) = classify(bytes)?;
        let text_chars = match kind {
            AttachmentKind::Image => {
                if bytes.len() as u64 > MAX_IMAGE_BYTES {
                    return Err(AttachmentError::ImageTooLarge {
                        bytes: bytes.len() as u64,
                        limit: MAX_IMAGE_BYTES,
                    });
                }
                0
            }
            AttachmentKind::Text => {
                let text = std::str::from_utf8(bytes).map_err(|_| AttachmentError::NotUtf8)?;
                let chars = text.chars().count();
                if chars == 0 {
                    return Err(AttachmentError::Empty);
                }
                if chars > MAX_TEXT_CHARS {
                    return Err(AttachmentError::TextTooLong {
                        chars,
                        limit: MAX_TEXT_CHARS,
                    });
                }
                chars as u64
            }
        };
        let hash = hex(&Sha256::digest(bytes));
        let reference = AttachmentRef {
            sha256: hash.clone(),
            media_type: media_type.to_string(),
            kind,
            bytes: bytes.len() as u64,
            text_chars,
            name: clean_name(name),
            alt: clean_name(alt),
        };
        let path = self.path_for(&reference)?;
        if !path.exists() {
            self.write_atomic(&path, bytes)?;
        }
        Ok(reference)
    }

    /// Load bytes for a trusted reference. Invalid hashes cannot escape `root`.
    pub fn load(&self, reference: &AttachmentRef) -> Result<Vec<u8>, AttachmentError> {
        Ok(fs::read(self.path_for(reference)?)?)
    }

    pub fn text(&self, reference: &AttachmentRef) -> Result<String, AttachmentError> {
        if reference.kind != AttachmentKind::Text {
            return Err(AttachmentError::WrongKind);
        }
        let bytes = self.load(reference)?;
        String::from_utf8(bytes).map_err(|_| AttachmentError::NotUtf8)
    }

    pub fn image_data_url(&self, reference: &AttachmentRef) -> Result<String, AttachmentError> {
        if reference.kind != AttachmentKind::Image {
            return Err(AttachmentError::WrongKind);
        }
        let bytes = self.load(reference)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        Ok(format!("data:{};base64,{encoded}", reference.media_type))
    }

    fn path_for(&self, reference: &AttachmentRef) -> Result<PathBuf, AttachmentError> {
        if reference.sha256.len() != 64
            || !reference
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(AttachmentError::InvalidHash);
        }
        let extension = match reference.kind {
            AttachmentKind::Image => match reference.media_type.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                _ => return Err(AttachmentError::UnsupportedType),
            },
            AttachmentKind::Text => "txt",
        };
        Ok(self
            .root
            .join(format!("{}.{}", reference.sha256, extension)))
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<(), AttachmentError> {
        let temp = self.root.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        drop(file);
        if let Err(error) = fs::rename(&temp, path) {
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        Ok(())
    }
}

/// Enforce per-turn limits and the shared 25% attachment/context budget.
pub fn check_budget(
    attachments: &[AttachmentRef],
    context_budget_tokens: usize,
) -> Result<(), AttachmentError> {
    if attachments.len() > MAX_FILES_PER_TURN {
        return Err(AttachmentError::TooManyFiles {
            count: attachments.len(),
            limit: MAX_FILES_PER_TURN,
        });
    }
    let mut text_tokens = 0;
    let mut image_tokens = 0;
    for attachment in attachments {
        match attachment.kind {
            AttachmentKind::Image => {
                if attachment.bytes > MAX_IMAGE_BYTES {
                    return Err(AttachmentError::ImageTooLarge {
                        bytes: attachment.bytes,
                        limit: MAX_IMAGE_BYTES,
                    });
                }
                image_tokens += IMAGE_TOKEN_ESTIMATE;
            }
            AttachmentKind::Text => {
                if attachment.text_chars as usize > MAX_TEXT_CHARS {
                    return Err(AttachmentError::TextTooLong {
                        chars: attachment.text_chars as usize,
                        limit: MAX_TEXT_CHARS,
                    });
                }
                text_tokens += (attachment.text_chars as usize).div_ceil(4);
            }
        }
    }
    let total = text_tokens + image_tokens;
    let allowance = context_budget_tokens / ATTACHMENT_CONTEXT_FRACTION;
    if total > allowance {
        return Err(AttachmentError::BudgetExceeded {
            needed: total,
            allowed: allowance,
        });
    }
    if text_tokens > MAX_TEXT_TOKENS {
        return Err(AttachmentError::TextBudgetExceeded {
            needed: text_tokens,
            allowed: MAX_TEXT_TOKENS,
        });
    }
    Ok(())
}

fn classify(bytes: &[u8]) -> Result<(AttachmentKind, &'static str, &'static str), AttachmentError> {
    if bytes.is_empty() {
        return Err(AttachmentError::Empty);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok((AttachmentKind::Image, "image/png", "png"));
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Ok((AttachmentKind::Image, "image/jpeg", "jpg"));
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok((AttachmentKind::Image, "image/webp", "webp"));
    }
    if bytes.contains(&0) {
        return Err(AttachmentError::UnsupportedType);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| AttachmentError::UnsupportedType)?;
    if text.trim().is_empty() {
        return Err(AttachmentError::Empty);
    }
    Ok((AttachmentKind::Text, "text/plain; charset=utf-8", "txt"))
}

fn clean_name(value: Option<&str>) -> Option<String> {
    value
        .map(|value| {
            value
                .chars()
                .filter(|character| !character.is_control())
                .take(128)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

#[cfg(unix)]
fn set_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug, Error)]
pub enum AttachmentError {
    #[error("attachment is empty")]
    Empty,
    #[error("unsupported attachment type")]
    UnsupportedType,
    #[error("attachment is not valid UTF-8")]
    NotUtf8,
    #[error("wrong attachment kind")]
    WrongKind,
    #[error("invalid attachment hash")]
    InvalidHash,
    #[error("image is {bytes} bytes; limit is {limit}")]
    ImageTooLarge { bytes: u64, limit: u64 },
    #[error("text has {chars} characters; limit is {limit}")]
    TextTooLong { chars: usize, limit: usize },
    #[error("too many attachments: {count}; limit is {limit}")]
    TooManyFiles { count: usize, limit: usize },
    #[error("attachment budget needs {needed} tokens; allowance is {allowed}")]
    BudgetExceeded { needed: usize, allowed: usize },
    #[error("text budget needs {needed} tokens; allowance is {allowed}")]
    TextBudgetExceeded { needed: usize, allowed: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot-a-real-png";

    #[test]
    fn images_are_content_addressed_and_private() {
        let directory = tempfile::tempdir().unwrap();
        let store = AttachmentStore::open(directory.path().join("attachments")).unwrap();
        let first = store.put(PNG, Some("shot.png"), Some("desktop")).unwrap();
        let second = store
            .put(PNG, Some("../escape.png"), Some("desktop"))
            .unwrap();
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.name.as_deref(), Some("shot.png"));
        assert_eq!(second.name.as_deref(), Some("../escape.png"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = store.path_for(&first).unwrap();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(store.root()).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(store.load(&first).unwrap(), PNG);
        assert!(store
            .image_data_url(&first)
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn text_is_stored_and_limited() {
        let directory = tempfile::tempdir().unwrap();
        let store = AttachmentStore::open(directory.path()).unwrap();
        let reference = store
            .put(b"print('hi')\n", Some("script.py"), None)
            .unwrap();
        assert_eq!(reference.kind, AttachmentKind::Text);
        assert_eq!(reference.text_chars, 12);
        assert_eq!(store.text(&reference).unwrap(), "print('hi')\n");
        assert!(matches!(
            store.put(&vec![b'a'; MAX_TEXT_CHARS + 1], None, None),
            Err(AttachmentError::TextTooLong { .. })
        ));
        assert!(matches!(
            store.put(b"a\0b", None, None),
            Err(AttachmentError::UnsupportedType)
        ));
    }

    #[test]
    fn budget_rejects_oversize_and_too_many() {
        let directory = tempfile::tempdir().unwrap();
        let store = AttachmentStore::open(directory.path()).unwrap();
        let text = store.put(b"hello", None, None).unwrap();
        let mut refs = vec![text; MAX_FILES_PER_TURN + 1];
        assert!(matches!(
            check_budget(&refs, 64_000),
            Err(AttachmentError::TooManyFiles { .. })
        ));
        refs.truncate(1);
        assert!(check_budget(&refs, 64_000).is_ok());

        let small_context = store.put(b"hello", None, None).unwrap();
        assert!(matches!(
            check_budget(&[small_context], 4),
            Err(AttachmentError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn invalid_hashes_cannot_escape() {
        let directory = tempfile::tempdir().unwrap();
        let store = AttachmentStore::open(directory.path()).unwrap();
        let reference = AttachmentRef {
            sha256: "../escape".into(),
            media_type: "text/plain".into(),
            kind: AttachmentKind::Text,
            bytes: 1,
            text_chars: 1,
            name: None,
            alt: None,
        };
        assert!(matches!(
            store.load(&reference),
            Err(AttachmentError::InvalidHash)
        ));
    }
}
