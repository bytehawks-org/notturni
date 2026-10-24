-- noct: generato da `noct-ctl migrate generate` — modulo=link_preview versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella link_preview_cache
CREATE TABLE "link_preview_cache" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "url_hash" varchar(64) NOT NULL,
    "url" text NOT NULL,
    "title" text,
    "description" text,
    "image" text,
    "fetch_ok" boolean NOT NULL DEFAULT false,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_link_preview_url_hash" UNIQUE ("url_hash")
);
