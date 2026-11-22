-- noct: generato da `noct-ctl migrate generate` — modulo=auth versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum api_token_owner
CREATE TYPE "api_token_owner" AS ENUM ('core', 'user');

-- crea tabella api_tokens
CREATE TABLE "api_tokens" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "name" varchar(255) NOT NULL,
    "owner_type" "api_token_owner" NOT NULL,
    "user_id" uuid REFERENCES "users" ("id") ON DELETE CASCADE,
    "token_prefix" varchar(16) NOT NULL,
    "token_hash" varchar(64) NOT NULL,
    "expires_at" timestamptz,
    "last_used_at" timestamptz,
    "revoked_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_api_tokens_hash" UNIQUE ("token_hash")
);
CREATE INDEX "ix_api_tokens_user_id" ON "api_tokens" ("user_id");

-- crea tabella password_reset_codes
CREATE TABLE "password_reset_codes" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "expires_at" timestamptz NOT NULL,
    "consumed_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_password_reset_codes_user_id_created_at" ON "password_reset_codes" ("user_id", "created_at");

-- crea tabella user_sessions
CREATE TABLE "user_sessions" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "refresh_token_hash" varchar(64) NOT NULL,
    "expires_at" timestamptz NOT NULL,
    "last_used_at" timestamptz,
    "revoked_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_user_sessions_refresh_hash" UNIQUE ("refresh_token_hash")
);
CREATE INDEX "ix_user_sessions_user_id" ON "user_sessions" ("user_id");
CREATE INDEX "ix_user_sessions_expires_at" ON "user_sessions" ("expires_at");
