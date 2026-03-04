//! Analysis module - Analysis bundles and Docker execution
//!
//! This module implements:
//! - Manifest parsing for .analysis directories
//! - Analysis discovery in the project tree
//! - Bundle rendering (templates + static files)
//! - Docker container execution
//! - Output validation

pub mod discovery;
pub mod docker;
pub mod manifest;
pub mod renderer;
pub mod runner;
pub mod validation;

// Re-export commonly used types
pub use discovery::{AnalysisBundle, DiscoveredAnalyses};
pub use docker::{DockerConfig, ExecutionResult};
pub use manifest::{InputSpec, Manifest, OutputSpec, ResourceLimits, TemplateSpec};
pub use renderer::RenderedAnalysis;
pub use runner::{DockerClientResource, ExecutedAnalysis};
pub use validation::ValidatedAnalysis;
