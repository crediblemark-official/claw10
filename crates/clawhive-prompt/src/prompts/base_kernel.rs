/// Base kernel prompt, loaded from external ICVS file.
pub const BASE_KERNEL_SOURCE: &str = include_str!("../../prompts/base_kernel.icvs");

#[cfg(test)]
#[path = "base_kernel_test.rs"]
mod tests;

