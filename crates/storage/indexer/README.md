# kora-indexer

Block and transaction indexer for Kora RPC queries.

This crate provides in-memory indexing for blocks, transactions, receipts, and logs to support Ethereum JSON-RPC queries such as:

- `eth_getBlockByNumber` / `eth_getBlockByHash`
- `eth_getTransactionByHash`
- `eth_getTransactionReceipt`
- `eth_getLogs`

## Usage

```rust,ignore
use kora_indexer::{BlockIndex, IndexedBlock, LogFilter};

// Create an index
let index = BlockIndex::new();

// Insert blocks, transactions, and receipts
index.insert_block(block, transactions, receipts);

// Query by hash or number
let block = index.get_block_by_number(1);
let tx = index.get_transaction(&tx_hash);
let receipt = index.get_receipt(&tx_hash);

// Filter logs
let filter = LogFilter::new()
    .from_block(0)
    .to_block(100)
    .address(vec![contract_address]);
let logs = index.get_logs(&filter);
```
