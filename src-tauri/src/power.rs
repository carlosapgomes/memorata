//! Power management module for preventing system sleep/screensaver.
//!
//! On Windows: Uses the Thread Execution State API to prevent idle sleep.
//! On other platforms: No-op implementation for cross-platform compatibility.
//!
//! Wake lock is active when the session is in any of these states:
//! - `recording`: Audio is being captured
//! - `paused`: Session is paused but may resume
//! - `processing`: Audio is being transcribed

use log::debug;

#[cfg(target_os = "windows")]
use log::warn;

#[cfg(target_os = "windows")]
use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};

/// Determines if the system should be kept awake based on session state flags.
///
/// This is a pure function for easy unit testing.
///
/// # Arguments
/// * `recording` - True if session is actively recording audio
/// * `paused` - True if session is paused
/// * `processing` - True if audio is being transcribed
///
/// # Returns
/// True if wake lock should be active
#[inline]
pub fn should_keep_awake(recording: bool, paused: bool, processing: bool) -> bool {
    recording || paused || processing
}

/// Updates the system wake lock based on current session state.
///
/// On Windows: Prevents sleep/screensaver by setting thread execution state.
/// On other platforms: No-op for cross-platform compatibility.
///
/// # Arguments
/// * `recording` - True if session is actively recording audio
/// * `paused` - True if session is paused
/// * `processing` - True if audio is being transcribed
pub fn update_keep_awake(recording: bool, paused: bool, processing: bool) {
    #[cfg(target_os = "windows")]
    {
        update_keep_awake_windows(should_keep_awake(recording, paused, processing));
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Use should_keep_awake for consistency (no-op on non-Windows)
        let _ = should_keep_awake(recording, paused, processing);
        debug!("Wake lock: no-op on this platform");
    }
}

/// Internal Windows implementation of wake lock.
/// Must not be called directly; use `update_keep_awake` instead.
#[cfg(target_os = "windows")]
fn update_keep_awake_windows(should_be_active: bool) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CURRENT_STATE: AtomicBool = AtomicBool::new(false);

    // Early exit if state hasn't changed
    if CURRENT_STATE.load(Ordering::Relaxed) == should_be_active {
        return;
    }

    if should_be_active {
        // Request wake lock: prevent system and display sleep
        // ES_CONTINUOUS keeps the state active until explicitly cleared
        // ES_SYSTEM_REQUIRED prevents system idle sleep
        // ES_DISPLAY_REQUIRED prevents display sleep
        let result = unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)
        };

        if result.is_ok() {
            CURRENT_STATE.store(true, Ordering::Relaxed);
            debug!("Wake lock: activated (preventing sleep/screensaver)");
        } else {
            warn!("Wake lock: failed to activate (error during SetThreadExecutionState)");
        }
    } else {
        // Clear wake lock by setting only ES_CONTINUOUS
        // This restores normal idle behavior
        let result = unsafe {
            SetThreadExecutionState(ES_CONTINUOUS)
        };

        if result.is_ok() {
            CURRENT_STATE.store(false, Ordering::Relaxed);
            debug!("Wake lock: deactivated (normal idle behavior restored)");
        } else {
            warn!("Wake lock: failed to deactivate (error during SetThreadExecutionState)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_keep_awake_all_states() {
        assert!(should_keep_awake(true, false, false), "Should keep awake when recording");
        assert!(should_keep_awake(false, true, false), "Should keep awake when paused");
        assert!(should_keep_awake(false, false, true), "Should keep awake when processing");
    }

    #[test]
    fn test_should_keep_awake_idle() {
        assert!(!should_keep_awake(false, false, false), "Should not keep awake when idle");
    }

    #[test]
    fn test_should_keep_awake_combined_states() {
        assert!(should_keep_awake(true, true, true), "Should keep awake with all states active");
        assert!(should_keep_awake(true, true, false), "Should keep awake with recording and paused");
        assert!(should_keep_awake(true, false, true), "Should keep awake with recording and processing");
        assert!(should_keep_awake(false, true, true), "Should keep awake with paused and processing");
    }

    #[test]
    fn test_should_keep_awake_pure_function() {
        // Pure function test: same inputs always produce same outputs
        let result1 = should_keep_awake(true, false, false);
        let result2 = should_keep_awake(true, false, false);
        assert_eq!(result1, result2, "Pure function should return consistent results");
    }
}