mod assembler;
mod audit;
mod bundle;
mod policy_digest;
mod prompts;
mod registry;
mod validation;

pub use assembler::*;
pub use audit::*;
pub use bundle::*;
pub use policy_digest::*;
pub use prompts::*;
pub use registry::*;
pub use validation::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum AgentRole {
    Root,
    Director,
    Planner,
    Orchestrator,
    Manager,
    Specialist,
    Research,
    Browser,
    Coding,
    Data,
    Communication,
    Device,
    Critic,
    Verifier,
    Judge,
    SecurityGuardian,
    MemoryCurator,
    SkillEngineer,
    CostController,
    Recovery,
    Watcher,
    Maintenance,
}

impl AgentRole {
    #[must_use]
    pub fn temperature(&self) -> f64 {
        match self {
            Self::Root | Self::Director => 0.3,
            Self::Planner
            | Self::Orchestrator
            | Self::Manager
            | Self::Research
            | Self::Critic
            | Self::SkillEngineer => 0.2,
            Self::Coding | Self::Data => 0.1,
            Self::Verifier | Self::MemoryCurator | Self::CostController | Self::Watcher | Self::Maintenance => 0.0,
            Self::Judge | Self::Recovery => 0.1,
            Self::SecurityGuardian => 0.1,
            _ => 0.2,
        }
    }

    #[must_use]
    pub fn max_output_tokens(&self) -> u32 {
        match self {
            Self::Root | Self::Director | Self::Planner => 4096,
            Self::Orchestrator | Self::Manager => 4096,
            Self::Specialist | Self::Research | Self::Coding | Self::Data => 8192,
            Self::Browser | Self::Communication | Self::Device => 4096,
            Self::Critic | Self::Verifier | Self::Judge | Self::SecurityGuardian => 2048,
            Self::MemoryCurator | Self::SkillEngineer | Self::CostController => 2048,
            Self::Recovery | Self::Watcher | Self::Maintenance => 2048,
        }
    }

    #[must_use]
    pub fn primary_output(&self) -> &'static str {
        match self {
            Self::Root => "MissionProposal",
            Self::Director => "DirectorDecision",
            Self::Planner => "TaskGraphProposal",
            Self::Orchestrator => "SpawnProposal",
            Self::Manager => "WorkstreamDecision",
            Self::Specialist => "WorkResult",
            Self::Research => "ResearchReport",
            Self::Browser => "BrowserExecutionReport",
            Self::Coding => "CodeChangeResult",
            Self::Data => "DataWorkResult",
            Self::Communication => "CommunicationDraft",
            Self::Device => "DeviceActionProposal",
            Self::Critic => "CritiqueReport",
            Self::Verifier => "VerificationDecision",
            Self::Judge => "JudgeDecision",
            Self::SecurityGuardian => "SecurityAssessment",
            Self::MemoryCurator => "MemoryAdmissionDecision",
            Self::SkillEngineer => "SkillCandidate",
            Self::CostController => "CostAssessment",
            Self::Recovery => "RecoveryPlan",
            Self::Watcher => "WatchDecision",
            Self::Maintenance => "MaintenanceReport",
        }
    }

    /// All 22 role variants for exhaustive iteration.
    #[must_use]
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::Root,
            Self::Director,
            Self::Planner,
            Self::Orchestrator,
            Self::Manager,
            Self::Specialist,
            Self::Research,
            Self::Browser,
            Self::Coding,
            Self::Data,
            Self::Communication,
            Self::Device,
            Self::Critic,
            Self::Verifier,
            Self::Judge,
            Self::SecurityGuardian,
            Self::MemoryCurator,
            Self::SkillEngineer,
            Self::CostController,
            Self::Recovery,
            Self::Watcher,
            Self::Maintenance,
        ]
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

