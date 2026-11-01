//! Requirement parser, matcher and shared types. Pure logic, no I/O.

mod matcher;
mod parser;

pub use matcher::{
    AntiCheat, Blocker, Component, Confidence, GameRequirements, HardwareRef, Level, LinuxInfo,
    ProtonTier, Requirement, Specs, Vendor, Verdict, Warning, evaluate,
};
pub use parser::{ParsedTier, parse};
