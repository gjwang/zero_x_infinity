#!/usr/bin/env python3
"""
Database Management Script for exchange_info_db
=================================================
Usage:
    python scripts/db/manage_db.py reset      # Reset schema to clean state
    python scripts/db/manage_db.py seed       # Apply seed data
    python scripts/db/manage_db.py init       # Reset + seed (full initialization)
    python scripts/db/manage_db.py status     # Show current counts
"""

import argparse
import subprocess
import sys
from pathlib import Path

# =============================================================================
# Configuration
# =============================================================================

PROJECT_DIR = Path(__file__).parent.parent.parent
MIGRATIONS_DIR = PROJECT_DIR / "migrations"
FIXTURES_DIR = PROJECT_DIR / "fixtures"

# Database settings (can be overridden via environment)
import os
PG_CONTAINER = os.getenv("PG_CONTAINER", "postgres")
PG_USER = os.getenv("PG_USER", "trading")
PG_DB = os.getenv("PG_DB", "exchange_info_db")

# Colors
GREEN = "\033[0;32m"
BLUE = "\033[0;34m"
RED = "\033[0;31m"
NC = "\033[0m"


def log_info(msg: str):
    print(f"{BLUE}[INFO]{NC} {msg}")


def log_success(msg: str):
    print(f"{GREEN}[OK]{NC} {msg}")


def log_error(msg: str):
    print(f"{RED}[ERROR]{NC} {msg}")


# =============================================================================
# Database Operations
# =============================================================================

def run_psql(sql: str) -> tuple[bool, str]:
    """Execute SQL via psql in docker container"""
    cmd = [
        "docker", "exec", PG_CONTAINER,
        "psql", "-U", PG_USER, "-d", PG_DB, "-c", sql
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.returncode == 0, result.stdout + result.stderr


def run_psql_file(filepath: Path) -> tuple[bool, str]:
    """Execute SQL file via psql in docker container"""
    with open(filepath) as f:
        sql = f.read()
    
    cmd = [
        "docker", "exec", "-i", PG_CONTAINER,
        "psql", "-U", PG_USER, "-d", PG_DB
    ]
    result = subprocess.run(cmd, input=sql, capture_output=True, text=True)
    return result.returncode == 0, result.stdout + result.stderr


def get_count(table: str) -> int:
    """Get row count for a table"""
    success, output = run_psql(f"SELECT COUNT(*) FROM {table}")
    if success:
        try:
            return int(output.strip().split('\n')[2].strip())
        except:
            return 0
    return 0


# =============================================================================
# Commands
# =============================================================================

def cmd_reset():
    """Reset database to clean schema (no data)"""
    log_info(f"Resetting database: {PG_DB}")
    
    # Drop tables
    log_info("Dropping existing tables...")
    run_psql("""
        DROP TABLE IF EXISTS symbols_tb CASCADE;
        DROP TABLE IF EXISTS assets_tb CASCADE;
        DROP TABLE IF EXISTS users_tb CASCADE;
    """)
    log_success("Tables dropped")
    
    # Apply schema
    schema_file = MIGRATIONS_DIR / "001_init_schema.sql"
    if not schema_file.exists():
        log_error(f"Schema file not found: {schema_file}")
        return False
    
    log_info("Applying fresh schema...")
    success, output = run_psql_file(schema_file)
    
    if success:
        log_success("Schema applied successfully")
        return True
    else:
        log_error(f"Failed to apply schema: {output}")
        return False


def cmd_seed():
    """Apply seed data"""
    log_info(f"Seeding test data into: {PG_DB}")
    
    seed_file = FIXTURES_DIR / "seed_data.sql"
    if not seed_file.exists():
        log_error(f"Seed data file not found: {seed_file}")
        return False
    
    success, output = run_psql_file(seed_file)
    
    if success:
        log_success("Seed data applied")
        cmd_status()
        return True
    else:
        log_error(f"Failed to apply seed data: {output}")
        return False


def cmd_init():
    """Full initialization: reset + seed"""
    log_info("Full database initialization")
    
    if not cmd_reset():
        return False
    
    return cmd_seed()


def cmd_status():
    """Show current database status"""
    log_info(f"Database status: {PG_DB}")
    
    assets = get_count("assets_tb")
    symbols = get_count("symbols_tb")
    users = get_count("users_tb")
    
    print(f"  Assets:  {assets}")
    print(f"  Symbols: {symbols}")
    print(f"  Users:   {users}")
    
    return True


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Database management for exchange_info_db"
    )
    parser.add_argument(
        "command",
        choices=["reset", "seed", "init", "status"],
        help="Command to execute"
    )
    
    args = parser.parse_args()
    
    commands = {
        "reset": cmd_reset,
        "seed": cmd_seed,
        "init": cmd_init,
        "status": cmd_status,
    }
    
    success = commands[args.command]()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
