-- noct: generato da `noct-ctl migrate generate` — modulo=taxonomy versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella post_categories
CREATE TABLE "post_categories" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "category_id" uuid NOT NULL REFERENCES "categories" ("id") ON DELETE CASCADE,
    PRIMARY KEY ("post_id")
);
CREATE INDEX "ix_post_categories_category_id" ON "post_categories" ("category_id");
