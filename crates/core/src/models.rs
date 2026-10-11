//! Protocol-independent model packages built from explicitly exposed model blocks.
//!
//! Structural schemas are shared vocabulary, not a synthetic HTTP or GraphQL API.
mod plan;
mod plugin;
pub use plan::{ModelPlan, SchemaModelBlock};
pub use plugin::{GeneratedModels, ModelOutput, ModelPlugin, ModelSymbol};
#[cfg(test)]
mod tests;
