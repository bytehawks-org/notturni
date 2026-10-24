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

-- Running upgrade e416be915439 -> bd9a65e7bdd4

CREATE TABLE tags (
    name VARCHAR(30) NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX ix_tags_name ON tags (name);

CREATE TABLE post_tags (
    post_id UUID NOT NULL, 
    tag_id UUID NOT NULL, 
    PRIMARY KEY (post_id, tag_id), 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE, 
    FOREIGN KEY(tag_id) REFERENCES tags (id) ON DELETE CASCADE
);

ALTER TABLE posts ADD COLUMN manual_tags VARCHAR(30)[] DEFAULT '{}' NOT NULL;

UPDATE alembic_version SET version_num='bd9a65e7bdd4' WHERE alembic_version.version_num = 'e416be915439';

-- Running upgrade bd9a65e7bdd4 -> f3ed30401d5f

ALTER TABLE blogs ADD COLUMN default_author_display_name VARCHAR(255);

ALTER TABLE users ADD COLUMN first_name VARCHAR(100);

ALTER TABLE users ADD COLUMN last_name VARCHAR(100);

ALTER TABLE users ADD COLUMN country VARCHAR(2);

ALTER TABLE users ADD COLUMN native_language VARCHAR(2);

ALTER TABLE users ADD COLUMN fallback_languages VARCHAR(2)[] DEFAULT '{}' NOT NULL;

UPDATE alembic_version SET version_num='f3ed30401d5f' WHERE alembic_version.version_num = 'bd9a65e7bdd4';

-- Running upgrade f3ed30401d5f -> d5dbeeb3f79f

ALTER TABLE posts ADD COLUMN cover_image_is_sensitive BOOLEAN DEFAULT 'false' NOT NULL;

UPDATE alembic_version SET version_num='d5dbeeb3f79f' WHERE alembic_version.version_num = 'f3ed30401d5f';

-- Running upgrade d5dbeeb3f79f -> b16963e9cdcb

CREATE TABLE categories (
    blog_id UUID NOT NULL, 
    name VARCHAR(50) NOT NULL, 
    slug VARCHAR(60) NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id), 
    CONSTRAINT uq_category_blog_slug UNIQUE (blog_id, slug)
);

ALTER TABLE posts ADD COLUMN category_id UUID;

ALTER TABLE posts ADD CONSTRAINT fk_posts_category_id_categories FOREIGN KEY(category_id) REFERENCES categories (id) ON DELETE SET NULL;

UPDATE alembic_version SET version_num='b16963e9cdcb' WHERE alembic_version.version_num = 'd5dbeeb3f79f';

-- Running upgrade b16963e9cdcb -> c4e1a7f2b830

ALTER TABLE users ADD COLUMN display_name VARCHAR(255);

ALTER TABLE blogs ADD COLUMN subtitle VARCHAR(64);

ALTER TABLE blogs ADD COLUMN description VARCHAR(256);

CREATE TYPE blog_visibility AS ENUM ('public', 'members', 'private');

ALTER TABLE blogs ADD COLUMN visibility blog_visibility DEFAULT 'public' NOT NULL;

ALTER TABLE blog_memberships ADD COLUMN author_display_name VARCHAR(255);

CREATE TYPE blog_invitation_status AS ENUM ('pending', 'accepted', 'declined', 'revoked');

CREATE TABLE blog_invitations (
    blog_id UUID NOT NULL, 
    invited_user_id UUID NOT NULL, 
    invited_by_id UUID NOT NULL, 
    role blog_role NOT NULL, 
    status blog_invitation_status DEFAULT 'pending' NOT NULL, 
    responded_at TIMESTAMP WITH TIME ZONE, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(invited_user_id) REFERENCES users (id), 
    FOREIGN KEY(invited_by_id) REFERENCES users (id), 
    CONSTRAINT uq_blog_invitation_blog_user UNIQUE (blog_id, invited_user_id)
);

UPDATE alembic_version SET version_num='c4e1a7f2b830' WHERE alembic_version.version_num = 'b16963e9cdcb';

-- Running upgrade c4e1a7f2b830 -> d8b3f1027a45

CREATE TYPE post_author_name_style AS ENUM ('username', 'full_name', 'display_name');

ALTER TABLE users ADD COLUMN post_author_name_style post_author_name_style DEFAULT 'username' NOT NULL;

ALTER TABLE blogs ADD COLUMN mentions_enabled BOOLEAN DEFAULT true NOT NULL;

UPDATE alembic_version SET version_num='d8b3f1027a45' WHERE alembic_version.version_num = 'c4e1a7f2b830';

-- Running upgrade d8b3f1027a45 -> e2c9a4517f60

CREATE TABLE post_notes (
    post_id UUID NOT NULL, 
    idx INTEGER NOT NULL, 
    content TEXT NOT NULL, 
    PRIMARY KEY (post_id, idx), 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE
);

