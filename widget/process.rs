//! The original's `subprocess` calls, with Python's observable behavior: the
//! child inherits stdin and the environment, descriptors above 2 are not
//! passed on (`close_fds=True`), text output is strict UTF-8 with universal
//! newlines, a signal death is a negative return code, and failures carry the
//! text Python's exception would have (`str(e)`).

use std::ffi::OsStr;
use std::io;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};

/// `subprocess.Popen(argv, ...)` without redirections.
pub fn command<S: AsRef<OsStr>>(argv: &[S]) -> Command {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    // SAFETY: the closure only makes a raw system call (async-signal-safe).
    unsafe {
        command.pre_exec(|| {
            // Python closes every descriptor above stderr in the child. Marking
            // them close-on-exec has the same effect on the new program and
            // keeps std's own exec-error pipe working. Failure (kernels older
            // than 5.11) leaves them as they are.
            libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC as libc::c_uint,
            );
            Ok(())
        });
    }
    command
}

/// Output of [`communicate`].
pub struct Completed {
    pub returncode: i32,
    #[allow(dead_code)] // decoded like the original, which ignores it
    pub stdout: String,
    pub stderr: String,
}

/// `Popen(argv, stdout=PIPE, stderr=PIPE, text=True).communicate()`.
pub fn communicate<S: AsRef<OsStr>>(argv: &[S]) -> Result<Completed, String> {
    let child = command(argv)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| spawn_error(&error, argv[0].as_ref()))?;
    let output = child.wait_with_output().map_err(|error| os_error(&error))?;
    let returncode = returncode(output.status);
    // Python decodes stdout first.
    let stdout = decode(output.stdout)?;
    let stderr = decode(output.stderr)?;
    Ok(Completed {
        returncode,
        stdout,
        stderr,
    })
}

/// `subprocess.run(argv, check=False)`: inherited stdio, waits for the exit.
pub fn run<S: AsRef<OsStr>>(argv: &[S]) -> io::Result<ExitStatus> {
    command(argv).status()
}

/// `subprocess.Popen(argv)`: inherited stdio, does not wait.
pub fn spawn<S: AsRef<OsStr>>(argv: &[S]) -> io::Result<Child> {
    command(argv).spawn()
}

/// `Popen.returncode`: the exit status, or minus the signal that killed it.
pub fn returncode(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0))
}

/// `str(OSError)` without a file name: `[Errno 2] No such file or directory`.
fn os_error(error: &io::Error) -> String {
    match error.raw_os_error() {
        Some(code) => format!("[Errno {code}] {}", strerror(code)),
        None => error.to_string(),
    }
}

/// `str(e)` of the `OSError` Popen raises when the program cannot be
/// executed: `[Errno 2] No such file or directory: '/bin/bash'`.
fn spawn_error(error: &io::Error, program: &OsStr) -> String {
    match error.raw_os_error() {
        Some(_) => format!(
            "{}: {}",
            os_error(error),
            python_repr(&program.to_string_lossy())
        ),
        None => error.to_string(),
    }
}

/// `os.strerror(code)`, which is the C library's message for the locale.
fn strerror(code: i32) -> String {
    let text = io::Error::from_raw_os_error(code).to_string();
    let suffix = format!(" (os error {code})");
    text.strip_suffix(&suffix).unwrap_or(&text).to_owned()
}

/// `repr()` of a printable str, as used in OSError messages.
fn python_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut repr = String::with_capacity(text.len() + 2);
    repr.push(quote);
    for c in text.chars() {
        match c {
            '\\' => repr.push_str("\\\\"),
            '\n' => repr.push_str("\\n"),
            '\r' => repr.push_str("\\r"),
            '\t' => repr.push_str("\\t"),
            c if c == quote => {
                repr.push('\\');
                repr.push(c);
            }
            c if c.is_control() => repr.push_str(&format!("\\x{:02x}", c as u32)),
            c => repr.push(c),
        }
    }
    repr.push(quote);
    repr
}

/// Text-mode decoding of a pipe: strict UTF-8 (the original runs with a
/// UTF-8 locale or in Python's UTF-8 mode), then `\r\n` and `\r` become `\n`.
/// The error is `str()` of Python's `UnicodeDecodeError`.
pub fn decode(bytes: Vec<u8>) -> Result<String, String> {
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text.replace("\r\n", "\n").replace('\r', "\n")),
        Err(error) => {
            let bytes = error.as_bytes();
            let utf8 = error.utf8_error();
            let start = utf8.valid_up_to();
            let (end, reason) = match utf8.error_len() {
                None => (bytes.len(), "unexpected end of data"),
                Some(length) => {
                    let lead = bytes[start];
                    let invalid_start = (0x80..=0xC1).contains(&lead) || lead >= 0xF5;
                    let reason = if invalid_start {
                        "invalid start byte"
                    } else {
                        "invalid continuation byte"
                    };
                    (start + length, reason)
                }
            };
            Err(if end == start + 1 {
                format!(
                    "'utf-8' codec can't decode byte 0x{:02x} in position {start}: {reason}",
                    bytes[start]
                )
            } else {
                format!(
                    "'utf-8' codec can't decode bytes in position {start}-{}: {reason}",
                    end - 1
                )
            })
        }
    }
}

/// Python's `str.strip()` (whitespace as `str.isspace()` defines it).
pub fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || ('\x1c'..='\x1f').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_matches_python() {
        assert_eq!(decode(b"a\r\nb\rc\n".to_vec()).unwrap(), "a\nb\nc\n");
        assert_eq!(
            decode(b"ok\xff".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode byte 0xff in position 2: invalid start byte"
        );
        assert_eq!(
            decode(b"\xe2\x82x".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode bytes in position 0-1: invalid continuation byte"
        );
        assert_eq!(
            decode(b"ab\xe2\x82".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode bytes in position 2-3: unexpected end of data"
        );
        assert_eq!(
            decode(b"\xe0\x80".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode byte 0xe0 in position 0: invalid continuation byte"
        );
        assert_eq!(
            decode(b"\xc3".to_vec()).unwrap_err(),
            "'utf-8' codec can't decode byte 0xc3 in position 0: unexpected end of data"
        );
    }

    #[test]
    fn strip_matches_python() {
        assert_eq!(strip(" \t\x1f err \n\u{a0}"), "err");
        assert_eq!(strip(""), "");
    }

    #[test]
    fn spawn_errors_read_like_python() {
        let error = communicate(&["/nonexistent/linexin-test"]).err().unwrap();
        assert!(error.starts_with("[Errno 2] "), "{error}");
        assert!(error.ends_with(": '/nonexistent/linexin-test'"), "{error}");
        assert_eq!(python_repr("it's"), "\"it's\"");
    }

    #[test]
    fn communicate_collects_status_and_text() {
        let done = communicate(&[
            "/bin/sh",
            "-c",
            "printf 'out'; printf ' e\\r\\n' >&2; exit 3",
        ])
        .unwrap();
        assert_eq!(done.returncode, 3);
        assert_eq!(done.stdout, "out");
        assert_eq!(strip(&done.stderr), "e");
        let killed = communicate(&["/bin/sh", "-c", "kill -9 $$"]).unwrap();
        assert_eq!(killed.returncode, -9);
    }
}
