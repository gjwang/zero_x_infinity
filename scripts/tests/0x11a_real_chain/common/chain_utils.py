"""
Common utilities for 0x11-a Real Chain Integration Tests.
Provides helpers for BTC/ETH RPC interactions.
"""

import os
import json
import subprocess
import requests
import time
from typing import Optional, Dict, Any, List
from dataclasses import dataclass

# =============================================================================
# Configuration
# =============================================================================

BTC_RPC_URL = os.getenv("BTC_RPC_URL", "http://127.0.0.1:18443")
BTC_RPC_USER = os.getenv("BTC_RPC_USER", "user")
BTC_RPC_PASS = os.getenv("BTC_RPC_PASS", "pass")
BTC_WALLET = os.getenv("BTC_WALLET", "sentinel_test")

ETH_RPC_URL = os.getenv("ETH_RPC_URL", "http://127.0.0.1:8545")

GATEWAY_URL = os.getenv("GATEWAY_URL", "http://127.0.0.1:8080")
INTERNAL_SECRET = os.getenv("INTERNAL_SECRET", "dev-secret")


@dataclass
class BlockInfo:
    height: int
    hash: str
    parent_hash: str
    tx_count: int


@dataclass
class DepositStatus:
    tx_hash: str
    status: str
    confirmations: int
    amount: str
    asset: str


# =============================================================================
# Bitcoin RPC Helpers
# =============================================================================

class BtcRpc:
    """Bitcoin Core RPC client for regtest."""
    
    def __init__(self, url: str = BTC_RPC_URL, user: str = BTC_RPC_USER, password: str = BTC_RPC_PASS, wallet: str = BTC_WALLET):
        # Append wallet path if specified and not already in URL
        if wallet and "/wallet/" not in url:
            self.url = f"{url.rstrip('/')}/wallet/{wallet}"
        else:
            self.url = url
            
        self.auth = (user, password)
        self._id = 0
    
    def _call(self, method: str, params: List[Any] = None) -> Any:
        self._id += 1
        payload = {
            "jsonrpc": "2.0",
            "id": self._id,
            "method": method,
            "params": params or []
        }
        resp = requests.post(self.url, json=payload, auth=self.auth)
        result = resp.json()
        if "error" in result and result["error"]:
            raise Exception(f"RPC Error: {result['error']}")
        return result.get("result")
    
    def get_blockchain_info(self) -> Dict[str, Any]:
        return self._call("getblockchaininfo")
    
    def get_block_count(self) -> int:
        return self._call("getblockcount")
    
    def get_block_hash(self, height: int) -> str:
        return self._call("getblockhash", [height])
    
    def get_block(self, blockhash: str, verbosity: int = 2) -> Dict[str, Any]:
        return self._call("getblock", [blockhash, verbosity])
    
    def mine_blocks(self, count: int = 1, address: str = None) -> List[str]:
        """Mine blocks to the given address (or generate new)."""
        if not address:
            address = self._call("getnewaddress")
        return self._call("generatetoaddress", [count, address])
    
    def send_to_address(self, address: str, amount: float) -> str:
        """Send BTC to address. Returns txid."""
        return self._call("sendtoaddress", [address, amount])
    
    def invalidate_block(self, blockhash: str) -> None:
        """Invalidate a block (simulate re-org)."""
        self._call("invalidateblock", [blockhash])
    
    def reconsider_block(self, blockhash: str) -> None:
        """Reconsider a previously invalidated block."""
        self._call("reconsiderblock", [blockhash])
    
    def get_latest_block_info(self) -> BlockInfo:
        height = self.get_block_count()
        hash_ = self.get_block_hash(height)
        block = self.get_block(hash_, 1)
        return BlockInfo(
            height=height,
            hash=hash_,
            parent_hash=block.get("previousblockhash", ""),
            tx_count=len(block.get("tx", []))
        )


# =============================================================================
# Ethereum (Anvil) Helpers
# =============================================================================

