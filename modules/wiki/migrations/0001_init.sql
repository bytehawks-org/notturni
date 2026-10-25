-- noct: generato da `noct-ctl migrate generate` — modulo=wiki versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella post_outline
CREATE TABLE "post_outline" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "parent_id" uuid REFERENCES "posts" ("id") ON DELETE SET NULL,
    "position" integer NOT NULL DEFAULT 0,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("post_id")
);
CREATE INDEX "ix_post_outline_parent_id" ON "post_outline" ("parent_id");

-- crea tabella post_revisions
CREATE TABLE "post_revisions" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "revision" integer NOT NULL,
    "title" varchar(255) NOT NULL,
    "content" text NOT NULL,
    "editor_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "summary" varchar(300),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_post_revision" UNIQUE ("post_id", "revision")
);
CREATE INDEX "ix_post_revisions_editor_id" ON "post_revisions" ("editor_id");

-- crea tabella post_wikilinks
CREATE TABLE "post_wikilinks" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "position" integer NOT NULL,
    "target_slug" varchar(255) NOT NULL,
    "label" text,
    PRIMARY KEY ("post_id", "position")
);
CREATE INDEX "ix_post_wikilinks_target_slug" ON "post_wikilinks" ("target_slug");
