//! Startup defaults for the GPU back end's performance modes.
//!
//! The compatibility path waits for each submission, prepares the next one,
//! and copies shader results back on one thread. It also reports a 4K display
//! to the title. The world therefore spends hundreds of milliseconds per
//! frame on a desktop GPU. The presets here turn on the scheduling and memory
//! paths that are already implemented and tested in `victory-gpu`, and choose
//! an internal resolution that makes a 30 FPS target plausible.
//!
//! An explicit `VICTORY_*` environment variable is never overwritten: every
//! switch remains available for diagnosis and tuning. The preset only fills
//! in values that were not chosen by the user.

use crate::Config;

/// Selects a performance profile (`VICTORY_PERFORMANCE` or `--performance`).
pub const PROFILE_ENV: &str = "VICTORY_PERFORMANCE";
/// The display size reported to the title.
pub const DISPLAY_MODE_ENV: &str = "VICTORY_DISPLAY_MODE";
/// The frame rate the fast profiles are tuned for.
pub const TARGET_FPS: u32 = 30;

/// A bundle of GPU back-end switches that share one correctness/quality
/// trade-off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    /// The old, conservative path: 4K, synchronous submissions, no presets.
    Compatibility,
    /// Asynchronous submissions and parallel CPU work at 1080p.
    Balanced,
    /// The 30 FPS target: the balanced switches plus 720p and more GPU-local
    /// copies for the title's vertex/index/constant windows.
    #[default]
    Performance,
}

impl Profile {
    /// Parses a profile name, including a few convenient aliases.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "compatibility" | "compat" | "legacy" | "off" | "0" => Some(Self::Compatibility),
            "balanced" | "quality" => Some(Self::Balanced),
            "performance" | "fast" | "30" | "30fps" => Some(Self::Performance),
            _ => None,
        }
    }

    /// The canonical name, also written back to [`PROFILE_ENV`].
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatibility => "compatibility",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }
}

type Switch = (&'static str, &'static str);

/// Switches shared by the two fast profiles.
///
/// `VICTORY_RELAY=2` implies late-wait stage 3 and its copy-back worker. The
/// `WIDE_*` switches move snapshots, refreshes, comparisons and merges off
/// the submitting thread. The remaining switches batch small host/GPU steps
/// that otherwise happen thousands of times per frame.
const COMMON: &[Switch] = &[
    ("VICTORY_RELAY", "2"),
    ("VICTORY_LATE_WAIT", "3"),
    ("VICTORY_WIDE_BACK", "1"),
    ("VICTORY_WIDE_REFRESH", "1"),
    ("VICTORY_WIDE_MERGE", "1"),
    ("VICTORY_CARD_REHASH_RUNS", "1"),
    ("VICTORY_JOIN_COPIES", "1"),
    ("VICTORY_INDIRECT_WINDOW", "1"),
    ("VICTORY_TEXTURE_NO_WAIT", "1"),
    ("VICTORY_PIPELINE_WAIT_MS", "24"),
    ("VICTORY_CARD_BUFFERS_MIN_MB", "1"),
    ("VICTORY_CARD_BUFFERS_MB", "1024"),
];

/// A 1080p compromise; a quarter of 4K's pixels.
const BALANCED_EXTRA: &[Switch] = &[
    (DISPLAY_MODE_ENV, "1920x1080"),
    ("VICTORY_CARD_WINDOWS_MB", "256"),
];

/// 720p is one ninth of 4K's pixels. The card-window limit is checked
/// against the adapter's video-memory budget by `victory-gpu` before a copy
/// is placed, so this is a ceiling rather than an unconditional allocation.
const PERFORMANCE_EXTRA: &[Switch] = &[
    (DISPLAY_MODE_ENV, "1280x720"),
    ("VICTORY_CARD_WINDOWS_MB", "512"),
];

/// Defaults for `profile`, before explicit environment variables are
/// considered.
pub fn defaults(profile: Profile) -> Vec<Switch> {
    let mut out = COMMON.to_vec();
    match profile {
        Profile::Compatibility => {}
        Profile::Balanced => out.extend_from_slice(BALANCED_EXTRA),
        Profile::Performance => out.extend_from_slice(PERFORMANCE_EXTRA),
    }
    out
}

/// Whether `value` is a `WIDTHxHEIGHT` mode the DXGI layer accepts.
pub fn valid_display_mode(value: &str) -> bool {
    let Some((width, height)) = value.trim().split_once(['x', 'X']) else {
        return false;
    };
    match (
        width.trim().parse::<u32>(),
        height.trim().parse::<u32>(),
    ) {
        (Ok(w), Ok(h)) => (640..=7680).contains(&w) && (360..=4320).contains(&h),
        _ => false,
    }
}

/// The environment changes a profile would make. `present` says whether the
/// user already chose a variable. A configured display mode wins over the
/// profile's mode, but never over an explicit environment variable.
pub fn overrides(
    profile: Profile,
    configured_mode: Option<&str>,
    present: impl Fn(&str) -> bool,
) -> Vec<(&'static str, String)> {
    let configured_mode = configured_mode.filter(|mode| valid_display_mode(mode));
    // Canonicalise the profile name for relaunched child processes and for
    // `victory config`, even when an alias such as `fast` was used.
    let mut out = vec![(PROFILE_ENV, profile.as_str().to_owned())];
    for (key, value) in defaults(profile) {
        if present(key) {
            continue;
        }
        let value = if key == DISPLAY_MODE_ENV {
            configured_mode.unwrap_or(value)
        } else {
            value
        };
        out.push((key, value.to_owned()));
    }
    // Compatibility has no defaults, but an explicit resolution in
    // victory.toml is still a user choice and should be honoured.
    if let Some(mode) = configured_mode
        && !present(DISPLAY_MODE_ENV)
        && !out.iter().any(|(key, _)| *key == DISPLAY_MODE_ENV)
    {
        out.push((DISPLAY_MODE_ENV, mode.to_owned()));
    }
    out
}