class EthRpc:
    """Ethereum JSON-RPC client (Anvil/Geth compatible)."""
    
    def __init__(self, url: str = ETH_RPC_URL):
        self.url = url
        self._id = 0
    
    def _call(self, method: str, params: List[Any] = None) -> Any:
        self._id += 1
        payload = {
            "jsonrpc": "2.0",
            "id": self._id,
            "method": method,
            "params": params or []
        }
        resp = requests.post(self.url, json=payload)
        result = resp.json()
        if "error" in result and result["error"]:
            raise Exception(f"RPC Error: {result['error']}")
        return result.get("result")
    
    def get_block_number(self) -> int:
        hex_result = self._call("eth_blockNumber")
        return int(hex_result, 16)
    
    def get_block_by_number(self, number: int, full_tx: bool = False) -> Dict[str, Any]:
        return self._call("eth_getBlockByNumber", [hex(number), full_tx])
    
    def send_transaction(self, from_: str, to: str, value_wei: int) -> str:
        """Send ETH. Returns txhash."""
        tx = {
            "from": from_,
            "to": to,
            "value": hex(value_wei)
        }
        return self._call("eth_sendTransaction", [tx])
    
    def mine_block(self) -> None:
        """Anvil-specific: Mine a block."""
        self._call("evm_mine")
    
    def set_balance(self, address: str, balance_wei: int) -> None:
        """Anvil-specific: Set account balance (cheat)."""
        self._call("anvil_setBalance", [address, hex(balance_wei)])
    
    def snapshot(self) -> str:
        """Anvil-specific: Create a snapshot. Returns snapshot ID."""
        return self._call("evm_snapshot")
    
    def revert(self, snapshot_id: str) -> bool:
        """Anvil-specific: Revert to snapshot (simulate re-org)."""
        return self._call("evm_revert", [snapshot_id])
    
    def get_latest_block_info(self) -> BlockInfo:
        height = self.get_block_number()
        block = self.get_block_by_number(height)
        return BlockInfo(
            height=height,
            hash=block["hash"],
            parent_hash=block["parentHash"],
            tx_count=len(block.get("transactions", []))
        )


# =============================================================================
# Gateway API Helpers
# =============================================================================

class GatewayClient:
    """Client for interacting with the Gateway API."""
    
    def __init__(self, base_url: str = GATEWAY_URL):
        self.base_url = base_url
    
    def get_deposit_address(self, headers: Dict[str, str], asset: str, network: str) -> str:
        """Get deposit address for authenticated user."""
        resp = requests.get(
            f"{self.base_url}/api/v1/capital/deposit/address",
            params={"asset": asset, "network": network},
            headers=headers
        )
        resp.raise_for_status()
        return resp.json()["data"]["address"]
    
    def get_deposit_history(self, headers: Dict[str, str], asset: str) -> List[Dict]:
        """Get deposit history for authenticated user."""
        resp = requests.get(
            f"{self.base_url}/api/v1/capital/deposit/history",
            params={"asset": asset},
            headers=headers
        )
        if resp.status_code == 404:
            return []
        resp.raise_for_status()
        return resp.json()["data"]
    
    def get_balance(self, headers: Dict[str, str], asset: str) -> Optional[float]:
        """Get balance for authenticated user."""
        resp = requests.get(
            f"{self.base_url}/api/v1/private/account",
            headers=headers
        )
        if resp.status_code != 200:
            return None
        balances = resp.json().get("data", {}).get("balances", [])
        for b in balances:
            if b["asset"] == asset:
                return float(b["available"])
        return 0.0
    
    def mock_deposit(self, user_id: int, asset: str, amount: str, tx_hash: str, chain: str) -> bool:
        """Trigger mock deposit (internal API)."""
        resp = requests.post(
            f"{self.base_url}/internal/mock/deposit",
            json={
                "user_id": user_id,
                "asset": asset,
                "amount": amount,
                "tx_hash": tx_hash,
                "chain": chain
            },
            headers={"X-Internal-Secret": INTERNAL_SECRET}
        )
        return resp.status_code == 200


# =============================================================================
# Test Utilities
# =============================================================================

def wait_for_confirmations(
    gateway: GatewayClient,
    headers: Dict[str, str],
    asset: str,
    tx_hash: str,
    target_confirmations: int,
    timeout_seconds: int = 60
) -> DepositStatus:
    """Wait for deposit to reach target confirmations."""
    start = time.time()
    while time.time() - start < timeout_seconds:
        history = gateway.get_deposit_history(headers, asset)
        for record in history:
            if record.get("tx_hash") == tx_hash:
                confs = record.get("confirmations", 0)
                if confs >= target_confirmations:
                    return DepositStatus(
                        tx_hash=tx_hash,
                        status=record.get("status", "UNKNOWN"),
                        confirmations=confs,
                        amount=record.get("amount", "0"),
                        asset=asset
                    )
        time.sleep(1)
    raise TimeoutError(f"Deposit {tx_hash} did not reach {target_confirmations} confirmations")


def check_node_health(btc: BtcRpc = None, eth: EthRpc = None) -> Dict[str, bool]:
    """Check health of blockchain nodes."""
    result = {}
    
    if btc:
        try:
            btc.get_block_count()
            result["btc"] = True
        except Exception:
            result["btc"] = False
    
    if eth:
        try:
            eth.get_block_number()
            result["eth"] = True
        except Exception:
            result["eth"] = False
    
    return result
