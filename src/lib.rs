//! `kp`: Kaiser Permanente health-plan claims and coverage through Kaiser's
//! Patient Access API, as the `health-records/v1` profile (medical-common).
//! `docs/api.md` has the API, its sources and its traps.

pub mod cli;
pub mod commands;
pub mod config;
pub mod credentials;
pub mod export;
pub mod fsutil;
pub mod kaiser;
pub mod session;
pub mod state;
