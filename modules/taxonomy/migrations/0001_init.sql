-- noct: generato da `noct-ctl migrate generate` — modulo=taxonomy versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella categories
CREATE TABLE "categories" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "name" varchar(50) NOT NULL,
    "slug" varchar(60) NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_category_blog_slug" UNIQUE ("blog_id", "slug")
);
