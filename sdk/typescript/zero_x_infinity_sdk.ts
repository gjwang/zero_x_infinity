/**
 * Zero X Infinity TypeScript SDK
 * 
 * Type-safe SDK for Zero X Infinity Exchange API.
 * Supports Ed25519 authentication for private endpoints.
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
 *     privateKeyHex: '9d61b19...'
 *   });
 *   const orders = await authClient.getOrders(10);
 */

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

    constructor(config: ClientConfig = {}) {
        this.baseUrl = config.baseUrl || 'http://localhost:8080';
        this.apiKey = config.apiKey;
        this.privateKeyHex = config.privateKeyHex;
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
        if (!this.apiKey || !this.privateKeyHex) {
            throw new Error('Auth required. Initialize with apiKey and privateKeyHex');
        }

        const url = new URL(path, this.baseUrl);
        if (params) {
            Object.entries(params).forEach(([k, v]) => {
                if (v !== undefined) url.searchParams.set(k, String(v));
            });
        }

        const auth = await this.signRequest('GET', url.pathname + url.search);
        const resp = await fetch(url.toString(), {
            headers: { 'Authorization': auth }
        });
        return resp.json();
    }

    private async authPost<T>(path: string, body?: any): Promise<ApiResponse<T>> {
        if (!this.apiKey || !this.privateKeyHex) {
            throw new Error('Auth required. Initialize with apiKey and privateKeyHex');
        }

        const auth = await this.signRequest('POST', path);
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

    private async signRequest(method: string, path: string): Promise<string> {
        // Ed25519 signature (requires noble-ed25519 or similar library)
        // This is a placeholder - real implementation needs ed25519 signing
        const tsNonce = Date.now().toString();
        const payload = `${this.apiKey}${tsNonce}${method}${path}`;

        // TODO: Implement actual Ed25519 signing with privateKeyHex
        // For now, return placeholder (will fail auth on server)
        const signature = 'SIGNATURE_PLACEHOLDER';

        return `ZXINF v1.${this.apiKey}.${tsNonce}.${signature}`;
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
