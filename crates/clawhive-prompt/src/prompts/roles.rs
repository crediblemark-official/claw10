/// All role prompts, loaded from external ICVS file.
pub const ROLES_SOURCE: &str = include_str!("../../prompts/roles.icvs");

#[cfg(test)]
#[path = "roles_test.rs"]
mod tests;

