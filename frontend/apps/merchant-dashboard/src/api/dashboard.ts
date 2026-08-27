import { http } from '@/utils/request';

export interface DashboardStats {
    total_volume_usdt: string;
    today_volume_usdt: string;
    total_transactions: number;
    total_transactions_today: number;
}

export function queryDashboardStats() {
    return http.get<DashboardStats>('/api/internal/merchants/stats');
}
