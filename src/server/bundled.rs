use std::path::{Path, PathBuf};
use std::process::Command;

pub fn binary_filename() -> &'static str {
    if cfg!(windows) {
        "nats-server.exe"
    } else {
        "nats-server"
    }
}

pub fn locate_bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(binary_filename());
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

pub fn locate_system() -> Option<PathBuf> {
    which::which(binary_filename().trim_end_matches(".exe")).ok()
}

#[derive(Debug, Clone, Copy)]
pub enum Resolution {
    BundledFirst,
    SystemFirst,
}

pub fn resolve(
    override_path: Option<&Path>,
    strategy: Resolution,
) -> Result<PathBuf, ResolveError> {
    if let Some(p) = override_path {
        if !p.is_file() {
            return Err(ResolveError::OverrideMissing(p.to_path_buf()));
        }
        return Ok(p.to_path_buf());
    }
    match strategy {
        Resolution::BundledFirst => {
            if let Some(p) = locate_bundled() {
                return Ok(p);
            }
            locate_system().ok_or(ResolveError::NotFound)
        }
        Resolution::SystemFirst => {
            if let Some(p) = locate_system() {
                return Ok(p);
            }
            locate_bundled().ok_or(ResolveError::NotFound)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("--nats-server '{0}' is not a file")]
    OverrideMissing(PathBuf),
    #[error("nats-server binary not found. Bundled binary missing alongside the hyerix-natsfixture executable and no 'nats-server' on PATH.")]
    NotFound,
}

pub fn detect_nats_server_version() -> Option<String> {
    let path = resolve(None, Resolution::BundledFirst).ok()?;
    let output = Command::new(&path).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    parse_version_line(&combined)
}

fn parse_version_line(s: &str) -> Option<String> {
    for line in s.lines() {
        for tok in line.split_whitespace() {
            let candidate = tok.trim_start_matches('v').trim_end_matches([',', '.']);
            if candidate.contains('.')
                && candidate.starts_with(|c: char| c.is_ascii_digit())
                && candidate
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_filename_matches_platform() {
        if cfg!(windows) {
            assert_eq!(binary_filename(), "nats-server.exe");
        } else {
            assert_eq!(binary_filename(), "nats-server");
        }
    }

    #[test]
    fn parse_version_handles_standard_output() {
        let out = "nats-server: v2.10.20\n";
        assert_eq!(parse_version_line(out).as_deref(), Some("2.10.20"));
    }
}
