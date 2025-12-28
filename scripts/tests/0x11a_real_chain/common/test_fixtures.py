"""
Common test fixtures for 0x11-a Real Chain Integration Tests.
Provides shared constants and test data.
"""

# =============================================================================
# Chain Configuration
# =============================================================================

# BTC Regtest Configuration
BTC_REQUIRED_CONFIRMATIONS = 6
BTC_MIN_DEPOSIT = 0.001  # 100k satoshi
BTC_MAX_REORG_DEPTH = 10

# ETH Configuration (Anvil)
ETH_REQUIRED_CONFIRMATIONS = 12
ETH_MIN_DEPOSIT = 0.01  # 0.01 ETH
ETH_MAX_REORG_DEPTH = 50

# =============================================================================
# Test Accounts (Anvil Default)
# =============================================================================

ANVIL_TEST_ACCOUNTS = [
    {
        "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    },
    {
        "address": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
        "private_key": "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    },
]

# =============================================================================
# Test Data
# =============================================================================

VALID_BTC_ADDRESSES = [
    "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",  # Bech32
    "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",  # Legacy (Satoshi's address)
    "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",  # P2SH
]

INVALID_BTC_ADDRESSES = [
    "bc1invalid",
    "0x1234567890",  # ETH format
    "",
    "abc123",
    "bc1" + "x" * 100,  # Too long
]

VALID_ETH_ADDRESSES = [
    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
    "0x0000000000000000000000000000000000000000",
]

INVALID_ETH_ADDRESSES = [
    "0x123",  # Too short
    "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",  # BTC format
    "0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",  # Invalid hex
    "",
]

# =============================================================================
# SQL Injection Payloads
# =============================================================================

SQL_INJECTION_PAYLOADS = [
    "'; DROP TABLE chain_cursor; --",
    "1; DELETE FROM users; --",
    "' OR '1'='1",
    "'; UPDATE accounts SET balance=999999 WHERE 1=1; --",
    "1' UNION SELECT password FROM admin--",
    "${7*7}",
    "{{7*7}}",
    "'; WAITFOR DELAY '0:0:5'--",  # Time-based
    "1 AND SLEEP(5)--",
]

# =============================================================================
# XSS Payloads
# =============================================================================

XSS_PAYLOADS = [
    "<script>alert('xss')</script>",
    "<img src=x onerror=alert('xss')>",
    "javascript:alert('xss')",
    "'><script>alert('xss')</script>",
]

# =============================================================================
# Expected Status Codes
# =============================================================================

class DepositStatus:
    DETECTED = "DETECTED"
    CONFIRMING = "CONFIRMING"
    SUCCESS = "SUCCESS"
    ORPHANED = "ORPHANED"
    REVERTED = "REVERTED"
    FAILED = "FAILED"


class WithdrawStatus:
    PENDING = "PENDING"
    PROCESSING = "PROCESSING"
    SUCCESS = "SUCCESS"
    FAILED = "FAILED"
    CANCELLED = "CANCELLED"


# =============================================================================
# Test Utilities
# =============================================================================

def satoshi_to_btc(satoshi: int) -> float:
    """Convert satoshi to BTC."""
    return satoshi / 100_000_000


def btc_to_satoshi(btc: float) -> int:
    """Convert BTC to satoshi."""
    return int(btc * 100_000_000)


def wei_to_eth(wei: int) -> float:
    """Convert wei to ETH."""
    return wei / 10**18


def eth_to_wei(eth: float) -> int:
    """Convert ETH to wei."""
    return int(eth * 10**18)