UPDATE alembic_version SET version_num='e2c9a4517f60' WHERE alembic_version.version_num = 'd8b3f1027a45';

-- Running upgrade e2c9a4517f60 -> 9fca56e73604

ALTER TABLE blogs ADD COLUMN static_pages_enabled BOOLEAN DEFAULT false NOT NULL;

ALTER TABLE pages ADD COLUMN blog_id UUID;

ALTER TABLE pages ADD CONSTRAINT fk_pages_blog_id_blogs FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE;

CREATE INDEX ix_pages_blog_id ON pages (blog_id);

ALTER TABLE pages DROP CONSTRAINT uq_page_slug_locale;

ALTER TABLE pages ADD CONSTRAINT uq_page_blog_slug_locale UNIQUE (blog_id, slug, locale);

CREATE UNIQUE INDEX uq_page_slug_locale_platform ON pages (slug, locale) WHERE blog_id IS NULL;

UPDATE alembic_version SET version_num='9fca56e73604' WHERE alembic_version.version_num = 'e2c9a4517f60';

-- Running upgrade 9fca56e73604 -> ef332d4924b0

CREATE TABLE post_links (
    post_id UUID NOT NULL, 
    position INTEGER NOT NULL, 
    url TEXT NOT NULL, 
    link_text TEXT NOT NULL, 
    PRIMARY KEY (post_id, position), 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE
);

CREATE TABLE post_media (
    post_id UUID NOT NULL, 
    position INTEGER NOT NULL, 
    url TEXT NOT NULL, 
    alt_text TEXT NOT NULL, 
    categories VARCHAR(20)[] NOT NULL, 
    PRIMARY KEY (post_id, position), 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE
);

ALTER TABLE posts ADD COLUMN cover_image_categories VARCHAR(20)[] DEFAULT '{}' NOT NULL;

UPDATE alembic_version SET version_num='ef332d4924b0' WHERE alembic_version.version_num = '9fca56e73604';

-- Running upgrade ef332d4924b0 -> 98da258b5f92

CREATE TABLE post_fragments (
    user_id UUID NOT NULL, 
    post_id UUID NOT NULL, 
    text TEXT NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(post_id) REFERENCES posts (id), 
    FOREIGN KEY(user_id) REFERENCES users (id), 
    CONSTRAINT uq_post_fragment_user_post_text UNIQUE (user_id, post_id, text)
);

CREATE INDEX ix_post_fragments_post_id ON post_fragments (post_id);

CREATE INDEX ix_post_fragments_user_id ON post_fragments (user_id);

UPDATE alembic_version SET version_num='98da258b5f92' WHERE alembic_version.version_num = 'ef332d4924b0';

-- Running upgrade 98da258b5f92 -> 1c9f6a2d3b4e

ALTER TABLE blogs ADD COLUMN is_suspended BOOLEAN DEFAULT 'false' NOT NULL;

UPDATE alembic_version SET version_num='1c9f6a2d3b4e' WHERE alembic_version.version_num = '98da258b5f92';

-- Running upgrade 1c9f6a2d3b4e -> 2d7e4b8c1f6a

ALTER TABLE posts ADD COLUMN is_hidden BOOLEAN DEFAULT 'false' NOT NULL;

UPDATE alembic_version SET version_num='2d7e4b8c1f6a' WHERE alembic_version.version_num = '1c9f6a2d3b4e';

-- Running upgrade 2d7e4b8c1f6a -> 4b2e8a1c9d30

CREATE TYPE audit_actor_type AS ENUM ('user', 'core_token', 'user_token', 'system', 'anonymous');

CREATE TABLE audit_log (
    id UUID NOT NULL, 
    occurred_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    actor_type audit_actor_type NOT NULL, 
    actor_id UUID, 
    actor_label VARCHAR(255), 
    action VARCHAR(100) NOT NULL, 
    target_type VARCHAR(50), 
    target_id UUID, 
    blog_id UUID, 
    ip INET, 
    user_agent VARCHAR(500), 
    payload JSONB DEFAULT '{}'::jsonb NOT NULL, 
    PRIMARY KEY (id)
);

CREATE INDEX ix_audit_log_occurred_at ON audit_log (occurred_at);

CREATE INDEX ix_audit_log_actor_id_occurred_at ON audit_log (actor_id, occurred_at);

CREATE INDEX ix_audit_log_blog_id_occurred_at ON audit_log (blog_id, occurred_at);

UPDATE alembic_version SET version_num='4b2e8a1c9d30' WHERE alembic_version.version_num = '2d7e4b8c1f6a';

-- Running upgrade 4b2e8a1c9d30 -> 5c3f9a71e0d2

