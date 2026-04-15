// Java Analyzer Provider Library
// Self-contained Rust-based Java analyzer for Konveyor

pub mod provider;
pub mod java_graph;
pub mod buildtool;
pub mod dependency;
pub mod filter;

#[allow(clippy::all)]
pub mod analyzer_service {
    include!("analyzer_service/provider.rs");
}
