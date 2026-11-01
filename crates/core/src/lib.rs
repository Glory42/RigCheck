//! Requirement parser, matcher and shared types. Pure logic, no I/O.

mod matcher;

pub use matcher::{
    AntiCheat, Blocker, Component, Confidence, GameRequirements, HardwareRef, Level, LinuxInfo,
    ProtonTier, Requirement, Specs, Vendor, Verdict, Warning, evaluate,
};
