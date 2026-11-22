-- noct: generato da `noct-ctl migrate generate` — modulo=platform versione=4
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella site_texts
CREATE TABLE "site_texts" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "locale" varchar(2) NOT NULL,
    "key" varchar(120) NOT NULL,
    "value" text NOT NULL,
    "updated_by" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_site_texts_locale_key" UNIQUE ("locale", "key")
);
