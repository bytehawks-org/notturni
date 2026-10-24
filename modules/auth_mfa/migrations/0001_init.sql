-- noct: generato da `noct-ctl migrate generate` — modulo=auth_mfa versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum mfa_method
CREATE TYPE "mfa_method" AS ENUM ('totp', 'email');

-- crea tabella mfa_email_codes
CREATE TABLE "mfa_email_codes" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "expires_at" timestamptz NOT NULL,
    "consumed_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_mfa_email_codes_user_id" ON "mfa_email_codes" ("user_id");

-- crea tabella mfa_settings
CREATE TABLE "mfa_settings" (
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "method" "mfa_method",
    "totp_secret" text,
    "pending_totp_secret" text,
    "totp_last_step" bigint,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("user_id")
);
