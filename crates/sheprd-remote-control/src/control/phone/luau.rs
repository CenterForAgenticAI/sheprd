//! A Luau screen of the phone's app changed from this computer while it
//! runs (`sheprd phone luau`): the files an agent edits go to the phone,
//! which lays them over the screen's bundled ones and reloads, keeping its
//! state, and says what loading them found. Only a debug build of the app
//! takes them; a build from the App Store runs its bundled Luau alone.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The most bytes of Luau one push carries, all files together.
pub const MAX_LUAU_BYTES: usize = 192 * 1024;
/// The most files one push carries.
pub const MAX_LUAU_FILES: usize = 64;
/// The longest a Luau screen's name may be, in bytes.
pub const MAX_SCREEN_BYTES: usize = 64;
/// The most lines of each kind a phone reports, so its answer fits the
/// live socket.
pub const MAX_REPORT_LINES: usize = 12;

/// Whether `name` may name a Luau screen, as its folder is named in the
/// app's bundle (`pull-requests`).
pub fn valid_screen(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SCREEN_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Whether `path` may name one of a Luau screen's files: a `.luau` file
/// relative to the screen's folder, as `main.luau`, `style.luau`,
/// `components/row.luau` or `mods/status/main.luau`, never out of it.
pub fn valid_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    path.ends_with(".luau")
        && path.len() <= 200
        && parts.len() <= 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

/// Whether `files` may be pushed: within the limits, each named by a
/// [`valid_path`].
pub fn valid_files(files: &BTreeMap<String, String>) -> bool {
    files.len() <= MAX_LUAU_FILES
        && files
            .iter()
            .map(|(path, text)| path.len() + text.len())
            .sum::<usize>()
            <= MAX_LUAU_BYTES
        && files.keys().all(|path| valid_path(path))
}

/// What the phone found loading a push, each list at most
/// [`MAX_REPORT_LINES`] lines.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LuauReport {
    /// Whether the screen is open, so it redrew with the push; else the
    /// files were checked without drawing and wait for it to open.
    #[serde(default)]
    pub shown: bool,
    /// What failed to load or draw, by file: the screen keeps the last
    /// version that worked meanwhile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// What draws as a placeholder: a component or element the app lacks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// The last lines the screen's mods logged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}
