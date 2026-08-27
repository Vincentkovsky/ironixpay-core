/**
 * Chain-agnostic payment detection poller.
 *
 * Frontend "radar" for USDT transfers — detects tx_hash and submits
 * to backend's notify-payment endpoint. The Indexer remains the sole
 * authority for confirming payments.
 *
 * Zero hardcoded URLs: all RPC endpoints come from SessionResponse.
 */

// ── Types ──

export interface BlockCursor {
    fromBlock: string | null;
}

export interface PollerConfig {
    /** "tron" | "evm" — from SessionResponse.chain_family */
    chainFamily: string;
    /** Public RPC URL from SessionResponse.detection_rpc_url */
    detectionRpcUrl: string;
    /** Session pay address */
    payAddress: string;
    /** Session created_at ISO string (for TRON timestamp filter) */
    sinceTimestamp: string;
    /** USDT contract address from SessionResponse.currency_contract */
    usdtContract: string;
    /** Mutable EVM block cursor (persisted across poll cycles) */
    evmCursor: BlockCursor;
}

/**
 * Create a PollerConfig from session response data.
 * Returns null if the session lacks detection fields (shouldn't happen for active sessions).
 */
export function createPollerConfig(session: Record<string, any>): PollerConfig | null {
    if (!session.detection_rpc_url || !session.chain_family || !session.currency_contract) {
        return null;
    }
    return {
        chainFamily: session.chain_family,
        detectionRpcUrl: session.detection_rpc_url,
        payAddress: session.pay_address,
        sinceTimestamp: session.created_at,
        usdtContract: session.currency_contract,
        evmCursor: { fromBlock: null },
    };
}

// ── Shared RPC helper ──

async function rpcCall(url: string, method: string, params: any[]): Promise<any> {
    try {
        const res = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ jsonrpc: '2.0', method, params, id: 1 }),
        });
        if (!res.ok) return null;
        const json = await res.json();
        return json.result ?? null;
    } catch {
        return null;
    }
}

// ── TRON: TronGrid account-level TRC20 query ──

async function pollTron(
    rpcUrl: string,
    address: string,
    sinceTs: number,
    usdtContract: string,
): Promise<string | null> {
    try {
        const cleanUrl = rpcUrl.replace(/\/$/, '');
        const url =
            `${cleanUrl}/v1/accounts/${address}/transactions/trc20` +
            `?limit=5&only_to=true&min_timestamp=${sinceTs}&order_by=block_timestamp,desc` +
            `&contract_address=${usdtContract}`;
        const res = await fetch(url);
        if (!res.ok) return null;
        const data = await res.json();
        return data.data?.[0]?.transaction_id ?? null;
    } catch {
        return null;
    }
}

// ── EVM: eth_getLogs with precise sliding window ──

const TRANSFER_TOPIC =
    '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

async function pollEvm(
    rpcUrl: string,
    usdtContract: string,
    payAddress: string,
    cursor: BlockCursor,
): Promise<string | null> {
    // 1. Lock current block height FIRST (prevents race condition gap)
    const toBlockHex = await rpcCall(rpcUrl, 'eth_blockNumber', []);
    if (!toBlockHex) return null;
    const toBlock = parseInt(toBlockHex, 16);

    // 2. Initialize or cap cursor
    if (!cursor.fromBlock) {
        cursor.fromBlock = '0x' + Math.max(0, toBlock - 10).toString(16);
    }
    // Cap range to 500 blocks (prevents public RPC rejection after long idle)
    const fromBlock = parseInt(cursor.fromBlock, 16);
    if (toBlock - fromBlock > 500) {
        cursor.fromBlock = '0x' + (toBlock - 500).toString(16);
    }

    // 3. Query with explicit [fromBlock, toBlock] — no 'latest'
    const paddedAddr =
        '0x' + payAddress.replace('0x', '').toLowerCase().padStart(64, '0');
    const logs = await rpcCall(rpcUrl, 'eth_getLogs', [
        {
            fromBlock: cursor.fromBlock,
            toBlock: toBlockHex,
            address: usdtContract,
            topics: [TRANSFER_TOPIC, null, paddedAddr],
        },
    ]);

    // 4. Always advance cursor to toBlock + 1 (gap-free)
    cursor.fromBlock = '0x' + (toBlock + 1).toString(16);

    if (logs && logs.length > 0) {
        return logs[logs.length - 1].transactionHash ?? null;
    }
    return null;
}

// ── Dispatcher ──

/**
 * Poll the appropriate chain for a USDT transfer to the session's pay_address.
 * Returns the tx_hash if found, null otherwise.
 */
export async function pollChainForTransfer(
    config: PollerConfig,
): Promise<string | null> {
    switch (config.chainFamily) {
        case 'tron':
            return pollTron(
                config.detectionRpcUrl,
                config.payAddress,
                new Date(config.sinceTimestamp).getTime(),
                config.usdtContract,
            );
        case 'evm':
            return pollEvm(
                config.detectionRpcUrl,
                config.usdtContract,
                config.payAddress,
                config.evmCursor,
            );
        default:
            return null;
    }
}
