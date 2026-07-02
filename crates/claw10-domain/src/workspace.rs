use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Workspace merepresentasikan konteks isolasi data untuk satu sesi kerja agen.
/// Setiap workspace memiliki namespace unik yang mengasingkan data dari workspace lain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    /// ID unik: 8 karakter hex SHA-256 dari nama workspace.
    pub id: String,
    /// Nama workspace yang diberikan pengguna.
    pub name: String,
    /// Deskripsi singkat opsional.
    pub description: Option<String>,
    /// Waktu workspace pertama kali dibuat.
    pub created_at: DateTime<Utc>,
    /// Waktu terakhir workspace digunakan (diperbarui tiap kali workspace dipilih).
    pub last_used_at: DateTime<Utc>,
}

impl Workspace {
    /// Buat workspace baru dengan nama dan deskripsi tertentu.
    pub fn new(name: impl Into<String>, description: Option<String>) -> Self {
        let name = name.into();
        let id = workspace_id_from_name(&name);
        let now = Utc::now();
        Self {
            id,
            name,
            description,
            created_at: now,
            last_used_at: now,
        }
    }

    /// Namespace prefix yang digunakan di database: `"ws:{id}:"`.
    #[must_use]
    pub fn namespace(&self) -> String {
        format!("ws:{}:", self.id)
    }

    /// Key database untuk menyimpan metadata workspace ini.
    #[must_use]
    pub fn store_key(&self) -> String {
        format!("workspace:{}", self.id)
    }
}

/// Hitung workspace ID dari nama: 8 karakter hex SHA-256 dari nama workspace.
#[must_use]
pub fn workspace_id_from_name(name: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}
