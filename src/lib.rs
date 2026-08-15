//! Goglz library crate.
//!
//! This exposes the internals of the `goglz` binary (config loading, directory
//! monitoring, document processing, and the `revise` pipeline) as a library so
//! that integration tests (and any future embedders) can exercise real
//! behavior without going through the CLI/daemon process.
pub mod ai_client;
pub mod config;
pub mod error;
pub mod monitor;
pub mod processor;
pub mod revise;
