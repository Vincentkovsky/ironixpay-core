//! Email Templates
//!
//! Contains HTML templates for system emails.

use super::EnterpriseLeadNotification;

fn escape_html(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#x27;".to_string(),
            _ => character.to_string(),
        })
        .collect()
}

fn optional_value(value: Option<&str>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .map(escape_html)
        .unwrap_or_else(|| "Not provided".to_string())
}

/// Generate the internal notification for a website enterprise inquiry.
pub fn enterprise_lead_notification_html(lead: &EnterpriseLeadNotification) -> String {
    let networks = if lead.networks.is_empty() {
        "Not specified".to_string()
    } else {
        escape_html(&lead.networks.join(", "))
    };
    let needs = if lead.integration_needs.is_empty() {
        "Not specified".to_string()
    } else {
        escape_html(&lead.integration_needs.join(", "))
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="margin:0;padding:0;background:#f4f7f9;color:#182230;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <div style="padding:32px 16px;">
    <div style="max-width:640px;margin:0 auto;background:#ffffff;border:1px solid #dfe7ee;border-radius:8px;overflow:hidden;">
      <div style="padding:24px 28px;background:#10251d;color:#ffffff;">
        <div style="font-size:13px;font-weight:700;color:#7ee2b8;text-transform:uppercase;">Enterprise inquiry</div>
        <h1 style="margin:6px 0 0;font-size:24px;line-height:1.3;">{company_name}</h1>
      </div>
      <div style="padding:28px;">
        <table role="presentation" style="width:100%;border-collapse:collapse;font-size:14px;line-height:1.5;">
          <tr><td style="width:170px;padding:9px 0;color:#667085;">Lead ID</td><td style="padding:9px 0;font-weight:600;">{lead_id}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Work email</td><td style="padding:9px 0;"><a href="mailto:{contact_email}" style="color:#176b4d;">{contact_email}</a></td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Telegram</td><td style="padding:9px 0;">{telegram}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Website</td><td style="padding:9px 0;">{company_website}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Business type</td><td style="padding:9px 0;">{business_type}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Monthly volume</td><td style="padding:9px 0;">{monthly_volume}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Networks</td><td style="padding:9px 0;">{networks}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Integration needs</td><td style="padding:9px 0;">{needs}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Locale</td><td style="padding:9px 0;">{locale}</td></tr>
          <tr><td style="padding:9px 0;color:#667085;">Submitted</td><td style="padding:9px 0;">{submitted_at}</td></tr>
        </table>
        <div style="margin-top:22px;padding:18px;background:#f7faf9;border-left:3px solid #176b4d;border-radius:0 6px 6px 0;">
          <div style="margin-bottom:6px;color:#667085;font-size:12px;font-weight:700;text-transform:uppercase;">Additional context</div>
          <div style="white-space:pre-wrap;font-size:14px;line-height:1.6;">{message}</div>
        </div>
      </div>
    </div>
  </div>
</body>
</html>"#,
        company_name = escape_html(&lead.company_name),
        lead_id = escape_html(&lead.lead_id),
        contact_email = escape_html(&lead.contact_email),
        telegram = optional_value(lead.telegram.as_deref()),
        company_website = optional_value(lead.company_website.as_deref()),
        business_type = escape_html(&lead.business_type),
        monthly_volume = escape_html(&lead.monthly_volume),
        networks = networks,
        needs = needs,
        locale = escape_html(&lead.locale),
        submitted_at = escape_html(&lead.submitted_at),
        message = optional_value(lead.message.as_deref()),
    )
}

