-- OAuth 2.0 / OpenID Connect Identity Provider tables

CREATE TABLE IF NOT EXISTS oauth_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    website_url TEXT,
    logo_url TEXT,
    client_id TEXT NOT NULL UNIQUE,
    client_secret_hash TEXT NOT NULL,
    client_secret_prefix TEXT NOT NULL,
    redirect_uris TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oauth_applications_user_id ON oauth_applications(user_id);
CREATE INDEX IF NOT EXISTS idx_oauth_applications_client_id ON oauth_applications(client_id);

CREATE TABLE IF NOT EXISTS oauth_authorization_codes (
    code TEXT PRIMARY KEY,
    application_id UUID NOT NULL REFERENCES oauth_applications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'openid profile email',
    state TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oauth_auth_codes_app ON oauth_authorization_codes(application_id);
CREATE INDEX IF NOT EXISTS idx_oauth_auth_codes_user ON oauth_authorization_codes(user_id);

CREATE TABLE IF NOT EXISTS oauth_access_tokens (
    token TEXT PRIMARY KEY,
    application_id UUID NOT NULL REFERENCES oauth_applications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope TEXT NOT NULL DEFAULT 'openid profile email',
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oauth_access_tokens_app ON oauth_access_tokens(application_id);
CREATE INDEX IF NOT EXISTS idx_oauth_access_tokens_user ON oauth_access_tokens(user_id);

CREATE TABLE IF NOT EXISTS oauth_user_consents (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    application_id UUID NOT NULL REFERENCES oauth_applications(id) ON DELETE CASCADE,
    granted_scopes TEXT NOT NULL DEFAULT 'openid profile email',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, application_id)
);
