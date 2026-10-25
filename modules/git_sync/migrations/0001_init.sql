-- noct: generato da `noct-ctl migrate generate` — modulo=git_sync versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella git_sync_configs
CREATE TABLE "git_sync_configs" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "owner_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "remote_url" varchar(2048) NOT NULL,
    "branch" varchar(200) NOT NULL DEFAULT 'notturni-backup',
    "username" varchar(100) NOT NULL DEFAULT 'git',
    "secret" text NOT NULL,
    "include_drafts" boolean NOT NULL DEFAULT false,
    "enabled" boolean NOT NULL DEFAULT true,
    "last_status" varchar(16),
    "last_run_at" timestamptz,
    "last_error" text,
    "last_commit" varchar(64),
    "last_pages" integer,
    "last_hash" varchar(64),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_git_sync_blog" UNIQUE ("blog_id")
);
CREATE INDEX "ix_git_sync_configs_owner_id" ON "git_sync_configs" ("owner_id");
