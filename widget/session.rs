//! Desktop detection and paths, computed exactly like the original's module
//! functions and `os.path` calls.

use std::ffi::{CStr, OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

/// Reads one environment variable (`std::env::var_os`, or a test's map).
type Environment<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn process_environment(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// `os.environ.get(name, '').lower()`.
fn lowered(environment: Environment, name: &str) -> String {
    environment(name)
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// `is_plasma_session()`: `KDE_SESSION_VERSION` set and non-empty, or
/// `plasma`/`kde` in `DESKTOP_SESSION` or `XDG_CURRENT_DESKTOP`.
pub fn is_plasma_session() -> bool {
    plasma_session_in(&process_environment)
}

fn plasma_session_in(environment: Environment) -> bool {
    let desktop_session = lowered(environment, "DESKTOP_SESSION");
    let xdg_current_desktop = lowered(environment, "XDG_CURRENT_DESKTOP");
    let kde_session = environment("KDE_SESSION_VERSION").unwrap_or_default();
    let plasma_indicators = ["plasma", "kde"];
    if !kde_session.is_empty() {
        return true;
    }
    plasma_indicators
        .iter()
        .any(|indicator| desktop_session.contains(indicator))
        || plasma_indicators
            .iter()
            .any(|indicator| xdg_current_desktop.contains(indicator))
}

/// `is_gnome_session()`: `gnome`, `ubuntu` or `pop` in `DESKTOP_SESSION`,
/// `XDG_CURRENT_DESKTOP` or `GDMSESSION`.
pub fn is_gnome_session() -> bool {
    gnome_session_in(&process_environment)
}

fn gnome_session_in(environment: Environment) -> bool {
    let gnome_indicators = ["gnome", "ubuntu", "pop"];
    ["DESKTOP_SESSION", "XDG_CURRENT_DESKTOP", "GDMSESSION"]
        .iter()
        .map(|name| lowered(environment, name))
        .any(|value| {
            gnome_indicators
                .iter()
                .any(|indicator| value.contains(indicator))
        })
}

/// `os.path.expanduser('~')`: `$HOME` (even when empty), else the password
/// database, without trailing slashes, `/` when that leaves nothing; `~`
/// unchanged when neither is available.
pub fn home_dir() -> OsString {
    home_dir_in(&process_environment)
}

fn home_dir_in(environment: Environment) -> OsString {
    let home = match environment("HOME") {
        Some(home) => home.into_vec(),
        None => match password_home() {
            Some(home) => home,
            None => return OsString::from("~"),
        },
    };
    let length = home.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1);
    if length == 0 {
        OsString::from("/")
    } else {
        OsString::from_vec(home[..length].to_vec())
    }
}

/// `pwd.getpwuid(os.getuid()).pw_dir`.
fn password_home() -> Option<Vec<u8>> {
    let mut buffer = vec![0 as libc::c_char; 16 * 1024];
    // SAFETY: zeroed plain C struct, filled by getpwuid_r.
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    // SAFETY: valid pointers to the struct, buffer and result.
    let status = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut entry,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() || entry.pw_dir.is_null() {
        return None;
    }
    // SAFETY: pw_dir points into `buffer`, NUL-terminated.
    Some(unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes().to_vec())
}

/// `os.path.join(base, name)` for a relative `name`.
pub fn join(base: &OsStr, name: &str) -> OsString {
    let mut joined = base.as_bytes().to_vec();
    if !joined.is_empty() && !joined.ends_with(b"/") {
        joined.push(b'/');
    }
    joined.extend_from_slice(name.as_bytes());
    OsString::from_vec(joined)
}

/// `os.path.exists(path)`.
pub fn exists(path: &OsStr) -> bool {
    std::fs::metadata(path).is_ok()
}

/// `os.access(path, os.X_OK)`.
pub fn is_executable(path: &OsStr) -> bool {
    let Ok(path) = std::ffi::CString::new(path.as_bytes()) else {
        return false;
    };
    // SAFETY: valid NUL-terminated path.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

/// `str(path)` for messages and `print`.
pub fn display(path: &OsStr) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs: Vec<(String, OsString)> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), OsString::from(value)))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn sessions_are_detected_like_the_original() {
        let plasma = |pairs: &[(&str, &str)]| plasma_session_in(&environment(pairs));
        let gnome = |pairs: &[(&str, &str)]| gnome_session_in(&environment(pairs));
        assert!(plasma(&[("KDE_SESSION_VERSION", "6")]));
        assert!(!plasma(&[("KDE_SESSION_VERSION", "")]));
        assert!(plasma(&[("XDG_CURRENT_DESKTOP", "KDE")]));
        assert!(plasma(&[(
            "DESKTOP_SESSION",
            "/usr/share/wayland-sessions/Plasma"
        )]));
        assert!(!plasma(&[
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("GDMSESSION", "kde")
        ]));
        assert!(!plasma(&[]));
        assert!(gnome(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]));
        assert!(gnome(&[("GDMSESSION", "Pop")]));
        assert!(gnome(&[("DESKTOP_SESSION", "gnome-xorg")]));
        assert!(!gnome(&[("XDG_CURRENT_DESKTOP", "LineXinBar")]));
        // Both match: the widget asks is_plasma_session() first.
        assert!(plasma(&[("XDG_CURRENT_DESKTOP", "KDE:GNOME")]));
    }

    #[test]
    fn home_follows_expanduser() {
        let home = |pairs: &[(&str, &str)]| home_dir_in(&environment(pairs));
        assert_eq!(home(&[("HOME", "/home/a")]), "/home/a");
        assert_eq!(home(&[("HOME", "/home/a//")]), "/home/a");
        assert_eq!(home(&[("HOME", "")]), "/");
        assert_eq!(home(&[("HOME", "/")]), "/");
        // Without HOME: the password database.
        assert!(!home(&[]).is_empty());
    }

    #[test]
    fn join_follows_os_path_join() {
        assert_eq!(
            join(OsStr::new("/home/a"), ".local/x/"),
            "/home/a/.local/x/"
        );
        assert_eq!(join(OsStr::new("/"), ".local/x/"), "/.local/x/");
        assert_eq!(join(OsStr::new("/base/"), "style1.sh"), "/base/style1.sh");
        assert_eq!(join(OsStr::new(""), "a"), "a");
    }

    #[test]
    fn executable_check_uses_access() {
        assert!(is_executable(OsStr::new("/bin/sh")));
        assert!(!is_executable(OsStr::new("/etc/hostname-does-not-exist")));
        assert!(exists(OsStr::new("/")));
        assert!(!exists(OsStr::new("/nonexistent/linexin")));
    }
}
