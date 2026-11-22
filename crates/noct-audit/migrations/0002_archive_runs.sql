-- noct: generato da `noct-ctl migrate generate` — modulo=audit versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella audit_archive_runs
CREATE TABLE "audit_archive_runs" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "period_start" timestamptz NOT NULL,
    "period_end" timestamptz NOT NULL,
    "week_label" varchar(10) NOT NULL,
    "object_key" varchar(255),
    "storage_backend" varchar(20) NOT NULL,
    "row_count" integer NOT NULL,
    "byte_size" bigint NOT NULL DEFAULT 0,
    "sha256" varchar(64),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_audit_archive_week" UNIQUE ("week_label")
);
CREATE INDEX "ix_audit_archive_runs_period_end" ON "audit_archive_runs" ("period_end");
