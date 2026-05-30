pub mod cli;
pub mod exec;
pub mod manifest;
pub mod server;
pub mod url;

pub const URL_BANNER_PREFIX: &str = "NATS_URL=";
pub const READY_SENTINEL: &str = "NATS_FIXTURE_READY";

pub mod exit_codes {
    pub const OK: i32 = 0;
    pub const MANIFEST_INVALID: i32 = 1;
    pub const PORT_IN_USE: i32 = 2;
    pub const SERVER_FAILED: i32 = 3;
    pub const APPLY_FAILED: i32 = 4;
}
