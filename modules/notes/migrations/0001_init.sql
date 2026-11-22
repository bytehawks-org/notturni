-- noct: generato da `noct-ctl migrate generate` — modulo=notes versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella blog_notes
CREATE TABLE "blog_notes" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "content" text NOT NULL,
    "normalized" varchar(600) NOT NULL,
    "kind" varchar(10) NOT NULL DEFAULT 'note',
    "url" text,
    "created_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "title" text,
    "author" text,
    "isbn" varchar(32),
    "doi" varchar(255),
    "page" varchar(32),
    "source" text,
    "issued" varchar(32),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_blog_notes_blog_id_normalized" ON "blog_notes" ("blog_id", "normalized");

-- crea tabella post_notes
CREATE TABLE "post_notes" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "idx" integer NOT NULL,
    "content" text NOT NULL,
    "note_id" uuid REFERENCES "blog_notes" ("id") ON DELETE SET NULL,
    "title" text,
    "author" text,
    "isbn" varchar(32),
    "doi" varchar(255),
    "page" varchar(32),
    "kind" varchar(10),
    "source" text,
    "issued" varchar(32),
    "url" text,
    PRIMARY KEY ("post_id", "idx")
);
CREATE INDEX "ix_post_notes_note_id" ON "post_notes" ("note_id");
