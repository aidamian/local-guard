#![warn(missing_docs)]
//! # local-guard-capture
//!
//! ## Purpose
//! Provides display enumeration and frame acquisition abstractions.
//!
//! ## Responsibilities
//! - Define a backend-agnostic capture trait.
//! - Expose real display capture on supported platforms.
//! - Expose deterministic synthetic capture for CI and unit tests.
//! - Provide FPS scheduling helpers used by the app orchestrator.
//!
//! ## Data flow
//! App selects a display -> backend captures [`local_guard_core::Frame`] at
//! configured cadence -> frames enter batch/mosaic pipeline.
//!
//! ## Ownership and lifetimes
//! Captured frames are owned values with independent buffers; no borrowed frame
//! memory escapes backend boundaries.
//!
//! ## Error model
//! Invalid FPS, unknown displays, and backend failures are reported as
//! [`CaptureError`] values.
//!
//! ## Security and privacy notes
//! Capture backends must avoid persisting raw frame bytes to disk for MVP.

use std::sync::Mutex;

use local_guard_core::Frame;
use thiserror::Error;

/// Metadata describing one available display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Stable display identifier.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Native display width in pixels.
    pub width: u32,
    /// Native display height in pixels.
    pub height: u32,
}

/// Capture configuration used by schedulers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    /// Frames captured per second.
    pub fps: u32,
}

impl CaptureConfig {
    /// Creates validated capture configuration.
    ///
    /// # Errors
    /// Returns [`CaptureError::InvalidFps`] when `fps == 0`.
    pub fn new(fps: u32) -> Result<Self, CaptureError> {
        if fps == 0 {
            return Err(CaptureError::InvalidFps);
        }
        Ok(Self { fps })
    }

    /// Returns capture interval in milliseconds.
    pub fn interval_ms(&self) -> u64 {
        1_000 / self.fps as u64
    }
}

/// Trait implemented by concrete capture providers.
pub trait CaptureBackend: Send + Sync {
    /// Enumerates available displays.
    fn list_displays(&self) -> Vec<DisplayInfo>;

    /// Captures one frame from selected display.
    ///
    /// # Errors
    /// Returns [`CaptureError::UnknownDisplay`] when display id is invalid.
    fn capture_frame(&self, display_id: &str, captured_at_ms: u64) -> Result<Frame, CaptureError>;
}

/// Real display capture backend for supported desktop targets.
///
/// # Notes
/// The backend snapshots display metadata at initialization and reacquires
/// current screen handles for each capture call.
#[derive(Debug, Clone)]
pub struct RealCaptureBackend {
    displays: Vec<RealDisplayRecord>,
}

#[derive(Debug, Clone)]
struct RealDisplayRecord {
    #[cfg(windows)]
    index: usize,
    info: DisplayInfo,
}

impl RealCaptureBackend {
    /// Discovers currently available displays and creates a real capture backend.
    ///
    /// # Errors
    /// Returns [`CaptureError::Backend`] when display enumeration fails or no
    /// displays are available.
    pub fn discover() -> Result<Self, CaptureError> {
        #[cfg(windows)]
        {
            use screenshots::Screen;

            let screens = Screen::all().map_err(|error| {
                CaptureError::Backend(format!("screen enumeration failed: {error}"))
            })?;

            if screens.is_empty() {
                return Err(CaptureError::Backend(
                    "no displays were reported by the OS".to_string(),
                ));
            }

            let mut displays = Vec::with_capacity(screens.len());
            for (index, screen) in screens.into_iter().enumerate() {
                let width = screen.display_info.width.max(1) as u32;
                let height = screen.display_info.height.max(1) as u32;
                displays.push(RealDisplayRecord {
                    #[cfg(windows)]
                    index,
                    info: DisplayInfo {
                        id: format!("real-display-{index}"),
                        name: format!("Display {}", index + 1),
                        width,
                        height,
                    },
                });
            }

            Ok(Self { displays })
        }

        #[cfg(not(windows))]
        {
            Err(CaptureError::Backend(
                "real capture backend is currently implemented for Windows only".to_string(),
            ))
        }
    }
}

impl CaptureBackend for RealCaptureBackend {
    fn list_displays(&self) -> Vec<DisplayInfo> {
        self.displays
            .iter()
            .map(|record| record.info.clone())
            .collect()
    }

