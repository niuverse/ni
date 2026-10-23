//! Attachment references stored in the append-only event log.
//!
//! Binary data lives in `niucode-attachments`; events carry only this metadata.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub sha256: String,
    pub media_type: String,
    pub kind: AttachmentKind,
    pub bytes: u64,
    /// Character count for text; zero for images.
    pub text_chars: u64,
    pub name: Option<String>,
    pub alt: Option<String>,
}
