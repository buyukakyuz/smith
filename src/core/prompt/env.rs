use chrono::Local;
use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    pub working_directory: PathBuf,
    pub is_git_repo: bool,
    pub platform: String,
    pub os_version: String,
    pub date: String,
    pub git_status: Option<String>,
    pub git_branch: Option<String>,
    pub git_main_branch: Option<String>,
    pub git_recent_commits: Option<String>,
}

impl EnvironmentInfo {
    #[must_use]
    pub fn collect() -> Self {
        let working_directory = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let is_git_repo = Self::check_git_repo(&working_directory);

        let (git_status, git_branch, git_main_branch, git_recent_commits) = if is_git_repo {
            (
                Self::get_git_status(),
                Self::get_git_branch(),
                Self::get_main_branch(),
                Self::get_recent_commits(),
            )
        } else {
            (None, None, None, None)
        };

        Self {
            working_directory,
            is_git_repo,
            platform: env::consts::OS.to_string(),
            os_version: Self::get_os_version(),
            date: Local::now().format("%Y-%m-%d").to_string(),
            git_status,
            git_branch,
            git_main_branch,
            git_recent_commits,
        }
    }

    fn check_git_repo(path: &PathBuf) -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(path)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_git_status() -> Option<String> {
        Command::new("git")
            .args(["status", "--short"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok()
                } else {
                    None
                }
            })
    }

    fn get_git_branch() -> Option<String> {
        Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }

    fn get_main_branch() -> Option<String> {
        let main_exists = Command::new("git")
            .args(["rev-parse", "--verify", "main"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if main_exists {
            return Some("main".to_string());
        }

        let master_exists = Command::new("git")
            .args(["rev-parse", "--verify", "master"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if master_exists {
            Some("master".to_string())
        } else {
            None
        }
    }

    fn get_recent_commits() -> Option<String> {
        Command::new("git")
            .args(["log", "--oneline", "-n", "5"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok()
                } else {
                    None
                }
            })
    }

    fn get_os_version() -> String {
        #[cfg(target_os = "macos")]
        {
            Command::new("uname")
                .args(["-s", "-r"])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map_or_else(|| "Unknown".to_string(), |s| s.trim().to_string())
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("uname")
                .args(["-s", "-r"])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", "ver"])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            "Unknown".to_string()
        }
    }

    #[must_use]
    pub fn format(&self) -> String {
        let mut env_str = format!(
            "Working directory: {}\n\
             Is directory a git repo: {}\n\
             Platform: {}\n\
             OS Version: {}\n\
             Today's date: {}",
            self.working_directory.display(),
            if self.is_git_repo { "Yes" } else { "No" },
            self.platform,
            self.os_version,
            self.date
        );

        if self.is_git_repo {
            if let Some(branch) = &self.git_branch {
                env_str.push_str(&format!("\nCurrent branch: {branch}"));
            }

            if let Some(main_branch) = &self.git_main_branch {
                env_str.push_str(&format!("\nMain branch: {main_branch}"));
            }
        }

        env_str
    }

    #[must_use]
    pub fn format_git_status(&self) -> Option<String> {
        if !self.is_git_repo {
            return None;
        }

        let mut git_info = String::new();

        git_info.push_str("gitStatus: This is the git status at the start of the conversation. Note that this status is a snapshot in time, and will not update during the conversation.\n");

        if let Some(branch) = &self.git_branch {
            git_info.push_str(&format!("Current branch: {branch}\n"));
        }

        if let Some(main_branch) = &self.git_main_branch {
            git_info.push_str(&format!(
                "\nMain branch (you will usually use this for PRs): {main_branch}\n"
            ));
        }

        if let Some(status) = &self.git_status {
            git_info.push_str(&format!("\nStatus:\n{status}"));
        }

        if let Some(commits) = &self.git_recent_commits {
            git_info.push_str(&format!("\nRecent commits:\n{commits}"));
        }

        Some(git_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_environment_info() {
        let env_info = EnvironmentInfo::collect();

        assert!(!env_info.platform.is_empty());
        assert!(!env_info.date.is_empty());
        assert!(env_info.working_directory.exists());
    }

    #[test]
    fn test_format_environment_info() {
        let env_info = EnvironmentInfo::collect();
        let formatted = env_info.format();

        assert!(formatted.contains("Working directory:"));
        assert!(formatted.contains("Platform:"));
        assert!(formatted.contains("Today's date:"));
    }
}
