import { http } from '@/utils/request';

// === Types ===

export interface TeamMember {
    id: string;
    user_id: string | null;
    email: string;
    name: string;
    role: string;
    status: string; // 'active' | 'pending'
    invited_at: string | null;
    accepted_at: string | null;
}

export interface TeamMembersResponse {
    members: TeamMember[];
}

// === API Functions ===

/** POST /api/internal/team/invite */
export function inviteMember(data: { email: string; role: string }) {
    return http.post<{ success: boolean; message: string }>(
        '/api/internal/team/invite',
        data,
    );
}

/** GET /api/internal/team/members */
export function listMembers() {
    return http.get<TeamMembersResponse>('/api/internal/team/members');
}

/** PUT /api/internal/team/members/:id/role */
export function changeMemberRole(memberId: string, role: string) {
    return http.put<{ success: boolean; message: string }>(
        `/api/internal/team/members/${memberId}/role`,
        { role },
    );
}

/** DELETE /api/internal/team/members/:id */
export function removeMember(memberId: string) {
    return http.delete<{ success: boolean; message: string }>(
        `/api/internal/team/members/${memberId}`,
    );
}

/** POST /api/internal/merchants/accept-invite */
export function acceptInvite(token: string) {
    return http.post<{ success: boolean; message: string }>(
        '/api/internal/merchants/accept-invite',
        { invite_token: token },
    );
}
