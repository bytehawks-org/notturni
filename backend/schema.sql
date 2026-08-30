-- Schema del database Notturni (PostgreSQL 16+).
--
-- Generato da Alembic a scopo di sola documentazione/riferimento:
--   alembic upgrade head --sql
-- Non è il modo previsto per applicare lo schema: usare "alembic upgrade head"
-- (vedi backend/README.md). Se lo schema cambia, questo file va rigenerato con
-- lo stesso comando e ricommittato — non modificarlo a mano.

BEGIN;

CREATE TABLE alembic_version (
    version_num VARCHAR(32) NOT NULL,
    CONSTRAINT alembic_version_pkc PRIMARY KEY (version_num)
);

-- Running upgrade  -> a788c6cab085

CREATE TYPE platform_role AS ENUM ('super_admin', 'amministratore', 'moderatore', 'utente');

CREATE TABLE users (
    username VARCHAR(32) NOT NULL,
    email VARCHAR(255) NOT NULL,
    hashed_password VARCHAR(255),
    platform_role platform_role NOT NULL,
    is_active BOOLEAN NOT NULL,
    mfa_enabled BOOLEAN NOT NULL,
    mfa_totp_secret VARCHAR(64),
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX ix_users_email ON users (email);

CREATE UNIQUE INDEX ix_users_username ON users (username);

CREATE TABLE blogs (
    slug VARCHAR(63) NOT NULL,
    title VARCHAR(255) NOT NULL,
    custom_domain VARCHAR(255),
    owner_id UUID NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT ck_blog_slug_min_length CHECK (length(slug) >= 4),
    FOREIGN KEY(owner_id) REFERENCES users (id),
    UNIQUE (custom_domain)
);

CREATE UNIQUE INDEX ix_blogs_slug ON blogs (slug);

CREATE TYPE blog_role AS ENUM ('autore', 'co_autore', 'revisore', 'mediatore');

CREATE TABLE blog_memberships (
    user_id UUID NOT NULL,
    blog_id UUID NOT NULL,
    role blog_role NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(blog_id) REFERENCES blogs (id),
    FOREIGN KEY(user_id) REFERENCES users (id),
    CONSTRAINT uq_blog_membership_user_blog UNIQUE (user_id, blog_id)
);

CREATE TYPE post_status AS ENUM ('draft', 'published');

CREATE TABLE posts (
    blog_id UUID NOT NULL,
    author_id UUID NOT NULL,
    author_display_name VARCHAR(255) NOT NULL,
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    status post_status NOT NULL,
    published_at TIMESTAMP WITH TIME ZONE,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(author_id) REFERENCES users (id),
    FOREIGN KEY(blog_id) REFERENCES blogs (id),
    CONSTRAINT uq_post_blog_slug UNIQUE (blog_id, slug)
);

CREATE INDEX ix_posts_slug ON posts (slug);

CREATE TYPE comment_status AS ENUM ('pending', 'approved', 'rejected');

CREATE TABLE comments (
    post_id UUID NOT NULL,
    author_id UUID,
    author_display_name VARCHAR(255) NOT NULL,
    author_email VARCHAR(255),
    content TEXT NOT NULL,
    status comment_status NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(author_id) REFERENCES users (id),
    FOREIGN KEY(post_id) REFERENCES posts (id)
);

INSERT INTO alembic_version (version_num) VALUES ('a788c6cab085') RETURNING alembic_version.version_num;

-- Running upgrade a788c6cab085 -> 6b37f8bc737a

CREATE TYPE api_token_owner_type AS ENUM ('core', 'user');

CREATE TABLE api_tokens (
    name VARCHAR(255) NOT NULL,
    owner_type api_token_owner_type NOT NULL,
    user_id UUID,
    token_prefix VARCHAR(16) NOT NULL,
    token_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    revoked_at TIMESTAMP WITH TIME ZONE,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT ck_api_token_owner_consistency CHECK ((owner_type = 'user' AND user_id IS NOT NULL) OR (owner_type = 'core' AND user_id IS NULL)),
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE UNIQUE INDEX ix_api_tokens_token_hash ON api_tokens (token_hash);

UPDATE alembic_version SET version_num='6b37f8bc737a' WHERE alembic_version.version_num = 'a788c6cab085';

-- Running upgrade 6b37f8bc737a -> 0767b8d527ec

CREATE TABLE mfa_email_codes (
    user_id UUID NOT NULL,
    code_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    consumed_at TIMESTAMP WITH TIME ZONE,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE TYPE sso_provider AS ENUM ('google', 'microsoft', 'github', 'linkedin');

CREATE TABLE sso_identities (
    user_id UUID NOT NULL,
    provider sso_provider NOT NULL,
    provider_user_id VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(user_id) REFERENCES users (id),
    CONSTRAINT uq_sso_identity_provider_user UNIQUE (provider, provider_user_id)
);

CREATE TABLE user_sessions (
    user_id UUID NOT NULL,
    refresh_token_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_used_at TIMESTAMP WITH TIME ZONE,
    revoked_at TIMESTAMP WITH TIME ZONE,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE UNIQUE INDEX ix_user_sessions_refresh_token_hash ON user_sessions (refresh_token_hash);

ALTER TABLE blogs ADD COLUMN allow_anonymous_comments BOOLEAN DEFAULT false NOT NULL;

CREATE TYPE mfa_method AS ENUM ('totp', 'email');

ALTER TABLE users ADD COLUMN mfa_method mfa_method;

UPDATE alembic_version SET version_num='0767b8d527ec' WHERE alembic_version.version_num = '6b37f8bc737a';

-- Running upgrade 0767b8d527ec -> 07a5634edf75

CREATE TABLE pages (
    slug VARCHAR(255) NOT NULL,
    locale VARCHAR(2) NOT NULL,
    translation_group_id UUID NOT NULL,
    title VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    is_published BOOLEAN NOT NULL,
    updated_by_id UUID NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(updated_by_id) REFERENCES users (id),
    CONSTRAINT uq_page_slug_locale UNIQUE (slug, locale),
    CONSTRAINT uq_page_translation_group_locale UNIQUE (translation_group_id, locale)
);

CREATE INDEX ix_pages_slug ON pages (slug);

CREATE INDEX ix_pages_translation_group_id ON pages (translation_group_id);

CREATE TABLE social_links (
    user_id UUID NOT NULL,
    label VARCHAR(50) NOT NULL,
    url VARCHAR(500) NOT NULL,
    position INTEGER NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE TABLE user_follows (
    follower_id UUID NOT NULL,
    followed_user_id UUID NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT ck_user_follow_not_self CHECK (follower_id != followed_user_id),
    FOREIGN KEY(followed_user_id) REFERENCES users (id),
    FOREIGN KEY(follower_id) REFERENCES users (id),
    CONSTRAINT uq_user_follow_pair UNIQUE (follower_id, followed_user_id)
);

CREATE TABLE blog_follows (
    follower_id UUID NOT NULL,
    blog_id UUID NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(blog_id) REFERENCES blogs (id),
    FOREIGN KEY(follower_id) REFERENCES users (id),
    CONSTRAINT uq_blog_follow_pair UNIQUE (follower_id, blog_id)
);

ALTER TABLE blogs ADD COLUMN default_locale VARCHAR(2) DEFAULT 'it' NOT NULL;

ALTER TABLE posts ADD COLUMN locale VARCHAR(2) DEFAULT 'it' NOT NULL;

ALTER TABLE posts ADD COLUMN translation_group_id UUID DEFAULT gen_random_uuid() NOT NULL;

ALTER TABLE posts DROP CONSTRAINT uq_post_blog_slug;

CREATE INDEX ix_posts_translation_group_id ON posts (translation_group_id);

ALTER TABLE posts ADD CONSTRAINT uq_post_blog_slug_locale UNIQUE (blog_id, slug, locale);

ALTER TABLE posts ADD CONSTRAINT uq_post_translation_group_locale UNIQUE (translation_group_id, locale);

ALTER TABLE users ADD COLUMN bio TEXT;

ALTER TABLE users ADD COLUMN avatar_object_key VARCHAR(255);

UPDATE alembic_version SET version_num='07a5634edf75' WHERE alembic_version.version_num = '0767b8d527ec';

-- Running upgrade 07a5634edf75 -> a7bbd274e2af

CREATE TABLE blog_configs (
    blog_id UUID NOT NULL,
    config JSONB NOT NULL,
    id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY(blog_id) REFERENCES blogs (id),
    UNIQUE (blog_id)
);

UPDATE alembic_version SET version_num='a7bbd274e2af' WHERE alembic_version.version_num = '07a5634edf75';

-- Running upgrade a7bbd274e2af -> 2807a24ea58f

ALTER TYPE post_status ADD VALUE IF NOT EXISTS 'pending_review';

UPDATE alembic_version SET version_num='2807a24ea58f' WHERE alembic_version.version_num = 'a7bbd274e2af';

-- Running upgrade 2807a24ea58f -> e416be915439

ALTER TABLE posts ADD COLUMN cover_image_url VARCHAR(2048);

UPDATE alembic_version SET version_num='e416be915439' WHERE alembic_version.version_num = '2807a24ea58f';

COMMIT;
