# Regression Testing Baselines

These files represent the known correct state of the engine after processing specific datasets.
They are used to verify that future changes do not break correctness or consistency.

## Datasets

- **100k**: Standard 100,000 order dataset (fixtures/orders.csv)
- **1.3m**: High-frequency dataset with high balance and 300,000 cancels (fixtures/test_with_cancel_highbal)

## Content

- **orders_final.csv**: `order_id,user_id,side,price,qty,filled_qty,status`
- **trades.csv**: `user_id,asset_id,event_type,version,source_type,source_id,delta,avail_after,frozen_after`
- **balances_final.csv**: `user_id,asset_id,avail,frozen,version`

## Usage

These are **authoritative** outputs generated via `pipeline_st` mode.

1. **Automatic Check**: Run `./scripts/test_pipeline_compare.sh 100k`
2. **Manual Check**: `diff output/t2_balances_final.csv baseline/default/t2_balances_final.csv`

> [!WARNING]
> DO NOT update these files manually. Use `./scripts/generate_baseline.sh <dataset> --force` if you have verified the engine changes are correct and represent the new Ground Truth.