/// Generate email verification HTML
pub fn verification_email_html(
    merchant_name: &str,
    base_url: &str,
    verification_token: &str,
) -> String {
    let verification_link = format!("{}/verify-email?token={}", base_url, verification_token);

    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <style>
                body {{ margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; background-color: #f5f5f5; }}
                .wrapper {{ padding: 40px 20px; }}
                .container {{ max-width: 560px; margin: 0 auto; background: #ffffff; border-radius: 12px; box-shadow: 0 2px 12px rgba(0,0,0,0.06); overflow: hidden; }}
                .header {{ background: linear-gradient(135deg, #1d2939, #2563eb); padding: 32px 40px; text-align: center; }}
                .logo {{ font-size: 26px; font-weight: 700; color: white; letter-spacing: -0.5px; }}
                .logo-pay {{ color: #93c5fd; }}
                .body {{ padding: 36px 40px; }}
                .greeting {{ font-size: 18px; font-weight: 600; color: #1d2939; margin: 0 0 16px; }}
                .text {{ font-size: 15px; color: #4b5563; margin: 0 0 24px; }}
                .btn-wrap {{ text-align: center; margin: 28px 0; }}
                .button {{ display: inline-block; background: #2563eb; color: #ffffff !important; padding: 14px 36px; text-decoration: none; border-radius: 8px; font-weight: 600; font-size: 15px; }}
                .expire {{ font-size: 14px; color: #6b7280; margin: 0 0 16px; }}
                .divider {{ border: 0; border-top: 1px solid #e5e7eb; margin: 28px 0; }}
                .footer {{ padding: 24px 40px; background: #f9fafb; }}
                .link-text {{ font-size: 13px; color: #9ca3af; margin: 0 0 8px; }}
                .link {{ word-break: break-all; color: #6b7280; font-size: 12px; }}
                .team {{ font-size: 14px; color: #6b7280; margin: 16px 0 0; }}
            </style>
        </head>
        <body>
            <div class="wrapper">
                <div class="container">
                    <div class="header">
                        <div class="logo">Ironix<span class="logo-pay">Pay</span></div>
                    </div>
                    <div class="body">
                        <p class="greeting">Hi {merchant_name},</p>
                        <p class="text">Welcome to IronixPay! Please verify your email address to activate your merchant account and start accepting crypto payments.</p>
                        <div class="btn-wrap">
                            <a href="{verification_link}" class="button">Verify Email Address</a>
                        </div>
                        <p class="expire">This link will expire in <strong>24 hours</strong>.</p>
                        <p class="text" style="margin-bottom: 0;">If you didn't create an account, you can safely ignore this email.</p>
                    </div>
                    <div class="footer">
                        <p class="link-text">If the button doesn't work, copy and paste this link into your browser:</p>
                        <p class="link">{verification_link}</p>
                        <p class="team">— The IronixPay Team</p>
                    </div>
                </div>
            </div>
        </body>
        </html>
        "#,
        merchant_name = merchant_name,
        verification_link = verification_link
    )
}

/// Generate password reset HTML
pub fn password_reset_email_html(merchant_name: &str, base_url: &str, reset_token: &str) -> String {
    let reset_link = format!("{}/reset-password?token={}", base_url, reset_token);

    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <style>
                body {{ margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; background-color: #f5f5f5; }}
                .wrapper {{ padding: 40px 20px; }}
                .container {{ max-width: 560px; margin: 0 auto; background: #ffffff; border-radius: 12px; box-shadow: 0 2px 12px rgba(0,0,0,0.06); overflow: hidden; }}
                .header {{ background: linear-gradient(135deg, #1d2939, #2563eb); padding: 32px 40px; text-align: center; }}
                .logo {{ font-size: 26px; font-weight: 700; color: white; letter-spacing: -0.5px; }}
                .logo-pay {{ color: #93c5fd; }}
                .body {{ padding: 36px 40px; }}
                .greeting {{ font-size: 18px; font-weight: 600; color: #1d2939; margin: 0 0 16px; }}
                .text {{ font-size: 15px; color: #4b5563; margin: 0 0 24px; }}
                .btn-wrap {{ text-align: center; margin: 28px 0; }}
                .button {{ display: inline-block; background: #2563eb; color: #ffffff !important; padding: 14px 36px; text-decoration: none; border-radius: 8px; font-weight: 600; font-size: 15px; }}
                .warning {{ background: #fef3c7; border-left: 4px solid #f59e0b; padding: 16px; margin: 24px 0; border-radius: 0 8px 8px 0; font-size: 14px; color: #92400e; }}
                .expire {{ font-size: 14px; color: #6b7280; margin: 0 0 16px; }}
                .divider {{ border: 0; border-top: 1px solid #e5e7eb; margin: 28px 0; }}
                .footer {{ padding: 24px 40px; background: #f9fafb; }}
                .link-text {{ font-size: 13px; color: #9ca3af; margin: 0 0 8px; }}
                .link {{ word-break: break-all; color: #6b7280; font-size: 12px; }}
                .team {{ font-size: 14px; color: #6b7280; margin: 16px 0 0; }}
            </style>
        </head>
        <body>
            <div class="wrapper">
                <div class="container">
                    <div class="header">
                        <div class="logo">Ironix<span class="logo-pay">Pay</span></div>
                    </div>
                    <div class="body">
                        <p class="greeting">Hi {merchant_name},</p>
                        <p class="text">We received a request to reset your IronixPay password.</p>
                        <div class="btn-wrap">
                            <a href="{reset_link}" class="button">Reset Password</a>
                        </div>
                        <p class="expire">This link will expire in <strong>1 hour</strong> for security reasons.</p>
                        <div class="warning">
                            <strong>⚠️ Security Notice:</strong> If you didn't request a password reset, please ignore this email. Your password will remain unchanged.
                        </div>
                    </div>
                    <div class="footer">
                        <p class="link-text">If the button doesn't work, copy and paste this link into your browser:</p>
                        <p class="link">{reset_link}</p>
                        <p class="team">— The IronixPay Team</p>
                    </div>
                </div>
            </div>
        </body>
        </html>
        "#,
        merchant_name = merchant_name,
        reset_link = reset_link
    )
}

/// Generate team invitation HTML
pub fn invitation_email_html(
    inviter_name: &str,
    org_name: &str,
    role: &str,
    invite_link: &str,
) -> String {
    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <style>
                body {{ margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; background-color: #f5f5f5; }}
                .wrapper {{ padding: 40px 20px; }}
                .container {{ max-width: 560px; margin: 0 auto; background: #ffffff; border-radius: 12px; box-shadow: 0 2px 12px rgba(0,0,0,0.06); overflow: hidden; }}
                .header {{ background: linear-gradient(135deg, #1d2939, #2563eb); padding: 32px 40px; text-align: center; }}
                .logo {{ font-size: 26px; font-weight: 700; color: white; letter-spacing: -0.5px; }}
                .logo-pay {{ color: #93c5fd; }}
                .body {{ padding: 36px 40px; }}
                .greeting {{ font-size: 18px; font-weight: 600; color: #1d2939; margin: 0 0 16px; }}
                .text {{ font-size: 15px; color: #4b5563; margin: 0 0 24px; }}
                .highlight {{ background: #f0f9ff; border-left: 4px solid #2563eb; padding: 16px; margin: 24px 0; border-radius: 0 8px 8px 0; }}
                .highlight-label {{ font-size: 13px; color: #6b7280; margin: 0 0 4px; text-transform: uppercase; letter-spacing: 0.5px; }}
                .highlight-value {{ font-size: 16px; font-weight: 600; color: #1d2939; margin: 0; }}
                .btn-wrap {{ text-align: center; margin: 28px 0; }}
                .button {{ display: inline-block; background: #2563eb; color: #ffffff !important; padding: 14px 36px; text-decoration: none; border-radius: 8px; font-weight: 600; font-size: 15px; }}
                .expire {{ font-size: 14px; color: #6b7280; margin: 0 0 16px; }}
                .footer {{ padding: 24px 40px; background: #f9fafb; }}
                .link-text {{ font-size: 13px; color: #9ca3af; margin: 0 0 8px; }}
                .link {{ word-break: break-all; color: #6b7280; font-size: 12px; }}
                .team {{ font-size: 14px; color: #6b7280; margin: 16px 0 0; }}
            </style>
        </head>
        <body>
            <div class="wrapper">
                <div class="container">
                    <div class="header">
                        <div class="logo">Ironix<span class="logo-pay">Pay</span></div>
                    </div>
                    <div class="body">
                        <p class="greeting">You've been invited!</p>
                        <p class="text"><strong>{inviter_name}</strong> has invited you to join their organization on IronixPay.</p>
                        <div class="highlight">
                            <p class="highlight-label">Organization</p>
                            <p class="highlight-value">{org_name}</p>
                            <p class="highlight-label" style="margin-top: 12px;">Your Role</p>
                            <p class="highlight-value" style="text-transform: capitalize;">{role}</p>
                        </div>
                        <div class="btn-wrap">
                            <a href="{invite_link}" class="button">Accept Invitation</a>
                        </div>
                        <p class="expire">This invitation will expire in <strong>24 hours</strong>.</p>
                        <p class="text" style="margin-bottom: 0;">If you don't have an IronixPay account, you'll be asked to create one first.</p>
                    </div>
                    <div class="footer">
                        <p class="link-text">If the button doesn't work, copy and paste this link into your browser:</p>
                        <p class="link">{invite_link}</p>
                        <p class="team">— The IronixPay Team</p>
                    </div>
                </div>
            </div>
        </body>
        </html>
        "#,
        inviter_name = inviter_name,
        org_name = org_name,
        role = role,
        invite_link = invite_link
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_notification_escapes_submitted_html() {
        let lead = EnterpriseLeadNotification {
            lead_id: "lead_test".to_string(),
            company_name: "<script>alert(1)</script>".to_string(),
            company_website: None,
            contact_email: "ops@example.com".to_string(),
            telegram: None,
            business_type: "ecommerce".to_string(),
            monthly_volume: "50k_250k".to_string(),
            networks: vec!["tron".to_string()],
            integration_needs: vec!["checkout".to_string()],
            message: Some("<img src=x onerror=alert(1)>".to_string()),
            locale: "en".to_string(),
            submitted_at: "2026-08-10T00:00:00Z".to_string(),
        };

        let html = enterprise_lead_notification_html(&lead);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }
}
