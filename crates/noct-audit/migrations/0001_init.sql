-- noct: generato da `noct-ctl migrate generate` — modulo=audit versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum audit_actor_type
CREATE TYPE "audit_actor_type" AS ENUM ('user', 'core_token', 'user_token', 'system', 'anonymous');

-- crea tabella audit_log
CREATE TABLE "audit_log" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "occurred_at" timestamptz NOT NULL DEFAULT now(),
    "actor_type" "audit_actor_type" NOT NULL,
    "actor_id" uuid,
    "actor_label" varchar(255),
    "action" varchar(100) NOT NULL,
    "target_type" varchar(50),
    "target_id" uuid,
    "blog_id" uuid,
    "ip" varchar(45),
    "user_agent" varchar(500),
    "payload" jsonb NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_audit_log_occurred_at" ON "audit_log" ("occurred_at");
CREATE INDEX "ix_audit_log_action_occurred_at" ON "audit_log" ("action", "occurred_at");
CREATE INDEX "ix_audit_log_blog_id_occurred_at" ON "audit_log" ("blog_id", "occurred_at");
