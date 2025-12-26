/**
 * Zero X Infinity TypeScript SDK
 * 
 * Type-safe SDK for Zero X Infinity Exchange API.
 * Supports Ed25519 authentication for private endpoints.
 * 
 * Requirements:
 *   npm install @noble/ed25519
 * 
 * Usage:
 *   import { ZeroXInfinityClient } from './zero_x_infinity_sdk';
 *   
 *   // Public endpoints (no auth)
 *   const client = new ZeroXInfinityClient();
 *   const depth = await client.getDepth('BTC_USDT', 20);
 *   
 *   // Private endpoints (with auth)
 *   const authClient = new ZeroXInfinityClient({
 *     apiKey: 'AK_0000000000001001',
 *     privateKeyHex: '9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60'
 *   });
 *   const orders = await authClient.getOrders(10);
 */

import * as ed from '@noble/ed25519';

// =============================================================================
// Base62 Encoding (matches Rust server)
// =============================================================================

const ALPHABET = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

function base62Encode(bytes: Uint8Array): string {
    let num = BigInt(0);
    for (const byte of bytes) {
        num = num * 256n + BigInt(byte);
    }
    if (num === 0n) return ALPHABET[0];

    const result: string[] = [];
    while (num > 0n) {
        result.push(ALPHABET[Number(num % 62n)]);
        num = num / 62n;
    }
    return result.reverse().join('');
}

// =============================================================================
// Types
// =============================================================================

export interface ApiResponse<T> {
    code: number;
    msg: string;
    data?: T;
}

export interface HealthResponse {
    timestamp_ms: number;
}

export interface DepthData {
    symbol: string;
    bids: [string, string][];
    asks: [string, string][];
    last_update_id: number;
}

export interface AssetInfo {
    asset_id: number;
    asset: string;
    name: string;
    decimals: number;
    can_deposit: boolean;
    can_withdraw: boolean;
    can_trade: boolean;
}

export interface SymbolInfo {
    symbol_id: number;
    symbol: string;
    base_asset: string;
    quote_asset: string;
    price_decimals: number;
    qty_decimals: number;
    is_tradable: boolean;
    is_visible: boolean;
}

export interface ExchangeInfo {
    assets: AssetInfo[];
    symbols: SymbolInfo[];
    server_time: number;
}

export interface OrderResponse {
    order_id: number;
    cid?: string;
    order_status: string;
    accepted_at: number;
}

export interface ClientConfig {
    baseUrl?: string;
    apiKey?: string;
    privateKeyHex?: string;
}

// =============================================================================
// Client
// =============================================================================

export class ZeroXInfinityClient {
    private baseUrl: string;
    private apiKey?: string;
    private privateKeyHex?: string;
    private lastTsNonce: number = 0;

    constructor(config: ClientConfig = {}) {
        this.baseUrl = config.baseUrl || 'http://localhost:8080';
        this.apiKey = config.apiKey;
        this.privateKeyHex = config.privateKeyHex;
    }

    /**
     * Generate monotonically increasing ts_nonce (prevents replay)
     */
    private getTsNonce(): string {
        const now = Date.now();
        this.lastTsNonce = Math.max(now, this.lastTsNonce + 1);
        return this.lastTsNonce.toString();
    }

    /**
     * Sign request with Ed25519
     * Payload format: {api_key}{ts_nonce}{method}{path}{body}
     */
    private async signRequest(method: string, path: string, body: string = ''): Promise<string> {
        if (!this.apiKey || !this.privateKeyHex) {
            throw new Error('Auth required. Initialize with apiKey and privateKeyHex');
        }

        const tsNonce = this.getTsNonce();
        const payload = `${this.apiKey}${tsNonce}${method}${path}${body}`;

        // Ed25519 sign
        const privateKey = this.privateKeyHex.slice(0, 64); // First 32 bytes (64 hex chars)
        const messageBytes = new TextEncoder().encode(payload);
        const signature = await ed.signAsync(messageBytes, privateKey);
        const sigBase62 = base62Encode(signature);

        return `ZXINF v1.${this.apiKey}.${tsNonce}.${sigBase62}`;
    }

    private async get<T>(path: string, params?: Record<string, any>): Promise<ApiResponse<T>> {
        const url = new URL(path, this.baseUrl);
        if (params) {
            Object.entries(params).forEach(([k, v]) => {
                if (v !== undefined) url.searchParams.set(k, String(v));
            });
        }

        const resp = await fetch(url.toString());
        return resp.json();
    }

