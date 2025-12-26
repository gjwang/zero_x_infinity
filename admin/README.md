# Admin Dashboard

FastAPI-based Admin Dashboard for Zero X Infinity exchange.

## Quick Start

```bash
cd admin
source venv/bin/activate
uvicorn main:app --port 8001 --reload
```

## Testing

### Unit Tests (No Server)
```bash
./run_tests.sh
# or
pytest tests/ -m "not e2e" --ignore=tests/e2e -v
```

### Full Verification
```bash
./verify_all.sh
```

### Scripts

| Script | Purpose |
|--------|---------|
| `run_tests.sh` | Unit tests only |
| `verify_all.sh` | Full verification (unit + E2E) |

### CI Integration
```bash
scripts/test_admin_e2e_ci.sh  # For CI pipelines
```

## API Design

### Status Field (UX-08)

**Input**: String only
- Asset: `"ACTIVE"` / `"DISABLED"`
- Symbol: `"ONLINE"` / `"OFFLINE"` / `"CLOSE_ONLY"`

**Output**: String (via `field_serializer`)

**Integer input**: Rejected with error `"Status must be a string..."`

## Directory Structure

```
admin/
├── admin/          # CRUD admin classes
├── auth/           # Authentication
├── models/         # SQLAlchemy models
├── schemas/        # Pydantic schemas
├── tests/          # Unit tests
│   └── e2e/        # End-to-end tests
├── main.py         # App entry point
├── run_tests.sh    # Unit test runner
└── verify_all.sh   # Full verification
```
