import { http } from '@/utils/request';

// Analytics API types — mirrors backend AnalyticsResponse
export interface KpiEntry {
    currency: string
    gross_volume: number  // i64 microunits
    net_revenue: number   // i64 microunits
    fee_total: number     // i64 microunits
    tx_count: number
}

export interface TimeSeriesPoint {
    date: string        // "2026-03-25"
    currency: string    // "USDT" | "USDC"
    volume: number      // i64 microunits
    count: number
}

export interface DistributionEntry {
    label: string
    value: number
}

export interface AnalyticsResponse {
    kpis: KpiEntry[]
    time_series: TimeSeriesPoint[]
    network_distribution: DistributionEntry[]
    status_breakdown: DistributionEntry[]
    conversion_rate: number  // 0.0 ~ 1.0
}

export interface AnalyticsQuery {
    start_date?: string   // ISO 8601 UTC
    end_date?: string     // ISO 8601 UTC
    currency?: string     // "USDT" | "USDC"
    include_sub_merchants?: boolean
    sub_merchant_code?: string
}

export function queryAnalytics(params: AnalyticsQuery = {}) {
    return http.get<AnalyticsResponse>('/api/internal/analytics', { params });
}
