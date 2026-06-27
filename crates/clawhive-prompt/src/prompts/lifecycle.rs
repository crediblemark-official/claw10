/// Lifecycle prompts, loaded from external ICVS file.
pub const LIFECYCLE_SOURCE: &str = include_str!("../../prompts/lifecycle.icvs");

#[cfg(test)]
#[path = "lifecycle_test.rs"]
mod tests;