CREATE TABLE audit_archive_runs (
    id UUID NOT NULL, 
    period_start TIMESTAMP WITH TIME ZONE NOT NULL, 
    period_end TIMESTAMP WITH TIME ZONE NOT NULL, 
    week_label VARCHAR(16) NOT NULL, 
    object_key VARCHAR(512), 
    storage_backend VARCHAR(20) NOT NULL, 
    row_count INTEGER NOT NULL, 
    byte_size INTEGER DEFAULT '0' NOT NULL, 
    sha256 VARCHAR(64), 
    archived_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    CONSTRAINT uq_audit_archive_runs_period_start UNIQUE (period_start)
);

UPDATE alembic_version SET version_num='5c3f9a71e0d2' WHERE alembic_version.version_num = '4b2e8a1c9d30';

-- Running upgrade 5c3f9a71e0d2 -> b1c2d3e4f5a6

CREATE INDEX ix_comments_post_id ON comments (post_id);

CREATE INDEX ix_user_follows_followed_user_id ON user_follows (followed_user_id);

CREATE INDEX ix_blog_follows_blog_id ON blog_follows (blog_id);

CREATE INDEX ix_posts_author_id ON posts (author_id);

CREATE INDEX ix_posts_category_id ON posts (category_id);

CREATE INDEX ix_posts_status_published_at ON posts (status, published_at);

UPDATE alembic_version SET version_num='b1c2d3e4f5a6' WHERE alembic_version.version_num = '5c3f9a71e0d2';

-- Running upgrade b1c2d3e4f5a6 -> 7981bf8eb571

CREATE TYPE comments_mode AS ENUM ('everyone', 'members', 'closed');

ALTER TABLE blogs ADD COLUMN comments_mode comments_mode;

UPDATE blogs SET comments_mode = CASE WHEN allow_anonymous_comments THEN 'everyone'::comments_mode ELSE 'members'::comments_mode END;

ALTER TABLE blogs ALTER COLUMN comments_mode SET NOT NULL;

ALTER TABLE blogs DROP COLUMN allow_anonymous_comments;

ALTER TABLE comments ADD COLUMN parent_id UUID;

CREATE INDEX ix_comments_parent_id ON comments (parent_id);

ALTER TABLE comments ADD CONSTRAINT comments_parent_id_fkey FOREIGN KEY(parent_id) REFERENCES comments (id) ON DELETE CASCADE;

ALTER TABLE posts ADD COLUMN comments_mode comments_mode;

UPDATE alembic_version SET version_num='7981bf8eb571' WHERE alembic_version.version_num = 'b1c2d3e4f5a6';

-- Running upgrade 7981bf8eb571 -> c5c5ea3b5cf6

ALTER TABLE post_fragments ADD COLUMN is_public BOOLEAN DEFAULT false NOT NULL;

UPDATE alembic_version SET version_num='c5c5ea3b5cf6' WHERE alembic_version.version_num = '7981bf8eb571';

-- Running upgrade c5c5ea3b5cf6 -> 139cf285ee44

ALTER TABLE blogs ADD COLUMN search_indexing_enabled BOOLEAN DEFAULT true NOT NULL;

ALTER TABLE blogs ADD COLUMN ai_crawling_enabled BOOLEAN DEFAULT true NOT NULL;

ALTER TABLE blogs ALTER COLUMN search_indexing_enabled DROP DEFAULT;

ALTER TABLE blogs ALTER COLUMN ai_crawling_enabled DROP DEFAULT;

ALTER TABLE posts ADD COLUMN search_indexing_enabled BOOLEAN;

ALTER TABLE posts ADD COLUMN ai_crawling_enabled BOOLEAN;

UPDATE alembic_version SET version_num='139cf285ee44' WHERE alembic_version.version_num = 'c5c5ea3b5cf6';

-- Running upgrade 139cf285ee44 -> a7c1d2e3f4b5

CREATE TABLE post_reads_daily (
    post_id UUID NOT NULL, 
    day DATE NOT NULL, 
    reads INTEGER NOT NULL, 
    PRIMARY KEY (post_id, day), 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE
);

UPDATE alembic_version SET version_num='a7c1d2e3f4b5' WHERE alembic_version.version_num = '139cf285ee44';

-- Running upgrade a7c1d2e3f4b5 -> b8d2e3f4a5c6

ALTER TABLE blogs ADD COLUMN is_paused BOOLEAN DEFAULT false NOT NULL;

ALTER TABLE blogs ADD COLUMN deleted_at TIMESTAMP WITH TIME ZONE;

ALTER TABLE blogs ADD COLUMN extra_locales VARCHAR(2)[] DEFAULT '{}' NOT NULL;

UPDATE alembic_version SET version_num='b8d2e3f4a5c6' WHERE alembic_version.version_num = 'a7c1d2e3f4b5';

-- Running upgrade b8d2e3f4a5c6 -> c9e3f4a5b6d7

ALTER TABLE comments ADD COLUMN reported_to_platform BOOLEAN DEFAULT false NOT NULL;

ALTER TABLE comments ADD COLUMN report_note TEXT;

