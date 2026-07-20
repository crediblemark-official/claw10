//! Admission pipeline untuk memory candidate.
//!
//! Mengimplementasikan PRD section 27.3:
//!
//! ```text
//! Candidate
//! → Classification
//! → Injection Scan
//! → Deduplication
//! → Source Check
//! → Confidence Score
//! → Scope Assignment
//! → Verification
//! → Activation
//! ```
//!
//! Setiap stage mengembalikan `AdmissionDecision`.

use claw10_domain::{Memory, MemoryId, MemoryStatus};

/// Hasil dari satu tahap admission pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum AdmissionDecision {
    /// Lanjut ke tahap berikutnya.
    Continue,
    /// Pipeline selesai, memory diaktifkan.
    Activate,
    /// Pipeline ditolak, memory di-reject.
    Reject { reason: String },
}

/// Konfigurasi admission pipeline.
#[derive(Debug, Clone)]
pub struct AdmissionConfig {
    /// Confidence minimum untuk lolos pipeline.
    pub min_confidence: f64,
    /// Kata kunci yang mengindikasikan injection attack.
    pub injection_keywords: Vec<String>,
    /// Apakah memory duplicate diperbolehkan (same scope + content).
    pub allow_duplicates: bool,
    /// Jumlah minimum verifikasi dari agent lain yang dibutuhkan.
    pub minimum_verifiers: usize,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            injection_keywords: vec![
                "ignore previous".into(),
                "disregard".into(),
                "forget all".into(),
                "system prompt".into(),
                "jailbreak".into(),
                "</system>".into(),
                "<|im_start|>".into(),
            ],
            allow_duplicates: false,
            minimum_verifiers: 1,
        }
    }
}

/// Hasil akhir admission pipeline.
#[derive(Debug)]
pub enum AdmissionResult {
    /// Memory diaktifkan.
    Activated,
    /// Memory ditolak, dengan alasan.
    Rejected { reason: String },
}

/// Admission pipeline untuk memory candidate.
pub struct AdmissionPipeline {
    config: AdmissionConfig,
}

impl AdmissionPipeline {
    #[must_use]
    pub fn new(config: AdmissionConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(AdmissionConfig::default())
    }

    /// Jalankan seluruh pipeline terhadap satu memory candidate.
    /// Mengembalikan `AdmissionResult` yang menentukan apakah memory diaktifkan.
    pub fn evaluate(&self, candidate: &Memory, existing: &[Memory]) -> AdmissionResult {
        // Pastikan candidate dalam status Candidate
        if candidate.status != MemoryStatus::Candidate
            && candidate.status != MemoryStatus::Scanning
        {
            return AdmissionResult::Rejected {
                reason: format!(
                    "status tidak valid untuk admission: {:?}",
                    candidate.status
                ),
            };
        }

        // Stage 1: Injection scan
        if let AdmissionDecision::Reject { reason } = self.scan_injection(&candidate.content) {
            return AdmissionResult::Rejected { reason };
        }

        // Stage 2: Confidence check
        if let AdmissionDecision::Reject { reason } =
            self.check_confidence(candidate.confidence)
        {
            return AdmissionResult::Rejected { reason };
        }

        // Stage 3: Deduplication
        if let AdmissionDecision::Reject { reason } =
            self.check_duplicate(candidate, existing)
        {
            return AdmissionResult::Rejected { reason };
        }

        // Stage 4: Source check (harus punya source agent yang valid)
        if let AdmissionDecision::Reject { reason } = self.check_source(candidate) {
            return AdmissionResult::Rejected { reason };
        }

        // Stage 5: Classification check (tidak boleh empty)
        if let AdmissionDecision::Reject { reason } = self.check_classification(candidate) {
            return AdmissionResult::Rejected { reason };
        }

        // Stage 6: Verification check (jumlah minimum verifier)
        if let AdmissionDecision::Reject { reason } =
            self.check_verification(candidate.verified_by.len())
        {
            return AdmissionResult::Rejected { reason };
        }

        AdmissionResult::Activated
    }

    // ── Stages ────────────────────────────────────────────────────

    fn scan_injection(&self, content: &str) -> AdmissionDecision {
        let lower = content.to_lowercase();
        for keyword in &self.config.injection_keywords {
            if lower.contains(keyword.as_str()) {
                return AdmissionDecision::Reject {
                    reason: format!("injection pattern terdeteksi: '{keyword}'"),
                };
            }
        }
        AdmissionDecision::Continue
    }

    fn check_confidence(&self, confidence: f64) -> AdmissionDecision {
        if confidence < self.config.min_confidence {
            AdmissionDecision::Reject {
                reason: format!(
                    "confidence {:.2} di bawah minimum {:.2}",
                    confidence, self.config.min_confidence
                ),
            }
        } else {
            AdmissionDecision::Continue
        }
    }

    fn check_duplicate(&self, candidate: &Memory, existing: &[Memory]) -> AdmissionDecision {
        if self.config.allow_duplicates {
            return AdmissionDecision::Continue;
        }

        for mem in existing {
            if mem.id == candidate.id {
                continue; // sama diri sendiri
            }
            if mem.scope == candidate.scope
                && mem.content.trim().to_lowercase()
                    == candidate.content.trim().to_lowercase()
                && mem.status == MemoryStatus::Active
            {
                return AdmissionDecision::Reject {
                    reason: format!(
                        "duplikat dengan memory aktif {} dalam scope '{}'",
                        mem.id.0, candidate.scope
                    ),
                };
            }
        }

        AdmissionDecision::Continue
    }

    fn check_source(&self, candidate: &Memory) -> AdmissionDecision {
        // Agent ID tidak boleh nil UUID
        if candidate.source.agent_id.0.is_nil() {
            return AdmissionDecision::Reject {
                reason: "source agent_id tidak valid (nil UUID)".into(),
            };
        }
        AdmissionDecision::Continue
    }

    fn check_classification(&self, candidate: &Memory) -> AdmissionDecision {
        if candidate.classification.trim().is_empty() {
            return AdmissionDecision::Reject {
                reason: "classification tidak boleh kosong".into(),
            };
        }
        AdmissionDecision::Continue
    }

    fn check_verification(&self, verifier_count: usize) -> AdmissionDecision {
        if verifier_count < self.config.minimum_verifiers {
            AdmissionDecision::Reject {
                reason: "insufficient verifiers".into(),
            }
        } else {
            AdmissionDecision::Continue
        }
    }
}

/// Helper untuk mendapatkan MemoryId dari rejection target.
pub fn rejection_reason_for(id: &MemoryId, reason: &str) -> String {
    format!("memory {} ditolak: {}", id.0, reason)
}

#[cfg(test)]
#[path = "admission_test.rs"]
mod tests;

