"""
Common utilities for 0x11-b Sentinel Hardening Tests.
Extends chain_utils from 0x11-a with additional helpers for SegWit and ERC20 testing.
"""

import os
import sys
import json
import time
import hashlib
import requests
from typing import Optional, Dict, Any, List, Tuple
from dataclasses import dataclass

# Import base utilities from 0x11a
_script_dir = os.path.dirname(os.path.abspath(__file__))
_0x11a_common = os.path.join(os.path.dirname(_script_dir), "..", "0x11a_real_chain", "common")
sys.path.insert(0, os.path.abspath(_0x11a_common))
from chain_utils import BtcRpc, EthRpc, GatewayClient, check_node_health, BlockInfo

# Also import JWT helper
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "0x11_funding"))
try:
    from common_jwt import setup_jwt_user
except ImportError:
    # Fallback if common_jwt not available
    def setup_jwt_user():
        """Fallback JWT setup using Gateway API."""
        gateway_url = os.getenv("GATEWAY_URL", "http://127.0.0.1:8080")
        import uuid
        email = f"test_{uuid.uuid4().hex[:8]}@test.com"
        password = "TestPass123!"
        
        # Register
        username = f"user_{uuid.uuid4().hex[:8]}"
        resp = requests.post(f"{gateway_url}/api/v1/auth/register", json={
            "username": username,
            "email": email,
            "password": password
        })
        
        if resp.status_code not in [200, 201]:
            raise Exception(f"Registration failed: {resp.text}")
        
        # Login
        resp = requests.post(f"{gateway_url}/api/v1/auth/login", json={
            "email": email,
            "password": password
        })
        
        if resp.status_code != 200:
            raise Exception(f"Login failed: {resp.text}")
        
        data = resp.json().get("data", {})
        token = data.get("token")
        user_id = data.get("user_id")
        
        headers = {"Authorization": f"Bearer {token}"}
        return user_id, token, headers


# =============================================================================
# Configuration
# =============================================================================

BTC_REQUIRED_CONFIRMATIONS = int(os.getenv("BTC_REQUIRED_CONFIRMATIONS", "6"))
ETH_REQUIRED_CONFIRMATIONS = int(os.getenv("ETH_REQUIRED_CONFIRMATIONS", "12"))
MIN_DEPOSIT_AMOUNT_BTC = float(os.getenv("MIN_DEPOSIT_AMOUNT_BTC", "0.0001"))
GATEWAY_URL = os.getenv("GATEWAY_URL", "http://127.0.0.1:8080")


# =============================================================================
# Extended BTC Helpers (SegWit Support)
# =============================================================================

class BtcRpcExtended(BtcRpc):
    """Extended BTC RPC with SegWit-specific helpers."""
    
    def get_new_segwit_address(self) -> str:
        """Generate a new native SegWit (bech32) address."""
        return self._call("getnewaddress", ["", "bech32"])
    
    def get_new_legacy_address(self) -> str:
        """Generate a new legacy (P2PKH) address."""
        return self._call("getnewaddress", ["", "legacy"])
    
    def get_new_p2sh_segwit_address(self) -> str:
        """Generate a new P2SH-wrapped SegWit address."""
        return self._call("getnewaddress", ["", "p2sh-segwit"])
    
    def decode_raw_transaction(self, tx_hex: str) -> Dict[str, Any]:
        """Decode a raw transaction."""
        return self._call("decoderawtransaction", [tx_hex])
    
    def get_transaction(self, txid: str) -> Dict[str, Any]:
        """Get transaction details."""
        return self._call("gettransaction", [txid])
    
    def create_raw_transaction(self, inputs: List[Dict], outputs: Dict[str, float]) -> str:
        """Create a raw transaction."""
        return self._call("createrawtransaction", [inputs, outputs])
    
    def sign_raw_transaction(self, tx_hex: str) -> Dict[str, Any]:
        """Sign a raw transaction."""
        return self._call("signrawtransactionwithwallet", [tx_hex])
    
    def send_raw_transaction(self, tx_hex: str) -> str:
        """Broadcast a raw transaction."""
        return self._call("sendrawtransaction", [tx_hex])
    
    def list_unspent(self, min_conf: int = 0, max_conf: int = 9999999) -> List[Dict]:
        """List unspent outputs."""
        return self._call("listunspent", [min_conf, max_conf])
    
    def get_address_info(self, address: str) -> Dict[str, Any]:
        """Get information about an address."""
        return self._call("getaddressinfo", [address])
    
    def send_to_address_with_multiple_outputs(
        self, 
        outputs: List[Tuple[str, float]]
    ) -> str:
        """
        Create a transaction with multiple outputs to the same or different addresses.
        Returns txid.
        """
        # Get UTXOs
        utxos = self.list_unspent(1)
        if not utxos:
            raise Exception("No UTXOs available")
        
        # Calculate total needed
        total_needed = sum(amount for _, amount in outputs) + 0.0001  # + fee
        
        # Select inputs
        selected_inputs = []
        total_input = 0
        for utxo in utxos:
            selected_inputs.append({
                "txid": utxo["txid"],
                "vout": utxo["vout"]
            })
            total_input += utxo["amount"]
            if total_input >= total_needed:
                break
        
        if total_input < total_needed:
            raise Exception(f"Insufficient funds: have {total_input}, need {total_needed}")
        
        # Build outputs dict
        outputs_dict = {}
        for addr, amount in outputs:
            if addr in outputs_dict:
                outputs_dict[addr] += amount
            else:
                outputs_dict[addr] = amount
        
        # Add change
        change = total_input - sum(outputs_dict.values()) - 0.0001
        if change > 0.00001:
            change_addr = self._call("getnewaddress")
            outputs_dict[change_addr] = round(change, 8)
        
        # Create, sign, send
        raw_tx = self.create_raw_transaction(selected_inputs, outputs_dict)
        signed = self.sign_raw_transaction(raw_tx)
        return self.send_raw_transaction(signed["hex"])


