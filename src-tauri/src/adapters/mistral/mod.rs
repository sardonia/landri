//! mistral.rs adapter helpers.
//!
//! The heavy lifting is done by `InferenceManager` (in `managers/`) which owns
//! loaded model instances. This module just contains small, testable helper
//! functions that are specific to mistral.rs integration.

/// Best-effort mapping from a GGUF repo id to a tokenizer/chat-template repo.
///
/// Many GGUF repositories (especially community quantizations) do not ship the
/// original tokenizer assets. `mistralrs` can fetch tokenizer/chat-template
/// artifacts from an upstream HF repo when provided.
pub fn tok_model_id_hint(repo_id: &str) -> String {
    let lower = repo_id.to_lowercase();
    if lower.contains("phi-3-mini-4k-instruct") {
        "microsoft/Phi-3-mini-4k-instruct".to_string()
    } else {
        repo_id.to_string()
    }
}
