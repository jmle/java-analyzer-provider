fn main() {
    #[cfg(feature = "generate-proto")]
    {
        // Always download protoc to ensure we have the include files
        println!("Downloading protoc with includes...");
        dlprotoc::download_protoc().unwrap();

        tonic_build::configure()
            .out_dir("src/analyzer_service/")
            .build_client(true)
            .file_descriptor_set_path("src/analyzer_service/provider_service_descriptor.bin")
            .compile_well_known_types(true)
            .compile_protos(
                &["src/build/proto/provider.proto"],
                &["src/build/proto/"],
            )
            .unwrap();
    }

    // When not generating proto files, verify that the pre-generated files exist
    #[cfg(not(feature = "generate-proto"))]
    {
        use std::path::Path;

        let provider_rs = Path::new("src/analyzer_service/provider.rs");
        let descriptor_bin = Path::new("src/analyzer_service/provider_service_descriptor.bin");

        if !provider_rs.exists() {
            panic!("Pre-generated proto file not found: {}. Run with --features generate-proto to regenerate.", provider_rs.display());
        }

        if !descriptor_bin.exists() {
            panic!("Pre-generated descriptor file not found: {}. Run with --features generate-proto to regenerate.", descriptor_bin.display());
        }
    }
}
