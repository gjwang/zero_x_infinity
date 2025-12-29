# Chains & Tokens Schema Design

> **Phase**: 0x11-c (Planned)  
> **Status**: Draft  
> **Date**: 2025-12-29

## Design Principles

> [!IMPORTANT]
> **Clarity First, Performance Second**
>
> 1. Reducing confusion is the top priority
> 2. Data volume is small (< 100 chains), so VARCHAR FK is acceptable

## 1. Background

### Industry Standards

| Standard | Description | Link |
|:---|:---|:---|
| **EIP-155** | Numeric Chain ID (1, 137, 56) | [eips.ethereum.org](https://eips.ethereum.org/EIPS/eip-155) |
| **ethereum-lists/chains** | Chain registry with shortName | [GitHub](https://github.com/ethereum-lists/chains) |
| **SLIP-0044** | BIP-44 coin type derivation | [GitHub](https://github.com/satoshilabs/slips/blob/master/slip-0044.md) |

### Naming Conventions

| Field | Type | Example | Usage |
|:---|:---|:---|:---|
| `chain_slug` | VARCHAR | `"eth"`, `"btc"` | Business identifier (internal) |
| `chain_id` | INTEGER | `1`, `56` | EIP-155 identifier (EVM only) |
| `symbol` | VARCHAR | `"BTC_USDT"` | Trading pair symbol |
| `native_currency` | VARCHAR | `"ETH"` | Chain's native token |

---

## 2. Schema

### 2.1 Chains Table

```sql
CREATE TABLE chains (
    id SERIAL PRIMARY KEY,                   -- 1. Internal ID
    chain_slug VARCHAR(32) UNIQUE NOT NULL,  -- 2. Business ID: "eth", "btc"
    chain_id INTEGER,                        -- 3. EIP-155: 1, 56 (NULL for non-EVM)
    full_name VARCHAR(128) NOT NULL,         -- 4. "Ethereum Mainnet"
    display_name VARCHAR(64) NOT NULL,       -- 5. "Ethereum"
    native_currency VARCHAR(16) NOT NULL,    -- 6. "ETH"
    chain_type VARCHAR(16) NOT NULL,         -- 7. "EVM", "UTXO", "ACCOUNT"
    slip44_index INTEGER,                    -- 8. BIP-44: 60, 0
    explorer_url VARCHAR(255),               -- 9. Block explorer
    is_testnet BOOLEAN DEFAULT FALSE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 2.2 Tokens Table

```sql
CREATE TABLE tokens (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(16) NOT NULL,              -- "USDT"
    name VARCHAR(128) NOT NULL,               -- "Tether USD"
    decimals INTEGER NOT NULL,                -- 6
    token_type VARCHAR(16) NOT NULL,          -- "NATIVE", "ERC20"
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains(chain_slug),  -- FK to chains.chain_slug
    contract_address VARCHAR(255),            -- NULL for native
    is_active BOOLEAN DEFAULT TRUE,
    
    UNIQUE (chain_slug, contract_address)
);
```

### 2.3 User Addresses Table

```sql
CREATE TABLE user_addresses (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains(chain_slug),  -- FK to chains.chain_slug
    address VARCHAR(255) NOT NULL,
    
    UNIQUE (user_id, chain_slug)
);
```

---

## 3. Seed Data

| chain_slug | chain_id | full_name | native_currency | chain_type | slip44_index |
|:---|:---|:---|:---|:---|:---|
| `eth` | 1 | Ethereum Mainnet | ETH | EVM | 60 |
| `bnb` | 56 | BNB Smart Chain | BNB | EVM | 9006 |
| `matic` | 137 | Polygon Mainnet | MATIC | EVM | 60 |
| `btc` | 0 | Bitcoin Mainnet | BTC | UTXO | 0 |
| `trx` | 195 | Tron Mainnet | TRX | ACCOUNT | 195 |
| `sol` | 501 | Solana Mainnet | SOL | ACCOUNT | 501 |
| `btc_regtest` | 0 | Bitcoin Regtest | BTC | UTXO | 0 |

> [!NOTE]
> **chain_id 来源**:
> - **EVM 链**: EIP-155 标准 (1=ETH, 56=BSC, 137=Polygon)
> - **非 EVM 链**: SLIP-0044 `coin_type` (0=BTC, 195=TRX, 501=SOL)
>
> 参考: [SLIP-0044](https://github.com/satoshilabs/slips/blob/master/slip-0044.md)

---

## 4. Phase 0x11-b Quick Fix

For current sprint, use `chain_slug` column only:

```sql
-- Temporary (Phase 0x11-b)
CREATE TABLE user_addresses (
    user_id BIGINT,
    asset VARCHAR(32),
    chain_slug VARCHAR(32),  -- "eth", "btc"
    address VARCHAR(255),
    PRIMARY KEY (user_id, asset, chain_slug)
);
```

Full schema deferred to Phase 0x11-c.
