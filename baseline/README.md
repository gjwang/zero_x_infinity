# Regression Testing Baselines

These files represent the known correct state of the engine after processing specific datasets.
They are used to verify that future changes do not break correctness or consistency.

## Datasets

- **100k**: Standard 100,000 order dataset (fixtures/orders.csv)
- **1.3m**: High-frequency dataset with high balance and 300,000 cancels (fixtures/test_with_cancel_highbal)

## Content

- **orders_final.csv**: Final state of all orders (status, filled_qty, etc.)
- **trades.csv**: All trade events generated during matching.
- **balances_final.csv**: Final available and frozen balances for all users.
