use chrono::{DateTime, Duration, Utc};
use crypto::{random_token, SecretsService};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum OauthError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("OAuth application not found")]
    AppNotFound,
    #[error("Invalid client secret")]
    InvalidClientSecret,
    #[error("Invalid or expired authorization code")]
    InvalidCode,
    #[error("Invalid redirect URI")]
    InvalidRedirectUri,
    #[error("PKCE verification failed")]
    InvalidPkce,
    #[error("OAuth token not found or revoked")]
    InvalidToken,
    #[error("Application is disabled")]
    AppDisabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthApp {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub client_id: String,
    pub client_secret_prefix: String,
    pub redirect_uris: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthAppCreated {
    #[serde(flatten)]
    pub app: OauthApp,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAuthorizedAppItem {
    pub application_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub granted_scopes: String,
    pub authorized_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthUserInfo {
    pub sub: String,
    pub id: String,
    pub username: String,
    pub email: String,
    pub name: String,
    pub picture: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn create_application(
    pool: &PgPool,
    secrets: &SecretsService,
    user_id: Uuid,
    name: &str,
    description: Option<&str>,
    website_url: Option<&str>,
    logo_url: Option<&str>,
    redirect_uris: Vec<String>,
) -> Result<OauthAppCreated, OauthError> {
    let client_id = format!("sats_app_{}", random_token(16));
    let raw_secret = format!("sats_sec_{}", random_token(24));
    let secret_hash = secrets.hmac_hex(&raw_secret);
    let secret_prefix = format!("{}...", &raw_secret[..12]);

    let row = sqlx::query(
        r#"
        INSERT INTO oauth_applications (
            user_id, name, description, website_url, logo_url,
            client_id, client_secret_hash, client_secret_prefix, redirect_uris
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING
            id, user_id, name, description, website_url, logo_url,
            client_id, client_secret_prefix, redirect_uris, is_active,
            created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(description.map(str::trim))
    .bind(website_url.map(str::trim))
    .bind(logo_url.map(str::trim))
    .bind(&client_id)
    .bind(&secret_hash)
    .bind(&secret_prefix)
    .bind(&redirect_uris)
    .fetch_one(pool)
    .await?;

    let app = OauthApp {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        description: row.get("description"),
        website_url: row.get("website_url"),
        logo_url: row.get("logo_url"),
        client_id: row.get("client_id"),
        client_secret_prefix: row.get("client_secret_prefix"),
        redirect_uris: row.get("redirect_uris"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(OauthAppCreated {
        app,
        client_secret: raw_secret,
    })
}

pub async fn list_user_applications(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<OauthApp>, OauthError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, user_id, name, description, website_url, logo_url,
            client_id, client_secret_prefix, redirect_uris, is_active,
            created_at, updated_at
        FROM oauth_applications
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OauthApp {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            description: r.get("description"),
            website_url: r.get("website_url"),
            logo_url: r.get("logo_url"),
            client_id: r.get("client_id"),
            client_secret_prefix: r.get("client_secret_prefix"),
            redirect_uris: r.get("redirect_uris"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn get_application_by_client_id(
    pool: &PgPool,
    client_id: &str,
) -> Result<OauthApp, OauthError> {
    let row = sqlx::query(
        r#"
        SELECT
            id, user_id, name, description, website_url, logo_url,
            client_id, client_secret_prefix, redirect_uris, is_active,
            created_at, updated_at
        FROM oauth_applications
        WHERE client_id = $1
        "#,
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::AppNotFound)?;

    let is_active: bool = row.get("is_active");
    if !is_active {
        return Err(OauthError::AppDisabled);
    }

    Ok(OauthApp {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        description: row.get("description"),
        website_url: row.get("website_url"),
        logo_url: row.get("logo_url"),
        client_id: row.get("client_id"),
        client_secret_prefix: row.get("client_secret_prefix"),
        redirect_uris: row.get("redirect_uris"),
        is_active,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub async fn update_application(
    pool: &PgPool,
    user_id: Uuid,
    app_id: Uuid,
    name: &str,
    description: Option<&str>,
    website_url: Option<&str>,
    logo_url: Option<&str>,
    redirect_uris: Vec<String>,
) -> Result<OauthApp, OauthError> {
    let row = sqlx::query(
        r#"
        UPDATE oauth_applications
        SET
            name = $3,
            description = $4,
            website_url = $5,
            logo_url = $6,
            redirect_uris = $7,
            updated_at = now()
        WHERE id = $1 AND user_id = $2
        RETURNING
            id, user_id, name, description, website_url, logo_url,
            client_id, client_secret_prefix, redirect_uris, is_active,
            created_at, updated_at
        "#,
    )
    .bind(app_id)
    .bind(user_id)
    .bind(name.trim())
    .bind(description.map(str::trim))
    .bind(website_url.map(str::trim))
    .bind(logo_url.map(str::trim))
    .bind(&redirect_uris)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::AppNotFound)?;

    Ok(OauthApp {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        description: row.get("description"),
        website_url: row.get("website_url"),
        logo_url: row.get("logo_url"),
        client_id: row.get("client_id"),
        client_secret_prefix: row.get("client_secret_prefix"),
        redirect_uris: row.get("redirect_uris"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub async fn rotate_client_secret(
    pool: &PgPool,
    secrets: &SecretsService,
    user_id: Uuid,
    app_id: Uuid,
) -> Result<OauthAppCreated, OauthError> {
    let raw_secret = format!("sats_sec_{}", random_token(24));
    let secret_hash = secrets.hmac_hex(&raw_secret);
    let secret_prefix = format!("{}...", &raw_secret[..12]);

    let row = sqlx::query(
        r#"
        UPDATE oauth_applications
        SET
            client_secret_hash = $3,
            client_secret_prefix = $4,
            updated_at = now()
        WHERE id = $1 AND user_id = $2
        RETURNING
            id, user_id, name, description, website_url, logo_url,
            client_id, client_secret_prefix, redirect_uris, is_active,
            created_at, updated_at
        "#,
    )
    .bind(app_id)
    .bind(user_id)
    .bind(&secret_hash)
    .bind(&secret_prefix)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::AppNotFound)?;

    let app = OauthApp {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        description: row.get("description"),
        website_url: row.get("website_url"),
        logo_url: row.get("logo_url"),
        client_id: row.get("client_id"),
        client_secret_prefix: row.get("client_secret_prefix"),
        redirect_uris: row.get("redirect_uris"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(OauthAppCreated {
        app,
        client_secret: raw_secret,
    })
}

pub async fn delete_application(
    pool: &PgPool,
    user_id: Uuid,
    app_id: Uuid,
) -> Result<(), OauthError> {
    let res = sqlx::query(
        "DELETE FROM oauth_applications WHERE id = $1 AND user_id = $2",
    )
    .bind(app_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(OauthError::AppNotFound);
    }
    Ok(())
}

pub async fn create_authorization_code(
    pool: &PgPool,
    application_id: Uuid,
    user_id: Uuid,
    redirect_uri: &str,
    scope: &str,
    state: Option<&str>,
    code_challenge: Option<&str>,
    code_challenge_method: Option<&str>,
) -> Result<String, OauthError> {
    let code = format!("sats_code_{}", random_token(20));
    let expires_at = Utc::now() + Duration::minutes(10); // 10 min expiry

    sqlx::query(
        r#"
        INSERT INTO oauth_authorization_codes (
            code, application_id, user_id, redirect_uri, scope, state, expires_at,
            code_challenge, code_challenge_method
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(&code)
    .bind(application_id)
    .bind(user_id)
    .bind(redirect_uri)
    .bind(scope)
    .bind(state)
    .bind(expires_at)
    .bind(code_challenge)
    .bind(code_challenge_method)
    .execute(pool)
    .await?;

    // Record consent
    sqlx::query(
        r#"
        INSERT INTO oauth_user_consents (user_id, application_id, granted_scopes)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, application_id)
        DO UPDATE SET granted_scopes = $3, updated_at = now()
        "#,
    )
    .bind(user_id)
    .bind(application_id)
    .bind(scope)
    .execute(pool)
    .await?;

    Ok(code)
}

pub async fn exchange_authorization_code(
    pool: &PgPool,
    secrets: &SecretsService,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: Option<&str>,
) -> Result<(String, i64, String), OauthError> {
    // 1. Fetch app and verify secret
    let app_row = sqlx::query(
        r#"
        SELECT id, client_secret_hash, is_active
        FROM oauth_applications
        WHERE client_id = $1
        "#,
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::AppNotFound)?;

    let is_active: bool = app_row.get("is_active");
    if !is_active {
        return Err(OauthError::AppDisabled);
    }

    let stored_hash: String = app_row.get("client_secret_hash");
    if !secrets.verify_hmac_hex(client_secret, &stored_hash) {
        return Err(OauthError::InvalidClientSecret);
    }

    let app_id: Uuid = app_row.get("id");

    // 2. Fetch code row
    let code_row = sqlx::query(
        r#"
        SELECT code, application_id, user_id, redirect_uri, scope, expires_at, used_at,
               code_challenge, code_challenge_method
        FROM oauth_authorization_codes
        WHERE code = $1 AND application_id = $2
        "#,
    )
    .bind(code)
    .bind(app_id)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::InvalidCode)?;

    let used_at: Option<DateTime<Utc>> = code_row.get("used_at");
    let expires_at: DateTime<Utc> = code_row.get("expires_at");
    if used_at.is_some() || expires_at < Utc::now() {
        return Err(OauthError::InvalidCode);
    }

    let code_redirect_uri: String = code_row.get("redirect_uri");
    if code_redirect_uri != redirect_uri {
        return Err(OauthError::InvalidRedirectUri);
    }

    // 2b. PKCE — mandatory S256 (OAuth 2.1). Codes without a stored challenge are rejected.
    let stored_challenge: Option<String> = code_row.get("code_challenge");
    let Some(challenge) = stored_challenge.filter(|s| !s.is_empty()) else {
        return Err(OauthError::InvalidPkce);
    };
    let Some(verifier) = code_verifier.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(OauthError::InvalidPkce);
    };
    let method: Option<String> = code_row.get("code_challenge_method");
    if method.as_deref().unwrap_or("S256") != "S256" {
        return Err(OauthError::InvalidPkce);
    }
    if !verify_pkce_s256(verifier, &challenge) {
        return Err(OauthError::InvalidPkce);
    }

    // 3. Mark code as used
    sqlx::query(
        "UPDATE oauth_authorization_codes SET used_at = now() WHERE code = $1",
    )
    .bind(code)
    .execute(pool)
    .await?;

    // 4. Generate Access Token
    let token = format!("sats_tok_{}", random_token(24));
    let expires_in_secs: i64 = 86400 * 30; // 30 days
    let token_expires_at = Utc::now() + Duration::seconds(expires_in_secs);
    let user_id: Uuid = code_row.get("user_id");
    let scope: String = code_row.get("scope");

    sqlx::query(
        r#"
        INSERT INTO oauth_access_tokens (
            token, application_id, user_id, scope, expires_at
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&token)
    .bind(app_id)
    .bind(user_id)
    .bind(&scope)
    .bind(token_expires_at)
    .execute(pool)
    .await?;

    Ok((token, expires_in_secs, scope))
}

fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;
    if verifier.len() < 43 || verifier.len() > 128 {
        return false;
    }
    let digest = Sha256::digest(verifier.as_bytes());
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    bool::from(encoded.as_bytes().ct_eq(challenge.as_bytes()))
}

pub async fn get_user_by_oauth_token(
    pool: &PgPool,
    token: &str,
) -> Result<OauthUserInfo, OauthError> {
    let row = sqlx::query(
        r#"
        SELECT
            u.id,
            u.username,
            u.email,
            u.created_at,
            t.expires_at,
            t.revoked_at
        FROM oauth_access_tokens t
        JOIN users u ON u.id = t.user_id
        WHERE t.token = $1
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await?
    .ok_or(OauthError::InvalidToken)?;

    let revoked_at: Option<DateTime<Utc>> = row.get("revoked_at");
    let expires_at: DateTime<Utc> = row.get("expires_at");
    if revoked_at.is_some() || expires_at < Utc::now() {
        return Err(OauthError::InvalidToken);
    }

    let user_id: Uuid = row.get("id");
    let username: String = row.get("username");
    let email: String = row.get("email");
    let created_at: DateTime<Utc> = row.get("created_at");
    let name = username.clone();
    let picture = format!("https://api.dicebear.com/7.x/identicon/svg?seed={}", username);

    Ok(OauthUserInfo {
        sub: user_id.to_string(),
        id: user_id.to_string(),
        username,
        email,
        name,
        picture,
        email_verified: true,
        created_at,
    })
}

pub async fn list_user_consents(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<UserAuthorizedAppItem>, OauthError> {
    let rows = sqlx::query(
        r#"
        SELECT
            a.id AS application_id,
            a.name,
            a.description,
            a.website_url,
            a.logo_url,
            c.granted_scopes,
            c.created_at AS authorized_at
        FROM oauth_user_consents c
        JOIN oauth_applications a ON a.id = c.application_id
        WHERE c.user_id = $1
        ORDER BY c.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserAuthorizedAppItem {
            application_id: r.get("application_id"),
            name: r.get("name"),
            description: r.get("description"),
            website_url: r.get("website_url"),
            logo_url: r.get("logo_url"),
            granted_scopes: r.get("granted_scopes"),
            authorized_at: r.get("authorized_at"),
        })
        .collect())
}

pub async fn revoke_user_consent(
    pool: &PgPool,
    user_id: Uuid,
    application_id: Uuid,
) -> Result<(), OauthError> {
    sqlx::query(
        "DELETE FROM oauth_user_consents WHERE user_id = $1 AND application_id = $2",
    )
    .bind(user_id)
    .bind(application_id)
    .execute(pool)
    .await?;

    // Also revoke any active tokens for this user & app
    sqlx::query(
        "UPDATE oauth_access_tokens SET revoked_at = now() WHERE user_id = $1 AND application_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(application_id)
    .execute(pool)
    .await?;

    Ok(())
}
