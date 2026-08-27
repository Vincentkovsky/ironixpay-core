//! Team Management Routes
//!
//! Handles team member operations: invite, list, change role, remove.
//! All endpoints require JWT auth (mounted under /api/internal/team).

use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use std::str::FromStr;
use tracing::info;
use validator::Validate;

use crate::api::dtos::auth::SuccessResponse;
use crate::api::dtos::team::{
    ChangeRoleRequest, InviteRequest, TeamMemberResponse, TeamMembersListResponse,
};
use crate::api::error::AppError;
use crate::api::middleware::auth::{require_role, AuthenticatedMerchant};
use crate::entity::{
    org_members,
    org_members::{MemberRole, MemberStatus},
    users,
};
use crate::services::AppState;

/// Team management router.
/// Mounted at `/api/internal/team` (within JWT-auth group).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/invite", post(invite_member))
        .route("/members", get(list_members))
        .route("/members/:id/role", put(change_role))
        .route("/members/:id", delete(remove_member))
}

/// Helper: parse role string into MemberRole, returning AppError on failure.
fn parse_role(role: &str) -> Result<MemberRole, AppError> {
    MemberRole::from_str(role).map_err(|_| AppError::ValidationError {
        code: "INVALID_ROLE",
        message: format!(
            "Invalid role '{}'. Must be one of: owner, admin, developer, finance, viewer",
            role
        ),
        param: Some("role".into()),
    })
}

/// Helper: convert MemberRole enum to display string.
fn role_to_str(role: &MemberRole) -> &'static str {
    match role {
        MemberRole::Owner => "owner",
        MemberRole::Admin => "admin",
        MemberRole::Developer => "developer",
        MemberRole::Finance => "finance",
        MemberRole::Viewer => "viewer",
    }
}

/// Helper: convert MemberStatus enum to display string.
fn status_to_str(status: &MemberStatus) -> &'static str {
    match status {
        MemberStatus::Active => "active",
        MemberStatus::Pending => "pending",
        MemberStatus::Disabled => "disabled",
    }
}

// =============================================================
// Handlers
// =============================================================

/// POST /api/internal/team/invite
///
/// Invite a new member to the organization.
/// If a pending invitation for this email already exists, re-sends the invitation.
async fn invite_member(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<InviteRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    // Role guard: Owner, Admin can invite
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;
    body.validate()?;

    let email = body.email.trim().to_lowercase();
    let role = parse_role(&body.role)?;

    // Cannot invite an owner (only the system sets owner during registration)
    if role == MemberRole::Owner {
        return Err(AppError::ValidationError {
            code: "CANNOT_INVITE_OWNER",
            message: "Cannot invite members with Owner role. Owner is assigned during organization creation only."
                .into(),
            param: Some("role".into()),
        });
    }

    let db = &state.db;

    // Check if email is already an active member of this org (by invited_email)
    let existing_active = org_members::Entity::find()
        .filter(org_members::Column::OrgId.eq(&merchant.id))
        .filter(org_members::Column::InvitedEmail.eq(&email))
        .filter(org_members::Column::Status.eq(MemberStatus::Active))
        .one(db)
        .await?;

    if existing_active.is_some() {
        return Err(AppError::ValidationError {
            code: "ALREADY_MEMBER",
            message: "This email is already an active member of this organization.".into(),
            param: Some("email".into()),
        });
    }

    // Also check by user_id (active member who registered with this email)
    let existing_user = users::Entity::find()
        .filter(users::Column::Email.eq(&email))
        .one(db)
        .await?;

    if let Some(ref user) = existing_user {
        let already_member = org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(&merchant.id))
            .filter(org_members::Column::UserId.eq(Some(user.id.clone())))
            .filter(org_members::Column::Status.eq(MemberStatus::Active))
            .one(db)
            .await?;

        if already_member.is_some() {
            return Err(AppError::ValidationError {
                code: "ALREADY_MEMBER",
                message: "This email is already an active member of this organization.".into(),
                param: Some("email".into()),
            });
        }
    }

    // Check if there's already a pending invitation — re-invite
    let existing_pending = org_members::Entity::find()
        .filter(org_members::Column::OrgId.eq(&merchant.id))
        .filter(org_members::Column::InvitedEmail.eq(&email))
        .filter(org_members::Column::Status.eq(MemberStatus::Pending))
        .one(db)
        .await?;

    let member_id = if let Some(pending) = existing_pending {
        // Update existing pending invitation
        let mid = pending.id.clone();
        let mut active_model: org_members::ActiveModel = pending.into();
        active_model.role = Set(role.clone());
        active_model.invited_at = Set(Some(chrono::Utc::now().into()));
        active_model.invited_by = Set(Some(merchant.user_id.clone()));
        active_model.update(db).await?;

        info!(
            org_id = %merchant.id,
            email = %email,
            "Re-sent team invitation (updated existing pending)"
        );
        mid
    } else {
        // Create new pending org_member
        let member_id = format!("om_{}", uuid::Uuid::new_v4());

        let new_member = org_members::ActiveModel {
            id: Set(member_id.clone()),
            org_id: Set(merchant.id.clone()),
            user_id: Set(None), // NULL until accepted
            invited_email: Set(Some(email.clone())),
            role: Set(role.clone()),
            status: Set(MemberStatus::Pending),
            invited_by: Set(Some(merchant.user_id.clone())),
            invited_at: Set(Some(chrono::Utc::now().into())),
            accepted_at: Set(None),
            ..Default::default()
        };

        org_members::Entity::insert(new_member)
            .exec(db)
            .await
            .map_err(|e| AppError::InternalServerError(e.into()))?;

        info!(
            org_id = %merchant.id,
            email = %email,
            member_id = %member_id,
            "Created new team invitation"
        );
        member_id
    };

    // Generate invite JWT
    let invite_token =
        state
            .merchant_service
            .generate_invite_token(&member_id, &merchant.id, &email)?;

    // Fetch inviter name for the email
    let inviter = users::Entity::find_by_id(&merchant.user_id)
        .one(db)
        .await?
        .map(|u| u.name)
        .unwrap_or_else(|| "A team member".to_string());

    // Fetch org name
    let org = crate::entity::merchants::Entity::find_by_id(&merchant.id)
        .one(db)
        .await?
        .map(|m| m.name)
        .unwrap_or_else(|| "Organization".to_string());

    // Build invite link (dashboard handles the accept flow)
    let invite_link = format!(
        "{}/accept-invite?token={}",
        state.config.dashboard_base_url, invite_token
    );

    let role_display = role_to_str(&role).to_string();

    // Send invitation email (non-blocking, don't fail the request)
    if let Some(email_sender) = &state.merchant_service.get_email_sender() {
        let email_clone = email.clone();
        let email_sender = email_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = email_sender
                .send_invitation_email(&email_clone, &inviter, &org, &role_display, &invite_link)
                .await
            {
                tracing::error!(error = %e, email = %email_clone, "Failed to send invitation email");
            }
        });
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Invitation sent to {}", email),
    }))
}