    fn capture_frame(&self, display_id: &str, captured_at_ms: u64) -> Result<Frame, CaptureError> {
        let record = self
            .displays
            .iter()
            .find(|record| record.info.id == display_id)
            .ok_or_else(|| CaptureError::UnknownDisplay(display_id.to_string()))?;

        #[cfg(windows)]
        {
            use screenshots::Screen;

            let screens = Screen::all().map_err(|error| {
                CaptureError::Backend(format!("screen refresh failed: {error}"))
            })?;
            let screen = screens.get(record.index).ok_or_else(|| {
                CaptureError::Backend(format!(
                    "display index {} is not available anymore",
                    record.index
                ))
            })?;

            let captured = screen.capture().map_err(|error| {
                CaptureError::Backend(format!("screen capture failed: {error}"))
            })?;
            let width = captured.width();
            let height = captured.height();
            let rgba = captured.into_raw();

            return Frame::new(record.info.id.clone(), width, height, captured_at_ms, rgba)
                .map_err(|error| CaptureError::Backend(error.to_string()));
        }

        #[cfg(not(windows))]
        {
            let _ = record;
            let _ = captured_at_ms;
            Err(CaptureError::Backend(
                "real capture backend is currently implemented for Windows only".to_string(),
            ))
        }
    }
}

/// Deterministic synthetic backend for test and CI usage.
#[derive(Debug)]
pub struct SyntheticCaptureBackend {
    displays: Vec<DisplayInfo>,
    sequence: Mutex<u64>,
}

impl SyntheticCaptureBackend {
    /// Creates synthetic backend with one default display.
    pub fn new() -> Self {
        Self {
            displays: vec![DisplayInfo {
                id: "display-1".to_string(),
                name: "Synthetic Display".to_string(),
                width: 4,
                height: 4,
            }],
            sequence: Mutex::new(0),
        }
    }

    /// Creates backend with caller-provided display list.
    pub fn with_displays(displays: Vec<DisplayInfo>) -> Self {
        Self {
            displays,
            sequence: Mutex::new(0),
        }
    }
}

impl Default for SyntheticCaptureBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureBackend for SyntheticCaptureBackend {
    fn list_displays(&self) -> Vec<DisplayInfo> {
        self.displays.clone()
    }

    fn capture_frame(&self, display_id: &str, captured_at_ms: u64) -> Result<Frame, CaptureError> {
        let display = self
            .displays
            .iter()
            .find(|display| display.id == display_id)
            .ok_or_else(|| CaptureError::UnknownDisplay(display_id.to_string()))?;

        let mut sequence = self
            .sequence
            .lock()
            .map_err(|_| CaptureError::Backend("synthetic sequence lock poisoned".to_string()))?;
        *sequence += 1;

        let byte = (*sequence % 255) as u8;
        let rgba_len = (display.width as usize) * (display.height as usize) * 4;
        let rgba = vec![byte; rgba_len];

        Frame::new(
            display.id.clone(),
            display.width,
            display.height,
            captured_at_ms,
            rgba,
        )
        .map_err(|error| CaptureError::Backend(error.to_string()))
    }
}

/// Computes deterministic schedule timestamps for fixed-FPS capture.
///
/// # Returns
/// Vector of `count` timestamps starting at `start_ms` with `interval_ms` spacing.
pub fn scheduled_capture_times(config: CaptureConfig, start_ms: u64, count: usize) -> Vec<u64> {
    let interval = config.interval_ms();
    (0..count)
        .map(|index| start_ms.saturating_add(interval.saturating_mul(index as u64)))
        .collect()
}

/// Capture layer error type.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// FPS must be positive.
    #[error("invalid fps: must be greater than zero")]
    InvalidFps,
    /// Requested display is unknown to backend.
    #[error("unknown display: {0}")]
    UnknownDisplay(String),
    /// Backend runtime failure.
    #[error("capture backend failure: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    //! Unit tests for synthetic capture behavior.

    use super::*;

    #[test]
    fn synthetic_backend_generates_frames() {
        let backend = SyntheticCaptureBackend::new();
        let frame = backend
            .capture_frame("display-1", 42)
            .expect("capture should work");
        assert_eq!(frame.width, 4);
        assert_eq!(frame.height, 4);
        assert_eq!(frame.captured_at_ms, 42);
    }
}