ALTER TABLE comments ADD COLUMN reported_at TIMESTAMP WITH TIME ZONE;

ALTER TABLE blogs ADD COLUMN comments_auto_close_days INTEGER;

CREATE TABLE blog_blocked_authors (
    blog_id UUID NOT NULL, 
    user_id UUID, 
    email_hash VARCHAR(64), 
    label VARCHAR(255) NOT NULL, 
    note VARCHAR(255), 
    created_by_id UUID NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(user_id) REFERENCES users (id), 
    FOREIGN KEY(created_by_id) REFERENCES users (id)
);

CREATE INDEX ix_blog_blocked_authors_blog_id ON blog_blocked_authors (blog_id);

UPDATE alembic_version SET version_num='c9e3f4a5b6d7' WHERE alembic_version.version_num = 'b8d2e3f4a5c6';

-- Running upgrade c9e3f4a5b6d7 -> d0f4a5b6c7e8

CREATE TYPE report_target_type AS ENUM ('blog', 'post');

CREATE TYPE report_reason AS ENUM ('spam', 'abuse', 'illegal', 'other');

CREATE TYPE report_status AS ENUM ('open', 'dismissed', 'actioned');

CREATE TABLE content_reports (
    reporter_id UUID NOT NULL, 
    target_type report_target_type NOT NULL, 
    target_id UUID NOT NULL, 
    blog_id UUID NOT NULL, 
    reason report_reason NOT NULL, 
    note VARCHAR(500), 
    status report_status NOT NULL, 
    resolved_at TIMESTAMP WITH TIME ZONE, 
    resolved_by_id UUID, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(reporter_id) REFERENCES users (id), 
    FOREIGN KEY(resolved_by_id) REFERENCES users (id), 
    CONSTRAINT uq_content_report_per_reporter UNIQUE (reporter_id, target_type, target_id)
);

CREATE INDEX ix_content_reports_blog_id ON content_reports (blog_id);

CREATE INDEX ix_content_reports_reporter_id ON content_reports (reporter_id);

CREATE INDEX ix_content_reports_status ON content_reports (status);

CREATE INDEX ix_content_reports_target_id ON content_reports (target_id);

UPDATE alembic_version SET version_num='d0f4a5b6c7e8' WHERE alembic_version.version_num = 'c9e3f4a5b6d7';

-- Running upgrade d0f4a5b6c7e8 -> e1a5b6c7d8f9

ALTER TABLE users ADD COLUMN ui_locale VARCHAR(2);

CREATE TABLE platform_config (
    id SERIAL NOT NULL, 
    default_locale VARCHAR(2) NOT NULL, 
    registration_mode VARCHAR(10) NOT NULL, 
    sso_providers VARCHAR(20)[] NOT NULL, 
    mfa_required_for_admins BOOLEAN NOT NULL, 
    reserved_blog_names VARCHAR(63)[] NOT NULL, 
    moderation_threshold FLOAT NOT NULL, 
    max_blogs_per_user INTEGER NOT NULL, 
    anonymous_comments_allowed BOOLEAN NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE, 
    updated_by_id UUID, 
    PRIMARY KEY (id), 
    FOREIGN KEY(updated_by_id) REFERENCES users (id)
);

CREATE TYPE gdpr_request_type AS ENUM ('export', 'deletion');

CREATE TYPE gdpr_request_status AS ENUM ('open', 'approved', 'completed', 'rejected');

CREATE TABLE gdpr_requests (
    user_id UUID NOT NULL, 
    username VARCHAR(32) NOT NULL, 
    type gdpr_request_type NOT NULL, 
    status gdpr_request_status NOT NULL, 
    deadline_at TIMESTAMP WITH TIME ZONE NOT NULL, 
    note TEXT, 
    created_by_id UUID, 
    approved_by_id UUID, 
    approved_at TIMESTAMP WITH TIME ZONE, 
    completed_at TIMESTAMP WITH TIME ZONE, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(user_id) REFERENCES users (id), 
    FOREIGN KEY(created_by_id) REFERENCES users (id), 
    FOREIGN KEY(approved_by_id) REFERENCES users (id)
);

CREATE INDEX ix_gdpr_requests_user_id ON gdpr_requests (user_id);

CREATE INDEX ix_gdpr_requests_status ON gdpr_requests (status);

UPDATE alembic_version SET version_num='e1a5b6c7d8f9' WHERE alembic_version.version_num = 'd0f4a5b6c7e8';

-- Running upgrade e1a5b6c7d8f9 -> f2b6c7d8e9a0

CREATE TABLE media_files (
    blog_id UUID NOT NULL, 
    uploader_id UUID, 
    object_key VARCHAR(512), 
    url TEXT NOT NULL, 
    content_type VARCHAR(100) NOT NULL, 
    size_bytes INTEGER NOT NULL, 
    alt_text TEXT NOT NULL, 
    caption TEXT, 
    categories VARCHAR(20)[] NOT NULL, 
    is_sensitive BOOLEAN NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(uploader_id) REFERENCES users (id), 
    UNIQUE (url)
);