# =============================================================================
# Extended ETH Helpers (ERC20 Support)
# =============================================================================

# ERC20 Transfer event topic
ERC20_TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

@dataclass
class ERC20Transfer:
    """Represents an ERC20 Transfer event."""
    contract: str
    from_addr: str
    to_addr: str
    amount: int
    tx_hash: str
    block_number: int


class EthRpcExtended(EthRpc):
    """Extended ETH RPC with ERC20-specific helpers."""
    
    def get_logs(
        self, 
        from_block: int, 
        to_block: int, 
        address: str = None,
        topics: List[str] = None
    ) -> List[Dict]:
        """Get event logs."""
        params = {
            "fromBlock": hex(from_block),
            "toBlock": hex(to_block)
        }
        if address:
            params["address"] = address
        if topics:
            params["topics"] = topics
        
        return self._call("eth_getLogs", [params])
    
    def get_erc20_transfers(
        self, 
        from_block: int, 
        to_block: int,
        contract_address: str = None
    ) -> List[ERC20Transfer]:
        """Get all ERC20 Transfer events in a block range."""
        topics = [ERC20_TRANSFER_TOPIC]
        
        logs = self.get_logs(
            from_block, 
            to_block, 
            address=contract_address,
            topics=topics
        )
        
        transfers = []
        for log in logs:
            # Parse topics
            # topics[0] = Transfer signature
            # topics[1] = from (padded to 32 bytes)
            # topics[2] = to (padded to 32 bytes)
            # data = amount (uint256)
            
            if len(log.get("topics", [])) < 3:
                continue
            
            from_addr = "0x" + log["topics"][1][-40:]
            to_addr = "0x" + log["topics"][2][-40:]
            amount = int(log["data"], 16)
            
            transfers.append(ERC20Transfer(
                contract=log["address"],
                from_addr=from_addr,
                to_addr=to_addr,
                amount=amount,
                tx_hash=log["transactionHash"],
                block_number=int(log["blockNumber"], 16)
            ))
        
        return transfers
    
    def deploy_mock_erc20(self, name: str = "MockUSDT", decimals: int = 6) -> str:
        """Deploy a mock ERC20 contract. Returns contract address."""
        # For testing, we'll use Anvil's built-in USDT mock or skip
        # In real tests, this would deploy actual bytecode
        raise NotImplementedError("Use Anvil's pre-deployed contracts or forge script")
    
    def call_erc20_transfer(
        self, 
        contract: str, 
        from_addr: str,
        to_addr: str, 
        amount: int
    ) -> str:
        """Call ERC20 transfer function. Returns tx hash."""
        # transfer(address,uint256) = 0xa9059cbb
        # Encode: to address (32 bytes) + amount (32 bytes)
        data = "0xa9059cbb" + to_addr[2:].zfill(64) + hex(amount)[2:].zfill(64)
        
        tx = {
            "from": from_addr,
            "to": contract,
            "data": data
        }
        
        return self._call("eth_sendTransaction", [tx])
    
    def get_erc20_balance(self, contract: str, address: str) -> int:
        """Get ERC20 balance. Returns raw amount (need to divide by decimals)."""
        # balanceOf(address) = 0x70a08231
        data = "0x70a08231" + address[2:].zfill(64)
        
        result = self._call("eth_call", [{
            "to": contract,
            "data": data
        }, "latest"])
        
        return int(result, 16) if result else 0


# =============================================================================
# Extended Gateway Client
# =============================================================================

