-- noct: generato da `noct-ctl migrate generate` — modulo=sandbox versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum sandbox_note_kind
CREATE TYPE "sandbox_note_kind" AS ENUM ('note', 'todo');

-- crea tabella sandbox_notes
CREATE TABLE "sandbox_notes" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "title" varchar(80) NOT NULL,
    "body" text,
    "kind" "sandbox_note_kind" NOT NULL DEFAULT 'note',
    "position" integer NOT NULL DEFAULT 0,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_sandbox_note_title" UNIQUE ("blog_id", "title")
);
CREATE INDEX "ix_sandbox_notes_blog_id_position" ON "sandbox_notes" ("blog_id", "position");
