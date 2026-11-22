-- noct: generato da `noct-ctl migrate generate` — modulo=custom_domains versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum custom_domain_status
CREATE TYPE "custom_domain_status" AS ENUM ('pending', 'verified', 'failed');

-- crea tabella custom_domains
CREATE TABLE "custom_domains" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "domain" varchar(255) NOT NULL,
    "verification_token" varchar(64) NOT NULL,
    "status" "custom_domain_status" NOT NULL DEFAULT 'pending',
    "verified_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_custom_domains_user" UNIQUE ("user_id"),
    CONSTRAINT "uq_custom_domains_domain" UNIQUE ("domain")
);
