//! Durable editorial state kept in the same atomic envelope as the current project.

use serde::{Deserialize, Serialize};

/// Opaque identity, preserved by Save As and recovery. Historical projects adopt an identity
/// when opened; their original files are not changed on open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId(uuid::Uuid);

impl Default for DocumentId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Each snapshot is a versioned project envelope, so project migrations apply to retained
/// undo states too. History snapshot envelopes cannot contain nested session state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistorySnapshot {
    pub label: String,
    pub snapshot: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentState {
    pub version: u32,
    pub project_id: DocumentId,
    #[serde(with = "decimal_u64")]
    pub revision: u64,
    #[serde(with = "decimal_u64")]
    pub next_id: u64,
    pub undo: Vec<HistorySnapshot>,
    pub redo: Vec<HistorySnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<OperationRecord>,
}

/// A committed operation receipt. Credentials and grants never travel in project files.
/// `request` is a digest, not a prompt or a serialized request containing private paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationRecord {
    pub actor: String,
    pub operation_id: String,
    pub idempotency_key: String,
    pub request: String,
    #[serde(with = "decimal_u64")]
    pub sequence: u64,
    pub retained_until: i64,
    pub result: serde_json::Value,
}

/// Bind recovery location and legacy protection to the snapshot instead of a separately
/// written session.json file which could describe an older revision after an interrupted save.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryInfo {
    pub project_path: Option<String>,
    pub legacy_identity: bool,
    /// Schema of the original at project_path, which can differ from this recovery snapshot.
    #[serde(default)]
    pub source_schema: u32,
}

pub mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value = text.parse::<u64>().map_err(D::Error::custom)?;
        if value.to_string() != text {
            return Err(D::Error::custom("expected a canonical unsigned decimal string"));
        }
        Ok(value)
    }
}
