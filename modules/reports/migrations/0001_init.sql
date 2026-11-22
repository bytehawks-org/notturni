-- noct: generato da `noct-ctl migrate generate` — modulo=reports versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum report_target_type
CREATE TYPE "report_target_type" AS ENUM ('blog', 'post');

-- crea enum report_reason
CREATE TYPE "report_reason" AS ENUM ('spam', 'abuse', 'illegal', 'other');

-- crea enum report_status
CREATE TYPE "report_status" AS ENUM ('open', 'dismissed', 'actioned');

-- crea tabella content_reports
CREATE TABLE "content_reports" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "reporter_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "target_type" "report_target_type" NOT NULL,
    "target_id" uuid NOT NULL,
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "reason" "report_reason" NOT NULL,
    "note" varchar(500),
    "status" "report_status" NOT NULL DEFAULT 'open',
    "resolved_at" timestamptz,
    "resolved_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_content_report_per_reporter" UNIQUE ("reporter_id", "target_type", "target_id")
);
CREATE INDEX "ix_content_reports_target_id" ON "content_reports" ("target_id");
CREATE INDEX "ix_content_reports_blog_id" ON "content_reports" ("blog_id");
CREATE INDEX "ix_content_reports_status" ON "content_reports" ("status");
