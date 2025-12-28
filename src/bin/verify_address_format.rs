use bitcoincore_rpc::bitcoin::{Address, Network};
use std::str::FromStr;
use zero_x_infinity::funding::chain_adapter::{ChainClient, MockBtcChain};

#[tokio::main]
async fn main() {
    println!("=== Focused Verification: Address Format (DEF-001) ===");

    let chain = MockBtcChain;
    let address_str = chain
        .generate_address(123)
        .await
        .expect("Generation failed");
    println!("Generated: {}", address_str);

    // 1. Prefix Check
    if !address_str.starts_with("bcrt1") {
        println!("❌ FAIL: Does not start with 'bcrt1'");
        std::process::exit(1);
    }

    // 2. Network Parsing Check
    let addr = Address::from_str(&address_str).expect("Failed to parse address structure");

    // In newer bitcoin lib, parsing is unchecked for network unless explicit.
    // We check validity against Regtest specifically.
    if addr.is_valid_for_network(Network::Regtest) {
        println!("✅ Checksum/Network: PASS (Valid Regtest)");
        std::process::exit(0);
    } else {
        println!("❌ Checksum/Network: FAIL (Not valid for Regtest)");
        std::process::exit(1);
    }
}
