"""
Transfer Test Helper Functions
Reusable utilities for E2E transfer testing

Usage:
    from lib.transfer_test_helpers import TransferTestHelpers, get_balance_db
    
    helpers = TransferTestHelpers(client)
    balance = helpers.get_balance(1001, 'USDT', 'funding')
    result = helpers.execute_transfer(1001, 'funding', 'spot', 'USDT', '50')
"""

import os
import subprocess
from typing import Dict, Optional, Tuple

class TransferTestHelpers:
    """Helper functions for transfer E2E testing"""
    
    def __init__(self, client):
        self.client = client
        # Get DB credentials from environment
        self.pg_host = os.getenv('PG_HOST', 'localhost')
        self.pg_port = os.getenv('PG_PORT', '5433')
        self.pg_user = os.getenv('PG_USER', 'trading')
        self.pg_password = os.getenv('PG_PASSWORD', 'trading123')
        self.pg_db = os.getenv('PG_DB', 'exchange_info_db')
    
    # ============================================================
    # Balance Helpers
    # ============================================================
    
    def get_balance(self, user_id: int, asset_symbol: str, account_type_str: str) -> float:
        """
        Get balance via API
        
        Args:
            user_id: User ID
            asset_symbol: Asset symbol (e.g., 'USDT')
            account_type_str: 'spot' or 'funding'
        
        Returns:
            Balance in asset units
        """
        resp = self.client.get('/api/v1/private/balances/all',
                               headers={'X-User-ID': str(user_id)})
        
        if resp.status_code != 200:
            return 0.0
        
        balances = resp.json().get('data', [])
        key = f"{asset_symbol}:{account_type_str}"
        
        for b in balances:
            balance_key = f"{b['asset']}:{b['account_type']}"
            if balance_key == key:
                return float(b['available'])
        
        return 0.0
    
    def get_all_balances(self, user_id: int) -> Dict[str, float]:
        """Get all balances for a user"""
        resp = self.client.get('/api/v1/private/balances/all',
                               headers={'X-User-ID': str(user_id)})
        
        if resp.status_code != 200:
            return {}
        
        balances = {}
        for b in resp.json().get('data', []):
            key = f"{b['asset']}:{b['account_type']}"
            balances[key] = float(b['available'])
        
        return balances
    
    # ============================================================
    # Database Helpers
    # ============================================================
    
    def db_query(self, sql: str) -> Optional[str]:
        """Execute SQL query and return result"""
        cmd = [
            'psql',
            '-h', self.pg_host,
            '-p', self.pg_port,
            '-U', self.pg_user,
            '-d', self.pg_db,
            '-t', '-A',  # Tuples only, no alignment
            '-c', sql
        ]
        
        env = os.environ.copy()
        env['PGPASSWORD'] = self.pg_password
        
        try:
            result = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=5)
            if result.returncode == 0:
                return result.stdout.strip()
            return None
        except Exception:
            return None
    
    def get_balance_db(self, user_id: int, asset_id: int, account_type: int) -> float:
        """
        Get balance from database directly
        
        Args:
            account_type: 1=Spot, 2=Funding
        """
        sql = f"""
            SELECT available FROM balances_tb
            WHERE user_id = {user_id} 
              AND asset_id = {asset_id} 
              AND account_type = {account_type}
        """
        result = self.db_query(sql)
        
        if not result:
            return 0.0
        
        return float(result) / 1_000_000  # Scale from 10^6
    
    def setup_user_balance(self, user_id: int, asset_id: int, account_type: int, amount: float):
        """Setup user balance in database"""
        amount_scaled = int(amount * 1_000_000)
        
        sql = f"""
            INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, status)
            VALUES ({user_id}, {asset_id}, {account_type}, {amount_scaled}, 0, 1)
            ON CONFLICT (user_id, asset_id, account_type) 
            DO UPDATE SET available = {amount_scaled}, frozen = 0;
        """
        self.db_query(sql)
    
    def clear_user_transfers(self, user_id: int):
        """Clear all transfer records for a user"""
        # Clear operations first (foreign key)
        self.db_query(f"""
            DELETE FROM transfer_operations_tb WHERE transfer_id IN (
                SELECT transfer_id FROM fsm_transfers_tb WHERE user_id = {user_id}
            )
        """)
        # Then clear transfers
        self.db_query(f"DELETE FROM fsm_transfers_tb WHERE user_id = {user_id}")
    
    # ============================================================
    # Transfer Helpers
    # ============================================================
    
    def execute_transfer(self, user_id: int, from_account: str, to_account: str,
                        asset: str, amount: str, cid: Optional[str] = None) -> Dict:
        """
        Execute a transfer and return result
        
        Returns:
            {
                'success': bool,
                'status_code': int,
                'req_id': str or None,
                'state': str or None,
                'error': str or None
            }
        """
        payload = {
            'from': from_account,
            'to': to_account,
            'asset': asset,
            'amount': amount
        }
        
        if cid:
            payload['cid'] = cid
        
        resp = self.client.post('/api/v1/private/transfer',
                               json_body=payload,
                               headers={'X-User-ID': str(user_id)})
        
        result = {
            'success': resp.status_code == 200,
            'status_code': resp.status_code,
            'req_id': None,
            'state': None,
            'error': None
        }
        
        if resp.status_code == 200:
            data = resp.json().get('data', {})
            result['req_id'] = data.get('req_id')
            result['state'] = data.get('state')
        else:
            result['error'] = resp.text
        
        return result
    
    # ============================================================
    # FSM State Helpers
    # ============================================================
    
    def get_transfer_state(self, user_id: int, req_id: str) -> Optional[str]:
        """Query transfer state via API"""
        resp = self.client.get(f'/api/v1/private/transfer/{req_id}',
                              headers={'X-User-ID': str(user_id)})
        
        if resp.status_code != 200:
            return None
        
        return resp.json().get('data', {}).get('state')
    
    def get_transfer_state_db(self, req_id: str) -> Optional[int]:
        """Query transfer state from database"""
        sql = f"SELECT state FROM fsm_transfers_tb WHERE req_id = '{req_id}'"
        result = self.db_query(sql)
        
        if result:
            return int(result)
        return None
    
    FSM_STATE_NAMES = {
        0: 'INIT',
        10: 'SOURCE_PENDING',
        20: 'SOURCE_DONE',
        30: 'TARGET_PENDING',
        40: 'COMMITTED',
        -10: 'FAILED',
        -20: 'COMPENSATING',
        -30: 'ROLLED_BACK'
    }
    
    def state_code_to_name(self, code: int) -> str:
        """Convert state code to name"""
        return self.FSM_STATE_NAMES.get(code, f'UNKNOWN({code})')


# Standalone helper functions
def get_balance_db(user_id: int, asset_id: int, account_type: int, 
                   pg_host='localhost', pg_port='5433', 
                   pg_user='trading', pg_password='trading123',
                   pg_db='exchange_info_db') -> float:
    """Standalone function to get balance from database"""
    import subprocess
    
    sql = f"""
        SELECT available FROM balances_tb
        WHERE user_id = {user_id} 
          AND asset_id = {asset_id} 
          AND account_type = {account_type}
    """
    
    cmd = ['psql', '-h', pg_host, '-p', pg_port, '-U', pg_user, 
           '-d', pg_db, '-t', '-A', '-c', sql]
    
    env = os.environ.copy()
    env['PGPASSWORD'] = pg_password
    
    try:
        result = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=5)
        if result.returncode == 0 and result.stdout.strip():
            return float(result.stdout.strip()) / 1_000_000
    except Exception:
        pass
    
    return 0.0
