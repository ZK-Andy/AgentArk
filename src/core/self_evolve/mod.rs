//! Self-Evolve: policy-first and code-evolution engine
//!
//! Default path evolves runtime strategy/policy with benchmark + lineage + gates.
//! Codebase mutation remains available behind explicit opt-in.

pub mod agent;
pub mod classifier_prompt_evolution;
pub mod coding_guidelines;
pub mod policy_evolution;
pub mod prompt_evolution;
pub mod security_review;
pub mod specialist_prompt_evolution;
pub mod strategy_runtime;
pub mod tools;

pub use agent::{SelfEvolveAgent, SelfEvolveConfig};
pub use classifier_prompt_evolution::{
    ClassifierPromptBundleDiffSummary, ClassifierPromptBundleProfile,
    ClassifierPromptEvolutionConfig, ClassifierPromptEvolutionEngine,
    ClassifierPromptEvolutionResult, CLASSIFIER_PROMPT_BUNDLE_BASELINE_SNAPSHOT_KEY,
    CLASSIFIER_PROMPT_BUNDLE_CANARY_STATE_KEY, CLASSIFIER_PROMPT_BUNDLE_LAST_RESULT_KEY,
    CLASSIFIER_PROMPT_BUNDLE_PROFILE_CANARY_KEY, CLASSIFIER_PROMPT_BUNDLE_PROFILE_KEY,
};
pub use policy_evolution::{
    PolicyEvolutionConfig, PolicyEvolutionEngine, ROUTING_COMPLEXITY_POLICY_KEY,
};
pub use prompt_evolution::{
    PromptBundleDiffSummary, PromptBundleProfile, PromptEvolutionConfig, PromptEvolutionEngine,
    PromptEvolutionResult, PROMPT_BUNDLE_BASELINE_SNAPSHOT_KEY, PROMPT_BUNDLE_CANARY_STATE_KEY,
    PROMPT_BUNDLE_LAST_RESULT_KEY, PROMPT_BUNDLE_PROFILE_CANARY_KEY, PROMPT_BUNDLE_PROFILE_KEY,
};
pub use specialist_prompt_evolution::{
    SpecialistPromptBundleDiffSummary, SpecialistPromptBundleProfile,
    SpecialistPromptEvolutionConfig, SpecialistPromptEvolutionEngine,
    SpecialistPromptEvolutionResult, SPECIALIST_PROMPT_BUNDLE_BASELINE_SNAPSHOT_KEY,
    SPECIALIST_PROMPT_BUNDLE_CANARY_STATE_KEY, SPECIALIST_PROMPT_BUNDLE_LAST_RESULT_KEY,
    SPECIALIST_PROMPT_BUNDLE_PROFILE_CANARY_KEY, SPECIALIST_PROMPT_BUNDLE_PROFILE_KEY,
};
