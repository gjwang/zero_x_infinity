"""
Password Utilities

Per GAP-04 (Architect Decision):
- Minimum length: 12 characters
- Require uppercase: Yes
- Require number: Yes  
- Require special char: Yes
- Maximum age: 90 days
- History: 3 previous passwords
"""

import re
import bcrypt
from typing import Optional


# Password complexity requirements (per GAP-04)
MIN_LENGTH = 12
REQUIRE_UPPERCASE = True
REQUIRE_NUMBER = True
REQUIRE_SPECIAL = True


def validate_password_strength(password: str) -> bool:
    """Validate password meets complexity requirements
    
    Per GAP-04:
    - 12+ characters
    - At least one uppercase letter
    - At least one number
    - At least one special character
    
    Returns:
        True if password is strong enough, False otherwise
    """
    if len(password) < MIN_LENGTH:
        return False
    
    if REQUIRE_UPPERCASE and not re.search(r'[A-Z]', password):
        return False
    
    if REQUIRE_NUMBER and not re.search(r'\d', password):
        return False
    
    if REQUIRE_SPECIAL and not re.search(r'[!@#$%^&*(),.?":{}|<>]', password):
        return False
    
    return True


def get_password_requirements() -> dict:
    """Get human-readable password requirements"""
    return {
        "min_length": MIN_LENGTH,
        "require_uppercase": REQUIRE_UPPERCASE,
        "require_number": REQUIRE_NUMBER,
        "require_special": REQUIRE_SPECIAL,
        "message": f"Password must be at least {MIN_LENGTH} characters with uppercase, number, and special character"
    }


def hash_password(password: str) -> str:
    """Hash a password using bcrypt
    
    Per TC-DATA-01: Passwords must be hashed with bcrypt/argon2
    """
    salt = bcrypt.gensalt(rounds=12)
    hashed = bcrypt.hashpw(password.encode('utf-8'), salt)
    return hashed.decode('utf-8')


def verify_password(password: str, hashed: str) -> bool:
    """Verify a password against its hash
    
    Returns:
        True if password matches, False otherwise
    """
    try:
        return bcrypt.checkpw(password.encode('utf-8'), hashed.encode('utf-8'))
    except Exception:
        return False


def is_password_in_history(password: str, history: list[str]) -> bool:
    """Check if password was used recently
    
    Per GAP-04: History = 3 previous passwords
    """
    for old_hash in history[-3:]:  # Check last 3
        if verify_password(password, old_hash):
            return True
    return False
