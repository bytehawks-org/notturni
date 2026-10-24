-- noct: generato da `noct-ctl migrate generate` — modulo=auth_sso versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum sso_provider
CREATE TYPE "sso_provider" AS ENUM ('google', 'microsoft', 'github', 'linkedin');

-- crea tabella sso_identities
CREATE TABLE "sso_identities" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "provider" "sso_provider" NOT NULL,
    "provider_user_id" varchar(255) NOT NULL,
    "email" varchar(255) NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_sso_identity_provider_user" UNIQUE ("provider", "provider_user_id")
);
CREATE INDEX "ix_sso_identities_user_id" ON "sso_identities" ("user_id");

-- crea tabella sso_settings
CREATE TABLE "sso_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "enabled_providers" text[] NOT NULL DEFAULT '{}',
    PRIMARY KEY ("id")
);
