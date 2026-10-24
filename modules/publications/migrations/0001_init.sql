-- noct: generato da `noct-ctl migrate generate` — modulo=publications versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella publications
CREATE TABLE "publications" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "name" varchar(60) NOT NULL,
    "title" varchar(255) NOT NULL,
    "description" text,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_publication_blog_name" UNIQUE ("blog_id", "name")
);
CREATE INDEX "ix_publications_blog_id" ON "publications" ("blog_id");

-- crea tabella post_publications
CREATE TABLE "post_publications" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "publication_id" uuid NOT NULL REFERENCES "publications" ("id") ON DELETE CASCADE,
    "chapter_order" integer,
    PRIMARY KEY ("post_id")
);
CREATE INDEX "ix_post_publications_publication_id" ON "post_publications" ("publication_id");