/// GET /api/internal/team/members
///
/// List all team members (active + pending).
async fn list_members(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<TeamMembersListResponse>, AppError> {
    // Role guard: Owner, Admin can view team
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let db = &state.db;

    // Fetch all org_members (active + pending, not disabled)
    let members = org_members::Entity::find()
        .filter(org_members::Column::OrgId.eq(&merchant.id))
        .filter(org_members::Column::Status.ne(MemberStatus::Disabled))
        .order_by_asc(org_members::Column::CreatedAt)
        .all(db)
        .await?;

    // Batch load users for active members
    let user_ids: Vec<String> = members.iter().filter_map(|m| m.user_id.clone()).collect();
    let user_map: std::collections::HashMap<String, users::Model> = if !user_ids.is_empty() {
        users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|u| (u.id.clone(), u))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let responses: Vec<TeamMemberResponse> = members
        .into_iter()
        .map(|m| {
            let user = m.user_id.as_ref().and_then(|uid| user_map.get(uid));
            TeamMemberResponse {
                id: m.id,
                user_id: m.user_id.clone(),
                email: user
                    .map(|u| u.email.clone())
                    .or(m.invited_email.clone())
                    .unwrap_or_default(),
                name: user.map(|u| u.name.clone()).unwrap_or_default(),
                role: role_to_str(&m.role).to_string(),
                status: status_to_str(&m.status).to_string(),
                invited_at: m.invited_at.map(|t| t.to_rfc3339()),
                accepted_at: m.accepted_at.map(|t| t.to_rfc3339()),
            }
        })
        .collect();

    Ok(Json(TeamMembersListResponse { members: responses }))
}

/// PUT /api/internal/team/members/:id/role
///
/// Change a member's role. Owner only.
async fn change_role(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(member_id): Path<String>,
    Json(body): Json<ChangeRoleRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    // Role guard: Owner only can change roles
    require_role(&merchant, &[MemberRole::Owner])?;
    body.validate()?;

    let new_role = parse_role(&body.role)?;
    let db = &state.db;

    // Cannot promote to Owner (owner is set during registration only)
    if new_role == MemberRole::Owner {
        return Err(AppError::ValidationError {
            code: "CANNOT_PROMOTE_TO_OWNER",
            message: "Cannot promote a member to Owner. Owner is assigned during organization creation only.".into(),
            param: Some("role".into()),
        });
    }

    // Find the target member
    let member = org_members::Entity::find_by_id(&member_id)
        .filter(org_members::Column::OrgId.eq(&merchant.id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Team member not found".into()))?;

    // Cannot change own role
    if member.user_id.as_deref() == Some(&merchant.user_id) {
        return Err(AppError::ValidationError {
            code: "SELF_ROLE_CHANGE",
            message: "Cannot change your own role.".into(),
            param: None,
        });
    }

    // Owner count protection: if demoting an owner, ensure there's at least 1 other owner
    if member.role == MemberRole::Owner && new_role != MemberRole::Owner {
        let owner_count = org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(&merchant.id))
            .filter(org_members::Column::Role.eq(MemberRole::Owner))
            .filter(org_members::Column::Status.eq(MemberStatus::Active))
            .count(db)
            .await?;

        if owner_count <= 1 {
            return Err(AppError::ValidationError {
                code: "LAST_OWNER",
                message: "Cannot demote the last owner. Promote another member to Owner first."
                    .into(),
                param: None,
            });
        }
    }

    let target_user_id = member.user_id.clone();

    let mut active_model: org_members::ActiveModel = member.into();
    active_model.role = Set(new_role.clone());
    active_model.update(db).await?;

    // Invalidate the member's JWT so the new role takes effect immediately
    if let Some(uid) = &target_user_id {
        if let Some(user) = users::Entity::find_by_id(uid).one(db).await? {
            let mut user_am: users::ActiveModel = user.into();
            user_am.token_version = Set(user_am.token_version.unwrap() + 1);
            user_am.update(db).await?;
        }
    }

    let role_str = role_to_str(&new_role);
    info!(
        org_id = %merchant.id,
        member_id = %member_id,
        new_role = %role_str,
        "Team member role changed (token invalidated)"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Role changed to {}", role_str),
    }))
}

