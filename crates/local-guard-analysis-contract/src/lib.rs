#![warn(missing_docs)]
//! # local-guard-analysis-contract
//!
//! ## Purpose
//! Defines the server analysis response schema and client-side mapping helpers.
//!
//! ## Responsibilities
//! - Parse versioned analysis response payloads.
//! - Map risk categories to UI-safe risk levels.
//! - Preserve unknown categories for forward compatibility.
//!
//! ## Data flow
//! Raw JSON response -> [`parse_analysis_response`] -> [`map_risk_signals`] ->
//! runtime UI status projection.
//!
//! ## Ownership and lifetimes
//! Parsed values are owned structs to avoid borrowing from transient network
//! buffers.
//!
//! ## Error model
//! Invalid JSON or missing mandatory fields return [`AnalysisContractError`].
//!
//! ## Security and privacy notes
//! This crate processes only model outputs and risk metadata; it does not touch
//! authentication secrets.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical schema version for analysis response contract.
pub const ANALYSIS_SCHEMA_VERSION_V1: &str = "v1";

/// Parsed analysis response from protected API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResponse {
    /// Response schema version.
    pub schema_version: String,
    /// Request identifier for traceability.
    pub request_id: String,
    /// Individual model outputs.
    #[serde(default)]
    pub model_results: Vec<ModelResult>,
    /// Security/social-engineering risk categories.
    #[serde(default)]
    pub categories: Vec<CategoryAssessment>,
}

/// One model output emitted by analysis service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResult {
    /// Model identifier or name.
    pub model: String,
    /// Label/classification output.
    pub label: String,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f32,
}

/// One risk category score reported by analysis service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryAssessment {
    /// Category name (for example `credential_theft` or `social_engineering`).
    pub category: String,
    /// Severity score in [0, 100].
    pub severity: u8,
}

/// UI-safe risk level abstraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk.
    Low,
    /// Medium risk.
    Medium,
    /// High risk.
    High,
    /// Critical risk.
    Critical,
    /// Unknown/unsupported mapping.
    Unknown,
}

/// Risk signal projected for UI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiRiskSignal {
    /// Original category name from server response.
    pub category: String,
    /// Mapped risk level.
    pub level: RiskLevel,
}

/// Parses raw JSON into validated analysis response.
///
/// # Errors
/// Returns [`AnalysisContractError::Decode`] for invalid JSON.
/// Returns [`AnalysisContractError::InvalidContract`] when mandatory contract
/// fields are blank.
pub fn parse_analysis_response(raw: &str) -> Result<AnalysisResponse, AnalysisContractError> {
    let parsed: AnalysisResponse =
        serde_json::from_str(raw).map_err(AnalysisContractError::Decode)?;

    if parsed.schema_version.trim().is_empty() {
        return Err(AnalysisContractError::InvalidContract(
            "schema_version is empty".to_string(),
        ));
    }

    if parsed.request_id.trim().is_empty() {
        return Err(AnalysisContractError::InvalidContract(
            "request_id is empty".to_string(),
        ));
    }

    Ok(parsed)
}

/// Maps category severities into UI-safe risk signals.
///
/// Unknown category names are preserved with severity-based risk levels, so
/// newly introduced server categories do not crash client logic.
pub fn map_risk_signals(response: &AnalysisResponse) -> Vec<UiRiskSignal> {
    response
        .categories
        .iter()
        .map(|assessment| UiRiskSignal {
            category: assessment.category.clone(),
            level: severity_to_level(assessment.severity),
        })
        .collect()
}

fn severity_to_level(severity: u8) -> RiskLevel {
    match severity {
        0..=24 => RiskLevel::Low,
        25..=49 => RiskLevel::Medium,
        50..=79 => RiskLevel::High,
        80..=100 => RiskLevel::Critical,
        _ => RiskLevel::Unknown,
    }
}

/// Analysis contract errors.
#[derive(Debug, Error)]
pub enum AnalysisContractError {
    /// JSON decode failure.
    #[error("analysis decode failure: {0}")]
    Decode(#[from] serde_json::Error),
    /// Parsed payload violates contract invariants.
    #[error("analysis contract violation: {0}")]
    InvalidContract(String),
}

#[cfg(test)]
mod tests {
    //! Unit tests for response parsing and mapping.

    use super::*;

    #[test]
    fn preserves_unknown_categories() {
        let response = AnalysisResponse {
            schema_version: ANALYSIS_SCHEMA_VERSION_V1.to_string(),
            request_id: "req-1".to_string(),
            model_results: vec![],
            categories: vec![CategoryAssessment {
                category: "new_future_category".to_string(),
                severity: 30,
            }],
        };

        let signals = map_risk_signals(&response);
        assert_eq!(signals[0].category, "new_future_category");
        assert_eq!(signals[0].level, RiskLevel::Medium);
    }
}
