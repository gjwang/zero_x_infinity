#!/usr/bin/env python3
import os
import re
import sys

# Forbidden types (case-insensitive)
# We allow NUMERIC only if it has scale 0 (e.g., NUMERIC(20, 0)) to represent big integers.
# Simple regex to catch bad types.
FORBIDDEN_PATTERNS = [
    (r"DECIMAL\s*\(", "DECIMAL type is forbidden. Use atomic units (Satoshis/Wei) in NUMERIC(X,0), BIGINT, or VARCHAR."),
    (r"FLOAT", "FLOAT type is forbidden. Use Fixed-Point integers."),
    (r"REAL", "REAL type is forbidden. Use Fixed-Point integers."),
    (r"DOUBLE PRECISION", "DOUBLE PRECISION is forbidden. Use Fixed-Point integers."),
]

# Allow strictly NUMERIC(X, 0) or NUMERIC with no scale implied? 
# Usually NUMERIC without scale is arbitary precision, which is okay IF used as integer.
# But explicitly forbids "DECIMAL".
# Let's strictly catch "DECIMAL".

def check_file(filepath):
    errors = []
    with open(filepath, 'r') as f:
        lines = f.readlines()
        
    for i, line in enumerate(lines):
        # Skip comments
        code_line = line.split('--')[0]
        
        for pattern, msg in FORBIDDEN_PATTERNS:
            if re.search(pattern, code_line, re.IGNORECASE):
                errors.append(f"Line {i+1}: Found forbidden pattern '{pattern}'. {msg}\n   Context: {line.strip()}")
                
        # Special check for NUMERIC: must have scale 0
        # e.g. NUMERIC(30, 8) -> Bad
        # NUMERIC(30, 0) -> Good
        # NUMERIC -> Bad (ambiguous?) -> Postgres defaults to 0 scale? No, it allows any. 
        # Let's strictly require explicit (X, 0) or BIGINT.
        
        # Regex for NUMERIC(p, s) where s != 0
        # Matches NUMERIC(30, 8) or NUMERIC(30,8)
        numeric_match = re.search(r"NUMERIC\s*\(\s*\d+\s*,\s*(\d+)\s*\)", code_line, re.IGNORECASE)
        if numeric_match:
            scale = int(numeric_match.group(1))
            if scale != 0:
                 errors.append(f"Line {i+1}: Found NUMERIC with non-zero scale '{scale}'. Use atomic units (scale 0). \n   Context: {line.strip()}")

    return errors

def main():
    migration_dir = os.path.join(os.path.dirname(__file__), "../migrations")
    files = [f for f in os.listdir(migration_dir) if f.endswith('.sql')]
    
    total_errors = 0
    print(f"🔍 Scanning {len(files)} migration files in {migration_dir}...")
    
    for filename in files:
        filepath = os.path.join(migration_dir, filename)
        errors = check_file(filepath)
        if errors:
            print(f"\n❌ Violation in {filename}:")
            for err in errors:
                print(f"   {err}")
            total_errors += len(errors)
            
    if total_errors > 0:
        print(f"\n🚫 FAILED: Found {total_errors} schema violations.")
        sys.exit(1)
    else:
        print("\n✅ PASS: No forbidden types found.")
        sys.exit(0)

if __name__ == "__main__":
    main()