/// Applies `explicit` (from the CLI), else the configured profile, else the
/// environment profile, else the 30 FPS target profile.
///
/// # Safety
/// This changes the process environment and must be called once from `main`
/// before any other thread has been started. `victory` does so before logging
/// and before loading or starting the title.
pub unsafe fn apply(explicit: Option<Profile>) -> Profile {
    let config = Config::load().ok();
    let configured = config
        .as_ref()
        .and_then(|c| c.performance.profile.as_deref())
        .and_then(Profile::parse);
    let environment = std::env::var(PROFILE_ENV)
        .ok()
        .and_then(|value| Profile::parse(&value));
    let profile = explicit
        .or(configured)
        .or(environment)
        .unwrap_or_default();
    let configured_mode = config
        .as_ref()
        .and_then(|c| c.performance.display_mode.as_deref());
    let changes = overrides(profile, configured_mode, |key| {
        std::env::var_os(key).is_some()
    });
    for (key, value) in changes {
        // SAFETY: guaranteed by this function's caller: no other thread can
        // read or write the environment while these variables are set.
        unsafe { std::env::set_var(key, value) };
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(changes: &'a [(&'static str, String)], key: &str) -> Option<&'a str> {
        changes
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn aliases_parse_and_unknown_values_fall_back() {
        assert_eq!(Profile::parse("off"), Some(Profile::Compatibility));
        assert_eq!(Profile::parse("legacy"), Some(Profile::Compatibility));
        assert_eq!(Profile::parse("quality"), Some(Profile::Balanced));
        assert_eq!(Profile::parse("30fps"), Some(Profile::Performance));
        assert_eq!(Profile::parse("nonsense"), None);
        assert_eq!(Profile::default(), Profile::Performance);
    }

    #[test]
    fn performance_defaults_enable_the_30_fps_path() {
        let changes = overrides(Profile::Performance, None, |_| false);
        assert_eq!(value(&changes, PROFILE_ENV), Some("performance"));
        assert_eq!(value(&changes, DISPLAY_MODE_ENV), Some("1280x720"));
        assert_eq!(value(&changes, "VICTORY_RELAY"), Some("2"));
        assert_eq!(value(&changes, "VICTORY_LATE_WAIT"), Some("3"));
        assert_eq!(value(&changes, "VICTORY_WIDE_BACK"), Some("1"));
        assert_eq!(value(&changes, "VICTORY_WIDE_REFRESH"), Some("1"));
        assert_eq!(value(&changes, "VICTORY_WIDE_MERGE"), Some("1"));
        assert_eq!(value(&changes, "VICTORY_CARD_WINDOWS_MB"), Some("512"));
        assert_eq!(TARGET_FPS, 30);
    }

    #[test]
    fn balanced_uses_a_higher_resolution() {
        let changes = overrides(Profile::Balanced, None, |_| false);
        assert_eq!(value(&changes, PROFILE_ENV), Some("balanced"));
        assert_eq!(value(&changes, DISPLAY_MODE_ENV), Some("1920x1080"));
        assert_eq!(value(&changes, "VICTORY_CARD_WINDOWS_MB"), Some("256"));
        assert_eq!(value(&changes, "VICTORY_RELAY"), Some("2"));
    }

    #[test]
    fn explicit_variables_and_configured_resolution_win() {
        let chosen = ["VICTORY_RELAY", DISPLAY_MODE_ENV];
        let changes = overrides(
            Profile::Performance,
            Some("1600x900"),
            |key| chosen.contains(&key),
        );
        assert_eq!(value(&changes, PROFILE_ENV), Some("performance"));
        assert_eq!(value(&changes, "VICTORY_RELAY"), None);
        assert_eq!(value(&changes, DISPLAY_MODE_ENV), None);
        assert_eq!(value(&changes, "VICTORY_LATE_WAIT"), Some("3"));

        let changed = overrides(Profile::Performance, Some("1600x900"), |_| false);
        assert_eq!(value(&changed, DISPLAY_MODE_ENV), Some("1600x900"));
    }

    #[test]
    fn compatibility_keeps_the_old_path_but_honours_a_resolution() {
        let none = overrides(Profile::Compatibility, None, |_| false);
        assert_eq!(none.len(), 1);
        assert_eq!(value(&none, PROFILE_ENV), Some("compatibility"));
        let changed = overrides(Profile::Compatibility, Some("1280x720"), |_| false);
        assert_eq!(value(&changed, DISPLAY_MODE_ENV), Some("1280x720"));
    }

    #[test]
    fn invalid_configured_resolutions_are_ignored() {
        assert!(valid_display_mode("1280x720"));
        assert!(valid_display_mode("1920X1080"));
        assert!(!valid_display_mode("12x12"));
        assert!(!valid_display_mode("not-a-mode"));
        let changes = overrides(Profile::Performance, Some("bad"), |_| false);
        assert_eq!(value(&changes, DISPLAY_MODE_ENV), Some("1280x720"));
    }
}