    private async authGet<T>(path: string, params?: Record<string, any>): Promise<ApiResponse<T>> {
        const url = new URL(path, this.baseUrl);
        if (params) {
            Object.entries(params).forEach(([k, v]) => {
                if (v !== undefined) url.searchParams.set(k, String(v));
            });
        }

        const signPath = url.pathname + url.search;
        const auth = await this.signRequest('GET', signPath);
        const resp = await fetch(url.toString(), {
            headers: { 'Authorization': auth }
        });
        return resp.json();
    }

    private async authPost<T>(path: string, body?: any): Promise<ApiResponse<T>> {
        // Server uses empty body for signature (matches middleware.rs)
        const auth = await this.signRequest('POST', path, '');
        const resp = await fetch(`${this.baseUrl}${path}`, {
            method: 'POST',
            headers: {
                'Authorization': auth,
                'Content-Type': 'application/json'
            },
            body: body ? JSON.stringify(body) : undefined
        });
        return resp.json();
    }

    // =========================================================================
    // Public Endpoints
    // =========================================================================

    /** GET /api/v1/health */
    async healthCheck(): Promise<ApiResponse<HealthResponse>> {
        return this.get('/api/v1/health');
    }

    /** GET /api/v1/public/depth */
    async getDepth(symbol?: string, limit = 20): Promise<ApiResponse<DepthData>> {
        return this.get('/api/v1/public/depth', { symbol, limit });
    }

    /** GET /api/v1/public/klines */
    async getKlines(interval = '1m', limit = 100): Promise<ApiResponse<any[]>> {
        return this.get('/api/v1/public/klines', { interval, limit });
    }

    /** GET /api/v1/public/assets */
    async getAssets(): Promise<ApiResponse<AssetInfo[]>> {
        return this.get('/api/v1/public/assets');
    }

    /** GET /api/v1/public/symbols */
    async getSymbols(): Promise<ApiResponse<SymbolInfo[]>> {
        return this.get('/api/v1/public/symbols');
    }

    /** GET /api/v1/public/exchange_info */
    async getExchangeInfo(): Promise<ApiResponse<ExchangeInfo>> {
        return this.get('/api/v1/public/exchange_info');
    }

    // =========================================================================
    // Private Endpoints (Auth Required)
    // =========================================================================

    /** POST /api/v1/private/order */
    async createOrder(params: {
        symbol: string;
        side: 'BUY' | 'SELL';
        order_type: 'LIMIT' | 'MARKET';
        qty: string;
        price?: string;
        cid?: string;
    }): Promise<ApiResponse<OrderResponse>> {
        return this.authPost('/api/v1/private/order', params);
    }

    /** POST /api/v1/private/cancel */
    async cancelOrder(orderId: number): Promise<ApiResponse<OrderResponse>> {
        return this.authPost('/api/v1/private/cancel', { order_id: orderId });
    }

    /** GET /api/v1/private/order/:order_id */
    async getOrder(orderId: number): Promise<ApiResponse<any>> {
        return this.authGet(`/api/v1/private/order/${orderId}`);
    }

    /** GET /api/v1/private/orders */
    async getOrders(limit = 10): Promise<ApiResponse<any[]>> {
        return this.authGet('/api/v1/private/orders', { limit });
    }

    /** GET /api/v1/private/trades */
    async getTrades(limit = 100): Promise<ApiResponse<any[]>> {
        return this.authGet('/api/v1/private/trades', { limit });
    }

    /** GET /api/v1/private/balances */
    async getBalances(assetId: number): Promise<ApiResponse<any>> {
        return this.authGet('/api/v1/private/balances', { asset_id: assetId });
    }

    /** GET /api/v1/private/balances/all */
    async getAllBalances(): Promise<ApiResponse<any[]>> {
        return this.authGet('/api/v1/private/balances/all');
    }

    /** POST /api/v1/private/transfer */
    async createTransfer(params: {
        from: string;
        to: string;
        asset: string;
        amount: string;
        cid?: string;
    }): Promise<ApiResponse<any>> {
        return this.authPost('/api/v1/private/transfer', params);
    }

    /** GET /api/v1/private/transfer/:req_id */
    async getTransfer(reqId: string): Promise<ApiResponse<any>> {
        return this.authGet(`/api/v1/private/transfer/${reqId}`);
    }
}

// Export default instance for quick access
export const defaultClient = new ZeroXInfinityClient();
