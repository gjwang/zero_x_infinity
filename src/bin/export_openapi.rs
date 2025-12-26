//! Export OpenAPI specification to JSON file
//!
//! Usage:
//!   cargo run --bin export_openapi > openapi.json
//!
//! Or with file output:
//!   cargo run --bin export_openapi -- --output docs/openapi.json

use utoipa::OpenApi;
use zero_x_infinity::gateway::openapi::ApiDoc;

fn main() {
    // Generate OpenAPI spec
    let spec = ApiDoc::openapi();

    // Check for output file argument
    let args: Vec<String> = std::env::args().collect();
    let output_path = if args.len() > 2 && args[1] == "--output" {
        Some(args[2].as_str())
    } else {
        None
    };

    // Convert to JSON
    let json = spec
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI spec");

    // Output
    match output_path {
        Some(path) => {
            std::fs::write(path, &json).expect("Failed to write file");
            eprintln!("✅ OpenAPI spec exported to: {}", path);
        }
        None => {
            println!("{}", json);
        }
    }
}
