# Gateway API Usage

## Starting the Gateway

```bash
# Start Gateway with Trading Core
cargo run --release -- --gateway --port 8080

# The system will:
# 1. Start HTTP server on port 8080
# 2. Start Trading Core in main thread
# 3. Connect via Ring Buffer (same process)
```

## API Endpoints

### Create Order

**Endpoint**: `POST /api/v1/create_order`

**Headers**:
- `Content-Type: application/json`
- `X-User-ID: <user_id>` (required)

**Request Body**:
```json
{
  "symbol": "BTC_USDT",
  "side": "BUY",           // "BUY" or "SELL"
  "order_type": "LIMIT",   // "LIMIT" or "MARKET"
  "price": "85000.00",     // Required for LIMIT orders
  "qty": "0.001",          // Required
  "cid": "client-order-1"  // Optional client order ID
}
```

**Response** (202 Accepted):
```json
{
  "order_id": 1,
  "cid": "client-order-1",
  "status": "ACCEPTED",
  "accepted_at": 1702944000000
}
```

### Cancel Order

**Endpoint**: `POST /api/v1/cancel_order`

**Headers**:
- `Content-Type: application/json`
- `X-User-ID: <user_id>` (required)

**Request Body**:
```json
{
  "order_id": 1
}
```

**Response** (200 OK):
```json
{
  "order_id": 1,
  "status": "CANCEL_PENDING",
  "accepted_at": 1702944000000
}
```

## Error Responses

**401 Unauthorized** - Missing X-User-ID:
```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "Missing X-User-ID header"
  }
}
```

**400 Bad Request** - Invalid parameters:
```json
{
  "error": {
    "code": "INVALID_PARAMETER",
    "message": "Side must be BUY or SELL"
  }
}
```

**503 Service Unavailable** - Queue full:
```json
{
  "error": {
    "code": "SERVICE_UNAVAILABLE",
    "message": "Order queue is full, please try again later"
  }
}
```

## Testing

```bash
# Run test script
./scripts/test_gateway_simple.sh

# Or manual curl test
curl -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{"symbol":"BTC_USDT","side":"BUY","order_type":"LIMIT","price":"85000.00","qty":"0.001"}'
```

## Design Notes

- **Type Safety**: Uses `rust_decimal` for precise decimal handling
- **Async Processing**: Returns 202 Accepted immediately, processing continues asynchronously
- **Ring Buffer**: Same-process communication for lowest latency
- **No Blocking**: HTTP handlers never block on trading core operations
