-- noct: generato da `noct-ctl migrate generate` — modulo=media versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella media_files
CREATE TABLE "media_files" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "uploader_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "object_key" varchar(512),
    "url" text NOT NULL,
    "content_type" varchar(100) NOT NULL DEFAULT '',
    "size_bytes" bigint NOT NULL DEFAULT 0,
    "alt_text" text NOT NULL DEFAULT '',
    "caption" text,
    "categories" text[] NOT NULL DEFAULT '{}',
    "is_sensitive" boolean NOT NULL DEFAULT false,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_media_files_url" UNIQUE ("url")
);
CREATE INDEX "ix_media_files_blog_id" ON "media_files" ("blog_id");