class GatewayClientExtended(GatewayClient):
    """Extended Gateway client with additional helpers."""
    
    def get_deposit_address_with_validation(
        self, 
        headers: Dict[str, str], 
        asset: str, 
        network: str
    ) -> Tuple[str, bool]:
        """Get deposit address and validate format. Returns (address, is_valid)."""
        addr = self.get_deposit_address(headers, asset, network)
        
        if network == "BTC":
            # Validate BTC address formats
            is_valid = (
                addr.startswith("bcrt1") or  # Regtest SegWit
                addr.startswith("bc1") or     # Mainnet SegWit
                addr.startswith("1") or       # Legacy P2PKH
                addr.startswith("3") or       # P2SH
                addr.startswith("m") or       # Regtest Legacy
                addr.startswith("n") or       # Regtest Legacy
                addr.startswith("2")          # Regtest P2SH
            )
            return addr, is_valid
        
        elif network == "ETH":
            # Validate ETH address format
            is_valid = addr.startswith("0x") and len(addr) == 42
            return addr, is_valid
        
        return addr, True
    
    def get_chain_cursor(self, chain_id: str) -> Optional[Dict]:
        """Get chain cursor status (if available via API)."""
        try:
            resp = requests.get(
                f"{self.base_url}/internal/sentinel/cursor/{chain_id}",
                headers={"X-Internal-Secret": os.getenv("INTERNAL_SECRET", "dev-secret")}
            )
            if resp.status_code == 200:
                return resp.json().get("data")
        except:
            pass
        return None
    
    def get_deposit_by_tx_hash(
        self, 
        headers: Dict[str, str], 
        asset: str,
        tx_hash: str
    ) -> Optional[Dict]:
        """Get specific deposit by tx_hash."""
        history = self.get_deposit_history(headers, asset)
        for record in history:
            if record.get("tx_hash") == tx_hash:
                return record
        return None
    
    def wait_for_deposit_status(
        self,
        headers: Dict[str, str],
        asset: str,
        tx_hash: str,
        target_status: str,
        timeout: int = 60
    ) -> Optional[Dict]:
        """Wait for deposit to reach a specific status."""
        start = time.time()
        while time.time() - start < timeout:
            deposit = self.get_deposit_by_tx_hash(headers, asset, tx_hash)
            if deposit and deposit.get("status") == target_status:
                return deposit
            time.sleep(1)
        return None


# =============================================================================
# Test Utilities
# =============================================================================

def generate_random_tx_hash() -> str:
    """Generate a random transaction hash for testing."""
    return hashlib.sha256(os.urandom(32)).hexdigest()


def is_valid_bech32_address(address: str) -> bool:
    """Validate a Bech32 address format."""
    try:
        import bech32
        hrp, data = bech32.bech32_decode(address)
        return hrp is not None and data is not None
    except:
        # Basic validation if bech32 lib not available
        return (
            address.startswith("bc1") or 
            address.startswith("bcrt1") or
            address.startswith("tb1")
        ) and len(address) >= 42


def is_valid_eth_address(address: str) -> bool:
    """Validate an Ethereum address format."""
    if not address.startswith("0x"):
        return False
    if len(address) != 42:
        return False
    try:
        int(address[2:], 16)
        return True
    except ValueError:
        return False


def get_test_config() -> Dict[str, Any]:
    """Get test configuration from environment."""
    return {
        "gateway_url": GATEWAY_URL,
        "btc_rpc_url": os.getenv("BTC_RPC_URL", "http://127.0.0.1:18443"),
        "eth_rpc_url": os.getenv("ETH_RPC_URL", "http://127.0.0.1:8545"),
        "btc_confirmations": BTC_REQUIRED_CONFIRMATIONS,
        "eth_confirmations": ETH_REQUIRED_CONFIRMATIONS,
        "min_deposit_btc": MIN_DEPOSIT_AMOUNT_BTC,
    }


def print_test_header(test_id: str, title: str, agent: str = ""):
    """Print a standardized test header."""
    agent_colors = {
        "A": "\033[91m",  # Red
        "B": "\033[92m",  # Green
        "C": "\033[94m",  # Blue
    }
    reset = "\033[0m"
    
    color = agent_colors.get(agent, "")
    print(f"\n{color}{'='*70}")
    print(f"📋 {test_id}: {title}")
    print(f"{'='*70}{reset}")


def print_test_result(passed: bool, message: str = ""):
    """Print a standardized test result."""
    if passed:
        print(f"   ✅ PASSED{': ' + message if message else ''}")
    else:
        print(f"   ❌ FAILED{': ' + message if message else ''}")


# =============================================================================
# Re-export common utilities
# =============================================================================

__all__ = [
    "BtcRpc",
    "BtcRpcExtended",
    "EthRpc", 
    "EthRpcExtended",
    "GatewayClient",
    "GatewayClientExtended",
    "check_node_health",
    "BlockInfo",
    "ERC20Transfer",
    "setup_jwt_user",
    "generate_random_tx_hash",
    "is_valid_bech32_address",
    "is_valid_eth_address",
    "get_test_config",
    "print_test_header",
    "print_test_result",
    "BTC_REQUIRED_CONFIRMATIONS",
    "ETH_REQUIRED_CONFIRMATIONS",
    "MIN_DEPOSIT_AMOUNT_BTC",
    "GATEWAY_URL",
]
