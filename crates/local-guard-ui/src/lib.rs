#![warn(missing_docs)]
//! # local-guard-ui
//!
//! ## Purpose
//! Defines the UI-facing runtime state model for `local-guard`.
//!
//! ## Responsibilities
//! - Represent login/auth, consent, display selection, and pipeline statuses.
//! - Project analysis risk signals into display-safe status text.
//! - Expose guard checks for whether capture can start.
//!
//! ## Data flow
//! App orchestration events mutate [`UiState`], which drives rendered status in
//! the desktop shell.
//!
//! ## Ownership and lifetimes
//! `UiState` owns all string/status values to simplify event reducers and
//! minimize cross-thread borrowing complexity.
//!
//! ## Error model
//! This crate favors explicit state over recoverable errors. Invalid
//! combinations are prevented by guard methods.
//!
//! ## Security and privacy notes
//! UI state intentionally excludes secrets (credentials, tokens, raw frames).

use local_guard_analysis_contract::{RiskLevel, UiRiskSignal};

/// UI-auth state projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAuthState {
    /// User is not authenticated.
    Unauthenticated,
    /// Valid authenticated session.
    Authenticated,
    /// Session expired and requires reauth.
    ReauthRequired,
}

/// Generic stage status used for capture/network/upload flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    /// Stage has not started.
    Idle,
    /// Stage is currently running.
    Running,
    /// Stage completed successfully.
    Healthy,
    /// Stage encountered non-fatal error.
    Degraded,
}

/// Aggregate UI runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    /// App version string sourced from root `VERSION`.
    pub version: String,
    /// Current auth status.
    pub auth: UiAuthState,
    /// Whether user explicitly granted capture consent.
    pub consent_granted: bool,
    /// Selected display id.
    pub selected_display: Option<String>,
    /// Capture pipeline stage status.
    pub capture: StageStatus,
    /// Network stage status.
    pub network: StageStatus,
    /// Upload stage status.
    pub upload: StageStatus,
    /// Human-readable analysis status.
    pub analysis_status: String,
}

impl UiState {
    /// Creates default UI state.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            auth: UiAuthState::Unauthenticated,
            consent_granted: false,
            selected_display: None,
            capture: StageStatus::Idle,
            network: StageStatus::Idle,
            upload: StageStatus::Idle,
            analysis_status: "No analysis yet".to_string(),
        }
    }

    /// Sets display selection.
    pub fn select_display(&mut self, display_id: impl Into<String>) {
        self.selected_display = Some(display_id.into());
    }

    /// Sets consent flag.
    pub fn set_consent(&mut self, consent_granted: bool) {
        self.consent_granted = consent_granted;
    }

    /// Returns `true` when user may start capture.
    pub fn can_start_capture(&self) -> bool {
        self.auth == UiAuthState::Authenticated
            && self.consent_granted
            && self.selected_display.is_some()
    }

    /// Updates analysis status from risk signals.
    pub fn apply_risk_signals(&mut self, signals: &[UiRiskSignal]) {
        if signals.is_empty() {
            self.analysis_status = "No risks reported".to_string();
            return;
        }

        let highest = signals
            .iter()
            .map(|signal| signal.level)
            .max_by_key(|level| risk_priority(*level))
            .unwrap_or(RiskLevel::Unknown);

        self.analysis_status = match highest {
            RiskLevel::Low => "Low risk".to_string(),
            RiskLevel::Medium => "Medium risk".to_string(),
            RiskLevel::High => "High risk".to_string(),
            RiskLevel::Critical => "Critical risk".to_string(),
            RiskLevel::Unknown => "Unknown risk".to_string(),
        };
    }
}

fn risk_priority(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 2,
        RiskLevel::High => 3,
        RiskLevel::Critical => 4,
        RiskLevel::Unknown => 0,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for UI state gates.

    use super::*;

    #[test]
    fn capture_gate_requires_auth_consent_and_display() {
        let mut state = UiState::new("v0.1.0");
        assert!(!state.can_start_capture());

        state.auth = UiAuthState::Authenticated;
        state.set_consent(true);
        state.select_display("display-1");

        assert!(state.can_start_capture());
    }
}
