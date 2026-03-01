#![warn(missing_docs)]
//! # local-guard-app
//!
//! ## Purpose
//! Orchestrates auth, capture, mosaic, upload, and UI state for `local-guard`.
//!
//! ## Responsibilities
//! - Enforce auth and consent gates before capture.
//! - Convert chronological frame batches into upload payloads.
//! - Provide transport security checks and kill-switch behavior.
//! - Project analysis responses into UI-safe status signals.
//!
//! ## Data flow
//! Auth/session + UI consent -> capture frames -> mosaic composition -> payload
//! upload -> analysis parsing -> UI projection.
//!
//! ## Ownership and lifetimes
//! This crate passes owned payloads/state snapshots between subsystems to avoid
//! hidden aliasing between long-lived runtime stages.
//!
//! ## Error model
//! Subsystem failures are wrapped in [`AppError`] and categorized for runtime
//! observability.
//!
//! ## Security and privacy notes
//! - Capture is blocked unless auth and consent gates pass.
//! - Kill-switch env var can stop capture safely at runtime.
//! - Log redaction helpers strip token/credential strings.

use local_guard_analysis_contract::{
    AnalysisContractError, UiRiskSignal, map_risk_signals, parse_analysis_response,
};
use local_guard_auth::{AuthError, AuthStateMachine};
use local_guard_capture::{CaptureConfig, DisplayInfo, scheduled_capture_times};
use local_guard_core::{Frame, MosaicPayload, SCHEMA_VERSION_V1, build_metadata};
use local_guard_mosaic::{MosaicError, compose_temporal_mosaic};
use local_guard_ui::UiState;
use local_guard_upload::{UploadClient, UploadError, UploadReport};
use thiserror::Error;
use url::Url;

/// Build-time application version loaded from root `VERSION` file.
pub const APP_VERSION: &str = env!("LOCAL_GUARD_VERSION");

/// Consolidated runtime status snapshot for simple UI projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatus {
    /// Whether auth/consent/display gates currently allow capture.
    pub capture_allowed: bool,
    /// Capture subsystem state as human-readable string.
    pub capture: String,
    /// Network subsystem state.
    pub network: String,
    /// Upload subsystem state.
    pub upload: String,
    /// Analysis status text.
    pub analysis: String,
}

/// Returns the app version sourced from root `VERSION`.
pub fn app_version() -> &'static str {
    APP_VERSION
}

/// Returns `true` when auth state machine allows capture.
pub fn auth_allows_capture(machine: &AuthStateMachine, now_ms: u64) -> bool {
    machine.can_capture(now_ms)
}

/// Schedules deterministic capture times at configured FPS.
///
/// # Errors
/// Returns [`AppError::Capture`] when FPS config is invalid.
pub fn schedule_capture(fps: u32, start_ms: u64, frame_count: usize) -> Result<Vec<u64>, AppError> {
    let config = CaptureConfig::new(fps).map_err(AppError::Capture)?;
    Ok(scheduled_capture_times(config, start_ms, frame_count))
}

/// Selects a display by id from enumerated display list.
pub fn select_display(displays: &[DisplayInfo], display_id: &str) -> Option<DisplayInfo> {
    displays
        .iter()
        .find(|display| display.id == display_id)
        .cloned()
}

/// Builds upload payload from one complete frame batch.
///
/// # Errors
/// Returns [`AppError::Mosaic`] when frame batch is invalid for 3x3 compose.
/// Returns [`AppError::Core`] when metadata construction fails.
pub fn batch_to_payload(frames: &[Frame], session_id: &str) -> Result<MosaicPayload, AppError> {
    let mosaic = compose_temporal_mosaic(frames).map_err(AppError::Mosaic)?;
    let metadata = build_metadata(frames, session_id).map_err(AppError::Core)?;

    Ok(MosaicPayload {
        schema_version: SCHEMA_VERSION_V1.to_string(),
        metadata,
        mosaic_width: mosaic.width,
        mosaic_height: mosaic.height,
        mosaic_rgba: mosaic.rgba,
    })
}

/// Uploads one payload with configured retry semantics.
pub fn upload_payload(
    client: &UploadClient,
    payload: &MosaicPayload,
    token: &str,
) -> Result<UploadReport, UploadError> {
    client.upload_payload(payload, token)
}

/// Parses analysis response and maps to UI signals.
///
/// # Errors
/// Returns [`AppError::Analysis`] when payload parsing fails.
pub fn parse_analysis(raw: &str) -> Result<Vec<UiRiskSignal>, AppError> {
    let parsed = parse_analysis_response(raw).map_err(AppError::Analysis)?;
    Ok(map_risk_signals(&parsed))
}

/// Returns `true` when endpoint URL is HTTPS.
pub fn is_https_endpoint(endpoint: &str) -> bool {
    Url::parse(endpoint)
        .map(|url| url.scheme() == "https")
        .unwrap_or(false)
}

/// Redacts common secret markers in log-safe output.
pub fn redact_sensitive(input: &str) -> String {
    let mut redacted = input.to_string();
    for key in ["password", "token", "authorization", "bearer"] {
        redacted = redact_key_value(&redacted, key);
    }
    redacted
}

fn redact_key_value(input: &str, key: &str) -> String {
    let lower = input.to_ascii_lowercase();
    if let Some(position) = lower.find(key) {
        let prefix = &input[..position];
        return format!("{prefix}{key}=<redacted>");
    }

    input.to_string()
}

/// Checks runtime kill-switch env var.
///
/// Semantics:
/// - Unset => capture enabled.
/// - `0`, `false`, `off` (case-insensitive) => capture disabled.
/// - Any other value => capture enabled.
pub fn capture_enabled_from_env() -> bool {
    match std::env::var("LOCAL_GUARD_CAPTURE_ENABLED") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !(normalized == "0" || normalized == "false" || normalized == "off")
        }
        Err(_) => true,
    }
}

/// Projects UI runtime state into flat status snapshot.
pub fn project_runtime_status(state: &UiState) -> RuntimeStatus {
    RuntimeStatus {
        capture_allowed: state.can_start_capture() && capture_enabled_from_env(),
        capture: format!("{:?}", state.capture),
        network: format!("{:?}", state.network),
        upload: format!("{:?}", state.upload),
        analysis: state.analysis_status.clone(),
    }
}

/// App integration error type.
#[derive(Debug, Error)]
pub enum AppError {
    /// Auth subsystem error.
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    /// Capture subsystem error.
    #[error("capture error: {0}")]
    Capture(local_guard_capture::CaptureError),
    /// Core model error.
    #[error("core error: {0}")]
    Core(local_guard_core::CoreError),
    /// Mosaic composition error.
    #[error("mosaic error: {0}")]
    Mosaic(MosaicError),
    /// Analysis parse/mapping error.
    #[error("analysis error: {0}")]
    Analysis(AnalysisContractError),
}