/// DELETE /api/internal/team/members/:id
///
/// Remove a member from the organization.
/// Pending members are hard-deleted. Active members are set to disabled.
async fn remove_member(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(member_id): Path<String>,
) -> Result<Json<SuccessResponse>, AppError> {
    // Role guard: Owner, Admin can remove members
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let db = &state.db;

    // Find the target member
    let member = org_members::Entity::find_by_id(&member_id)
        .filter(org_members::Column::OrgId.eq(&merchant.id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Team member not found".into()))?;

    // Cannot remove self
    if member.user_id.as_deref() == Some(&merchant.user_id) {
        return Err(AppError::ValidationError {
            code: "SELF_REMOVAL",
            message: "Cannot remove yourself from the organization.".into(),
            param: None,
        });
    }

    // Admin cannot remove Owner
    if member.role == MemberRole::Owner && merchant.role != MemberRole::Owner {
        return Err(AppError::PermissionDenied(
            "Only owners can remove other owners.".into(),
        ));
    }

    // Owner count protection
    if member.role == MemberRole::Owner && member.status == MemberStatus::Active {
        let owner_count = org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(&merchant.id))
            .filter(org_members::Column::Role.eq(MemberRole::Owner))
            .filter(org_members::Column::Status.eq(MemberStatus::Active))
            .count(db)
            .await?;

        if owner_count <= 1 {
            return Err(AppError::ValidationError {
                code: "LAST_OWNER",
                message: "Cannot remove the last owner.".into(),
                param: None,
            });
        }
    }

    if member.status == MemberStatus::Pending {
        // Hard delete pending invitations
        org_members::Entity::delete_by_id(&member_id)
            .exec(db)
            .await?;
        info!(
            org_id = %merchant.id,
            member_id = %member_id,
            "Pending invitation revoked (deleted)"
        );
    } else {
        // Disable active members (audit trail)
        let target_user_id = member.user_id.clone();
        let mut active_model: org_members::ActiveModel = member.into();
        active_model.status = Set(MemberStatus::Disabled);
        active_model.update(db).await?;

        // Invalidate removed member's JWT
        if let Some(uid) = &target_user_id {
            if let Some(user) = users::Entity::find_by_id(uid).one(db).await? {
                let mut user_am: users::ActiveModel = user.into();
                user_am.token_version = Set(user_am.token_version.unwrap() + 1);
                user_am.update(db).await?;
            }
        }

        info!(
            org_id = %merchant.id,
            member_id = %member_id,
            "Active team member disabled (token invalidated)"
        );
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: "Member removed successfully".to_string(),
    }))
}
