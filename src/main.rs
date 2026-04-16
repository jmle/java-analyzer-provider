// Java Analyzer Provider
// Self-contained Rust-based Java analyzer for Konveyor

mod provider;
mod java_graph;
mod buildtool;
mod dependency;
mod filter;

#[allow(clippy::all)]
mod analyzer_service {
    include!("analyzer_service/provider.rs");
}

use provider::java::JavaProvider;
use analyzer_service::{
    provider_service_server::ProviderServiceServer,
    provider_code_location_service_server::ProviderCodeLocationServiceServer,
};
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;
use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Java Analyzer Provider starting...");
    info!("Status: Phase 2 - Task 2.6: gRPC Interface");

    // Parse command line arguments for the gRPC port
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].parse::<u16>().unwrap_or(9000)
    } else {
        9000
    };

    let addr = format!("0.0.0.0:{}", port).parse()?;
    info!("Starting gRPC server on {}", addr);

    // Create provider instances with shared state
    let java_provider1 = JavaProvider::new();
    let shared_state = java_provider1.get_shared_state();
    let java_provider2 = JavaProvider::new_with_shared_state(shared_state);

    // Load file descriptor set for reflection
    let file_descriptor_set = include_bytes!("analyzer_service/provider_service_descriptor.bin");
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(file_descriptor_set)
        .build_v1()
        .map_err(|e| anyhow::anyhow!("Failed to create reflection service: {}", e))?;

    // Start the gRPC server with all services
    match Server::builder()
        .add_service(ProviderServiceServer::new(java_provider1))
        .add_service(ProviderCodeLocationServiceServer::new(java_provider2))
        .add_service(reflection_service)
        .serve(addr)
        .await
    {
        Ok(_) => info!("gRPC server stopped gracefully"),
        Err(e) => error!("gRPC server error: {}", e),
    }

    Ok(())
}