CREATE INDEX ix_media_files_blog_id ON media_files (blog_id);

UPDATE alembic_version SET version_num='f2b6c7d8e9a0' WHERE alembic_version.version_num = 'e1a5b6c7d8f9';

-- Running upgrade f2b6c7d8e9a0 -> a3c7d8e9f0b1

CREATE TABLE blog_notes (
    blog_id UUID NOT NULL, 
    content TEXT NOT NULL, 
    normalized VARCHAR(600) NOT NULL, 
    kind VARCHAR(10) NOT NULL, 
    url TEXT, 
    created_by_id UUID, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(created_by_id) REFERENCES users (id)
);

CREATE INDEX ix_blog_notes_blog_id ON blog_notes (blog_id);

CREATE INDEX ix_blog_notes_normalized ON blog_notes (normalized);

ALTER TABLE post_notes ADD COLUMN note_id UUID;

ALTER TABLE post_notes ADD CONSTRAINT fk_post_notes_note_id FOREIGN KEY(note_id) REFERENCES blog_notes (id) ON DELETE SET NULL;

CREATE INDEX ix_post_notes_note_id ON post_notes (note_id);

UPDATE alembic_version SET version_num='a3c7d8e9f0b1' WHERE alembic_version.version_num = 'f2b6c7d8e9a0';

-- Running upgrade a3c7d8e9f0b1 -> b4d8e9f0a1c2

CREATE TABLE publications (
    blog_id UUID NOT NULL, 
    name VARCHAR(60) NOT NULL, 
    title VARCHAR(255) NOT NULL, 
    description TEXT, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    CONSTRAINT uq_publication_blog_name UNIQUE (blog_id, name)
);

CREATE INDEX ix_publications_blog_id ON publications (blog_id);

ALTER TABLE posts ADD COLUMN publication_id UUID;

ALTER TABLE posts ADD COLUMN chapter_order INTEGER;

ALTER TABLE posts ADD CONSTRAINT fk_posts_publication_id FOREIGN KEY(publication_id) REFERENCES publications (id) ON DELETE SET NULL;

CREATE INDEX ix_posts_publication_id ON posts (publication_id);

UPDATE alembic_version SET version_num='b4d8e9f0a1c2' WHERE alembic_version.version_num = 'a3c7d8e9f0b1';

-- Running upgrade b4d8e9f0a1c2 -> c5e9f0a1b2d3

ALTER TABLE blogs ADD COLUMN cover_image_url VARCHAR(2048);

ALTER TABLE blogs ADD COLUMN cover_image_is_sensitive BOOLEAN DEFAULT 'false' NOT NULL;

ALTER TABLE blogs ADD COLUMN cover_image_categories VARCHAR(20)[] DEFAULT '{}' NOT NULL;

ALTER TABLE blogs ADD COLUMN favicon_object_key VARCHAR(255);

UPDATE alembic_version SET version_num='c5e9f0a1b2d3' WHERE alembic_version.version_num = 'b4d8e9f0a1c2';

-- Running upgrade c5e9f0a1b2d3 -> d7f0a1b2c3e4

ALTER TABLE platform_config ADD COLUMN audit_retention_days INTEGER DEFAULT '105' NOT NULL;

UPDATE alembic_version SET version_num='d7f0a1b2c3e4' WHERE alembic_version.version_num = 'c5e9f0a1b2d3';

-- Running upgrade d7f0a1b2c3e4 -> e8a1b2c3d4f5

ALTER TABLE platform_config ADD COLUMN footer_column1_markdown TEXT;

ALTER TABLE platform_config ADD COLUMN footer_column2_markdown TEXT;

ALTER TABLE platform_config ADD COLUMN footer_column3_markdown TEXT;

ALTER TABLE platform_config ADD COLUMN footer_bottom_bar_markdown TEXT;

UPDATE alembic_version SET version_num='e8a1b2c3d4f5' WHERE alembic_version.version_num = 'd7f0a1b2c3e4';

-- Running upgrade e8a1b2c3d4f5 -> f1b2c3d4e5a6

