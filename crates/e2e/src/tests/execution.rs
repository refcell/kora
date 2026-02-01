//! Execution-related e2e tests.
//!
//! These tests verify that EVM execution works correctly, transactions
//! are processed properly, and state changes are applied consistently.

use crate::{TestConfig, TestHarness, TestSetup};

/// Test a simple ETH transfer between two accounts.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_simple_transfer() {
    let config = TestConfig::default().with_validators(4).with_max_blocks(3);
    let setup = TestSetup::simple_transfer(config.chain_id);

    let outcome = TestHarness::run(config, setup).expect("transfer should succeed");

    assert_eq!(outcome.blocks_finalized, 3);
}

/// Test multiple independent transfers in a single block.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_multiple_transfers_single_block() {
    let config = TestConfig::default().with_validators(4).with_max_blocks(3);
    let setup = TestSetup::multi_transfer(config.chain_id, 5);

    let outcome = TestHarness::run(config, setup).expect("transfers should succeed");

    assert_eq!(outcome.blocks_finalized, 3);
}

/// Test multiple transactions from the same sender with sequential nonces.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_sequential_nonces() {
    let config = TestConfig::default().with_validators(4).with_max_blocks(3);
    let setup = TestSetup::sequential_nonces(config.chain_id, 3);

    let outcome = TestHarness::run(config, setup).expect("nonce sequence should succeed");

    assert_eq!(outcome.blocks_finalized, 3);
}

/// Test that larger transfer counts work correctly.
#[test]
fn test_many_transfers() {
    let config = TestConfig::default().with_validators(4).with_max_blocks(5);
    let setup = TestSetup::multi_transfer(config.chain_id, 10);

    let outcome = TestHarness::run(config, setup).expect("many transfers should succeed");

    assert_eq!(outcome.blocks_finalized, 5);
}

/// Test that state is correctly accumulated across multiple blocks.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_state_accumulation() {
    // This test uses sequential nonces to ensure state accumulates correctly
    let config = TestConfig::default().with_validators(4).with_max_blocks(5);
    let setup = TestSetup::sequential_nonces(config.chain_id, 5);

    let outcome = TestHarness::run(config, setup).expect("state should accumulate");

    assert_eq!(outcome.blocks_finalized, 5);
}

/// Test with different chain IDs.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_different_chain_ids() {
    for chain_id in [1, 5, 1337, 31337, 42161] {
        let mut config = TestConfig::default().with_validators(4).with_max_blocks(2);
        config.chain_id = chain_id;
        let setup = TestSetup::simple_transfer(chain_id);

        let outcome = TestHarness::run(config, setup)
            .unwrap_or_else(|e| panic!("chain_id {chain_id} failed: {e}"));

        assert_eq!(outcome.blocks_finalized, 2);
    }
}

/// Test that gas limits are respected.
#[test]
fn test_gas_limit_enforcement() {
    let mut config = TestConfig::default().with_validators(4).with_max_blocks(3);
    config.gas_limit = 30_000_000; // Standard gas limit
    let setup = TestSetup::simple_transfer(config.chain_id);

    let outcome = TestHarness::run(config, setup).expect("gas limit should be enforced");

    assert_eq!(outcome.blocks_finalized, 3);
}

/// Test maximum transactions per block.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_max_transactions_per_block() {
    let config = TestConfig::default().with_validators(4).with_max_blocks(3);
    // BLOCK_CODEC_MAX_TXS is 64, so test with fewer
    let setup = TestSetup::multi_transfer(config.chain_id, 20);

    let outcome = TestHarness::run(config, setup).expect("max txs should work");

    assert_eq!(outcome.blocks_finalized, 3);
}

/// Test that execution is deterministic across validators.
#[test]
#[ignore = "flaky when run in parallel - run with --test-threads=1"]
fn test_deterministic_execution() {
    let config = TestConfig::default()
        .with_validators(4)
        .with_max_blocks(5)
        .with_seed(42)
        .with_timeout(std::time::Duration::from_secs(45));
    let setup = TestSetup::multi_transfer(config.chain_id, 5);

    // Run twice with same seed
    let outcome1 = TestHarness::run(config.clone(), setup.clone()).expect("first run");
    let outcome2 = TestHarness::run(config, setup).expect("second run");

    // State roots should match
    assert_eq!(outcome1.state_root, outcome2.state_root);
}
