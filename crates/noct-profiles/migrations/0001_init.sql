-- noct: generato da `noct-ctl migrate generate` — modulo=profiles versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella email_change_requests
CREATE TABLE "email_change_requests" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "new_email" varchar(255) NOT NULL,
    "old_expires_at" timestamptz NOT NULL,
    "old_consumed_at" timestamptz,
    "new_expires_at" timestamptz,
    "new_consumed_at" timestamptz,
    "completed_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_email_change_requests_user_id" ON "email_change_requests" ("user_id");

-- crea tabella social_links
CREATE TABLE "social_links" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "label" varchar(100) NOT NULL,
    "url" varchar(2048) NOT NULL,
    "position" integer NOT NULL DEFAULT 0,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_social_links_user_id" ON "social_links" ("user_id");

-- crea tabella verification_settings
CREATE TABLE "verification_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "gold_identifiers" text[] NOT NULL DEFAULT '{}',
    "silver_identifiers" text[] NOT NULL DEFAULT '{}',
    "blue_domains" text[] NOT NULL DEFAULT '{}',
    PRIMARY KEY ("id")
);
