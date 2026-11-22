-- noct: generato da `noct-ctl migrate generate` — modulo=fragments versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella post_fragments
CREATE TABLE "post_fragments" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "text" text NOT NULL,
    "text_hash" varchar(32) NOT NULL,
    "is_public" boolean NOT NULL DEFAULT false,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_post_fragment_user_post_text" UNIQUE ("user_id", "post_id", "text_hash")
);
CREATE INDEX "ix_post_fragments_post_id" ON "post_fragments" ("post_id");
