pub const GLOB_DEFAULT_LIMIT: usize = 100;
pub const GLOB_MAX_LIMIT: usize = 1000;

pub const GREP_DEFAULT_LIMIT: usize = 50;
pub const GREP_MAX_LIMIT: usize = 500;
pub const GREP_MAX_CONTEXT: usize = 5;

pub const READ_DEFAULT_OFFSET: usize = 1;
pub const READ_DEFAULT_LIMIT: usize = 2000;
pub const READ_MAX_LIMIT: usize = 10_000;
pub const READ_MAX_LINE_LENGTH: usize = 500;
pub const READ_BINARY_CHECK_SIZE: usize = 8192;

pub const LIST_MAX_DEPTH: usize = 5;

pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
pub const MAX_WRITE_SIZE: usize = 10 * 1024 * 1024;

pub const fn default_respect_gitignore() -> bool {
    true
}
