use serde::{Deserialize, Serialize};
use validator::Validate;

/// POST /api/internal/team/invite
#[derive(Debug, Deserialize, Validate)]
pub struct InviteRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    /// Role string: "admin" | "developer" | "finance" | "viewer"
    /// (Owner cannot be invited directly)
    pub role: String,
}

/// Response for a single team member
#[derive(Debug, Serialize, Clone)]
pub struct TeamMemberResponse {
    pub id: String,
    /// Set for active members, None for pending invitations
    pub user_id: Option<String>,
    /// For active members: user's email; for pending: invited_email
    pub email: String,
    /// User's display name (empty for pending invitations)
    pub name: String,
    pub role: String,
    /// "active" | "pending" | "disabled"
    pub status: String,
    pub invited_at: Option<String>,
    pub accepted_at: Option<String>,
}

/// GET /api/internal/team/members response
#[derive(Debug, Serialize)]
pub struct TeamMembersListResponse {
    pub members: Vec<TeamMemberResponse>,
}

/// PUT /api/internal/team/members/:id/role
#[derive(Debug, Deserialize, Validate)]
pub struct ChangeRoleRequest {
    /// Role string: "owner" | "admin" | "developer" | "finance" | "viewer"
    pub role: String,
}

/// POST /api/internal/merchants/accept-invite
#[derive(Debug, Deserialize, Validate)]
pub struct AcceptInviteRequest {
    #[validate(length(min = 1, message = "invite_token is required"))]
    pub invite_token: String,
}
