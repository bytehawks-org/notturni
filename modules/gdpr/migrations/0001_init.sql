-- noct: generato da `noct-ctl migrate generate` — modulo=gdpr versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum gdpr_request_type
CREATE TYPE "gdpr_request_type" AS ENUM ('export', 'deletion');

-- crea enum gdpr_request_status
CREATE TYPE "gdpr_request_status" AS ENUM ('open', 'approved', 'completed', 'rejected');

-- crea tabella gdpr_requests
CREATE TABLE "gdpr_requests" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "username" varchar(32) NOT NULL,
    "type" "gdpr_request_type" NOT NULL,
    "status" "gdpr_request_status" NOT NULL DEFAULT 'open',
    "deadline_at" timestamptz NOT NULL,
    "note" text,
    "created_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "approved_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "approved_at" timestamptz,
    "completed_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_gdpr_requests_user_id" ON "gdpr_requests" ("user_id");
CREATE INDEX "ix_gdpr_requests_status_deadline_at" ON "gdpr_requests" ("status", "deadline_at");