CREATE TABLE link_preview_cache (
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    url_hash VARCHAR(64) NOT NULL, 
    url TEXT NOT NULL, 
    title TEXT, 
    description TEXT, 
    image TEXT, 
    fetch_ok BOOLEAN DEFAULT 'false' NOT NULL, 
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX ix_link_preview_cache_url_hash ON link_preview_cache (url_hash);

UPDATE alembic_version SET version_num='f1b2c3d4e5a6' WHERE alembic_version.version_num = 'e8a1b2c3d4f5';

-- Running upgrade f1b2c3d4e5a6 -> b3d5e6f7a8c9

ALTER TABLE post_notes ADD COLUMN title TEXT;

ALTER TABLE post_notes ADD COLUMN author TEXT;

ALTER TABLE post_notes ADD COLUMN isbn VARCHAR(32);

ALTER TABLE post_notes ADD COLUMN doi VARCHAR(255);

ALTER TABLE post_notes ADD COLUMN page VARCHAR(32);

ALTER TABLE blog_notes ADD COLUMN title TEXT;

ALTER TABLE blog_notes ADD COLUMN author TEXT;

ALTER TABLE blog_notes ADD COLUMN isbn VARCHAR(32);

ALTER TABLE blog_notes ADD COLUMN doi VARCHAR(255);

ALTER TABLE blog_notes ADD COLUMN page VARCHAR(32);

UPDATE alembic_version SET version_num='b3d5e6f7a8c9' WHERE alembic_version.version_num = 'f1b2c3d4e5a6';

-- Running upgrade b3d5e6f7a8c9 -> c4d6e7f8a9b0

ALTER TABLE post_notes ADD COLUMN kind VARCHAR(10);

ALTER TABLE post_notes ADD COLUMN url TEXT;

ALTER TABLE post_notes ADD COLUMN source TEXT;

ALTER TABLE post_notes ADD COLUMN issued VARCHAR(32);

ALTER TABLE blog_notes ADD COLUMN source TEXT;

ALTER TABLE blog_notes ADD COLUMN issued VARCHAR(32);

UPDATE alembic_version SET version_num='c4d6e7f8a9b0' WHERE alembic_version.version_num = 'b3d5e6f7a8c9';

-- Running upgrade f1b2c3d4e5a6 -> c1d2e3f4a5b6

ALTER TABLE users ADD COLUMN username_changed_at TIMESTAMP WITH TIME ZONE;

CREATE TYPE verification_tier AS ENUM ('none', 'bronze', 'silver', 'gold', 'blue');

ALTER TABLE users ADD COLUMN verification_tier verification_tier DEFAULT 'none' NOT NULL;

CREATE TABLE email_change_requests (
    id UUID NOT NULL, 
    user_id UUID NOT NULL, 
    new_email VARCHAR(255) NOT NULL, 
    old_code_hash VARCHAR(64) NOT NULL, 
    old_expires_at TIMESTAMP WITH TIME ZONE NOT NULL, 
    old_consumed_at TIMESTAMP WITH TIME ZONE, 
    new_code_hash VARCHAR(64), 
    new_expires_at TIMESTAMP WITH TIME ZONE, 
    new_consumed_at TIMESTAMP WITH TIME ZONE, 
    completed_at TIMESTAMP WITH TIME ZONE, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE INDEX ix_email_change_requests_user_id ON email_change_requests (user_id);

CREATE TYPE custom_domain_status AS ENUM ('pending', 'verified', 'failed');

CREATE TABLE custom_domains (
    id UUID NOT NULL, 
    user_id UUID NOT NULL, 
    domain VARCHAR(255) NOT NULL, 
    verification_token VARCHAR(64) NOT NULL, 
    status custom_domain_status DEFAULT 'pending' NOT NULL, 
    verified_at TIMESTAMP WITH TIME ZONE, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    UNIQUE (user_id), 
    FOREIGN KEY(user_id) REFERENCES users (id), 
    UNIQUE (domain)
);

INSERT INTO alembic_version (version_num) VALUES ('c1d2e3f4a5b6') RETURNING alembic_version.version_num;

-- Running upgrade c1d2e3f4a5b6, c4d6e7f8a9b0 -> cf49d59f6471

ALTER TYPE post_author_name_style ADD VALUE IF NOT EXISTS 'verified_domain';

ALTER TABLE users ADD COLUMN verified_domain VARCHAR(255);

UPDATE users
        SET verified_domain = custom_domains.domain
        FROM custom_domains
        WHERE custom_domains.user_id = users.id
          AND custom_domains.status = 'verified';

DELETE FROM alembic_version WHERE alembic_version.version_num = 'c1d2e3f4a5b6';

UPDATE alembic_version SET version_num='cf49d59f6471' WHERE alembic_version.version_num = 'c4d6e7f8a9b0';

-- Running upgrade cf49d59f6471 -> 769f5009bad5

ALTER TABLE blogs ADD COLUMN newsletter_auto_notify_enabled BOOLEAN DEFAULT true NOT NULL;

ALTER TABLE blogs ALTER COLUMN newsletter_auto_notify_enabled DROP DEFAULT;

CREATE TYPE newsletter_subscriber_status AS ENUM ('pending', 'confirmed', 'unsubscribed');

CREATE TABLE newsletter_subscribers (
    blog_id UUID, 
    email VARCHAR(255) NOT NULL, 
    locale VARCHAR(2), 
    status newsletter_subscriber_status NOT NULL, 
    user_id UUID, 
    confirm_token_hash VARCHAR(64), 
    confirm_token_expires_at TIMESTAMP WITH TIME ZONE, 
    consent_ip VARCHAR(64), 
    consent_user_agent VARCHAR(512), 
    confirmed_at TIMESTAMP WITH TIME ZONE, 
    unsubscribed_at TIMESTAMP WITH TIME ZONE, 
    unsubscribe_reason VARCHAR(500), 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(user_id) REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX ix_newsletter_subscribers_confirm_token_hash ON newsletter_subscribers (confirm_token_hash);

CREATE UNIQUE INDEX uq_newsletter_subscriber_blog_email ON newsletter_subscribers (blog_id, lower(email)) WHERE blog_id IS NOT NULL;

CREATE UNIQUE INDEX uq_newsletter_subscriber_platform_email ON newsletter_subscribers (lower(email)) WHERE blog_id IS NULL;

CREATE TYPE newsletter_campaign_kind AS ENUM ('post_notification', 'manual');

CREATE TYPE newsletter_campaign_status AS ENUM ('draft', 'scheduled', 'sending', 'sent', 'canceled', 'failed');

CREATE TABLE newsletter_campaigns (
    blog_id UUID, 
    kind newsletter_campaign_kind NOT NULL, 
    post_id UUID, 
    created_by_id UUID, 
    subject VARCHAR(255) NOT NULL, 
    body_markdown TEXT, 
    status newsletter_campaign_status NOT NULL, 
    scheduled_at TIMESTAMP WITH TIME ZONE, 
    sent_at TIMESTAMP WITH TIME ZONE, 
    recipient_count INTEGER NOT NULL, 
    failed_count INTEGER NOT NULL, 
    id UUID NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(blog_id) REFERENCES blogs (id) ON DELETE CASCADE, 
    FOREIGN KEY(post_id) REFERENCES posts (id) ON DELETE CASCADE, 
    FOREIGN KEY(created_by_id) REFERENCES users (id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX uq_newsletter_campaign_post ON newsletter_campaigns (post_id) WHERE post_id IS NOT NULL;

UPDATE alembic_version SET version_num='769f5009bad5' WHERE alembic_version.version_num = 'cf49d59f6471';

-- Running upgrade cf49d59f6471 -> a1c2b3d4e5f6

CREATE TABLE password_reset_codes (
    id UUID NOT NULL, 
    user_id UUID NOT NULL, 
    code_hash VARCHAR(64) NOT NULL, 
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL, 
    consumed_at TIMESTAMP WITH TIME ZONE, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT now() NOT NULL, 
    PRIMARY KEY (id), 
    FOREIGN KEY(user_id) REFERENCES users (id)
);

CREATE INDEX ix_password_reset_codes_user_id ON password_reset_codes (user_id);

INSERT INTO alembic_version (version_num) VALUES ('a1c2b3d4e5f6') RETURNING alembic_version.version_num;

-- Running upgrade a1c2b3d4e5f6 -> b2c3d4e5f6a7

ALTER TABLE post_media ADD COLUMN is_sensitive BOOLEAN;

UPDATE post_media SET is_sensitive = (array_length(categories, 1) > 0);

UPDATE post_media SET is_sensitive = false WHERE is_sensitive IS NULL;

ALTER TABLE post_media ALTER COLUMN is_sensitive SET NOT NULL;

UPDATE alembic_version SET version_num='b2c3d4e5f6a7' WHERE alembic_version.version_num = 'a1c2b3d4e5f6';

-- Running upgrade b2c3d4e5f6a7 -> c9839d73bf05

ALTER TABLE users ADD COLUMN directory_listed BOOLEAN DEFAULT true NOT NULL;

ALTER TABLE users ALTER COLUMN directory_listed DROP DEFAULT;

UPDATE alembic_version SET version_num='c9839d73bf05' WHERE alembic_version.version_num = 'b2c3d4e5f6a7';

-- Running upgrade c9839d73bf05 -> 4d23cdb0ccc3

ALTER TABLE platform_config ADD COLUMN interests JSONB DEFAULT '[]'::jsonb NOT NULL;

ALTER TABLE platform_config ALTER COLUMN interests DROP DEFAULT;

UPDATE platform_config SET interests = CAST('[{"key": "music", "translations": {"it": "Musica", "en": "Music", "de": "Musik", "fr": "Musique"}}, {"key": "technology", "translations": {"it": "Tecnologia", "en": "Technology", "de": "Technologie", "fr": "Technologie"}}, {"key": "photography", "translations": {"it": "Fotografia", "en": "Photography", "de": "Fotografie", "fr": "Photographie"}}, {"key": "literature", "translations": {"it": "Letteratura", "en": "Literature", "de": "Literatur", "fr": "Litt\\u00e9rature"}}, {"key": "cinema", "translations": {"it": "Cinema", "en": "Cinema", "de": "Kino", "fr": "Cin\\u00e9ma"}}, {"key": "travel", "translations": {"it": "Viaggi", "en": "Travel", "de": "Reisen", "fr": "Voyages"}}, {"key": "food", "translations": {"it": "Cucina", "en": "Food", "de": "Kochen", "fr": "Cuisine"}}, {"key": "art", "translations": {"it": "Arte", "en": "Art", "de": "Kunst", "fr": "Art"}}, {"key": "nature", "translations": {"it": "Natura", "en": "Nature", "de": "Natur", "fr": "Nature"}}, {"key": "science", "translations": {"it": "Scienza", "en": "Science", "de": "Wissenschaft", "fr": "Science"}}, {"key": "sports", "translations": {"it": "Sport", "en": "Sports", "de": "Sport", "fr": "Sport"}}, {"key": "gaming", "translations": {"it": "Videogiochi", "en": "Gaming", "de": "Gaming", "fr": "Jeux vid\\u00e9o"}}, {"key": "fashion", "translations": {"it": "Moda", "en": "Fashion", "de": "Mode", "fr": "Mode"}}, {"key": "politics", "translations": {"it": "Politica", "en": "Politics", "de": "Politik", "fr": "Politique"}}, {"key": "philosophy", "translations": {"it": "Filosofia", "en": "Philosophy", "de": "Philosophie", "fr": "Philosophie"}}, {"key": "history", "translations": {"it": "Storia", "en": "History", "de": "Geschichte", "fr": "Histoire"}}]' AS jsonb) WHERE interests = '[]'::jsonb;

ALTER TABLE users ADD COLUMN interests VARCHAR(40)[] DEFAULT '{}' NOT NULL;

ALTER TABLE users ALTER COLUMN interests DROP DEFAULT;

UPDATE alembic_version SET version_num='4d23cdb0ccc3' WHERE alembic_version.version_num = 'c9839d73bf05';

-- Running upgrade 4d23cdb0ccc3, 769f5009bad5 -> a0579cc46db6

DELETE FROM alembic_version WHERE alembic_version.version_num = '4d23cdb0ccc3';

UPDATE alembic_version SET version_num='a0579cc46db6' WHERE alembic_version.version_num = '769f5009bad5';

-- Running upgrade a0579cc46db6 -> b4c5d6e7f809

ALTER TABLE users ADD COLUMN credentials_changed_at TIMESTAMP WITH TIME ZONE;

UPDATE alembic_version SET version_num='b4c5d6e7f809' WHERE alembic_version.version_num = 'a0579cc46db6';

-- Running upgrade b4c5d6e7f809 -> c5d6e7f8a910

ALTER TABLE newsletter_campaigns ADD COLUMN sent_to_subscriber_ids UUID[] DEFAULT '{}' NOT NULL;

ALTER TABLE newsletter_campaigns ALTER COLUMN sent_to_subscriber_ids DROP DEFAULT;

UPDATE alembic_version SET version_num='c5d6e7f8a910' WHERE alembic_version.version_num = 'b4c5d6e7f809';

-- Running upgrade c5d6e7f8a910 -> d6e7f8a9b021

ALTER TABLE blogs ADD COLUMN newsletter_sender_name VARCHAR(120);

ALTER TABLE blogs ADD COLUMN newsletter_banner_url VARCHAR(2048);

ALTER TABLE blogs ADD COLUMN newsletter_banner_alt_text VARCHAR(300) DEFAULT '' NOT NULL;

ALTER TABLE blogs ALTER COLUMN newsletter_banner_alt_text DROP DEFAULT;

ALTER TABLE platform_config ADD COLUMN newsletter_sender_name VARCHAR(120);

ALTER TABLE platform_config ADD COLUMN newsletter_banner_url VARCHAR(2048);

ALTER TABLE platform_config ADD COLUMN newsletter_banner_alt_text VARCHAR(300) DEFAULT '' NOT NULL;

ALTER TABLE platform_config ALTER COLUMN newsletter_banner_alt_text DROP DEFAULT;

UPDATE alembic_version SET version_num='d6e7f8a9b021' WHERE alembic_version.version_num = 'c5d6e7f8a910';

-- Running upgrade d6e7f8a9b021 -> e7f8a9b0c132

ALTER TABLE posts ADD COLUMN cover_image_alt_text VARCHAR(300) DEFAULT '' NOT NULL;

ALTER TABLE posts ALTER COLUMN cover_image_alt_text DROP DEFAULT;

ALTER TABLE blogs ADD COLUMN cover_image_alt_text VARCHAR(300) DEFAULT '' NOT NULL;

ALTER TABLE blogs ALTER COLUMN cover_image_alt_text DROP DEFAULT;

UPDATE alembic_version SET version_num='e7f8a9b0c132' WHERE alembic_version.version_num = 'd6e7f8a9b021';

COMMIT;

