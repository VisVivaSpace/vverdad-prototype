//! Message types for error handling
//!
//! ECS Pattern: Use messages for error reporting instead of panics.
//! DOP Pattern: Data structs with no behavior — use struct literals at call sites.

use std::path::PathBuf;

use bevy_ecs::prelude::*;

use crate::error::VVError;

// =============================================================================
// Error Messages (Pure Data)
// =============================================================================

/// Message emitted when loading a file or directory fails
#[derive(Message)]
pub struct LoadError {
    pub path: PathBuf,
    pub error: VVError,
}

/// Message emitted when dependency validation fails
#[derive(Message)]
pub struct ValidationError {
    pub unmet: Vec<String>,
}

/// Message emitted when template rendering fails
#[derive(Message)]
pub struct RenderError {
    pub template_name: String,
    pub error: VVError,
}

// =============================================================================
// Analysis Bundle Messages
// =============================================================================

/// Message emitted when manifest parsing fails
#[derive(Message)]
pub struct ManifestError {
    pub path: PathBuf,
    pub error: VVError,
}

// =============================================================================
// Processing Status Resource
// =============================================================================

/// Resource tracking whether processing should continue or abort
#[derive(Resource, Default)]
pub struct ProcessingStatus {
    pub has_errors: bool,
}

/// Marks that an error has occurred
pub fn mark_error(status: &mut ProcessingStatus) {
    status.has_errors = true;
}

/// Returns true if no errors have occurred and processing should continue
pub fn should_continue(status: &ProcessingStatus) -> bool {
    !status.has_errors
}
