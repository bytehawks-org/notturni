-- noct: generato da `noct-ctl migrate generate` — modulo=interests versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella interests_settings
CREATE TABLE "interests_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "items" jsonb NOT NULL DEFAULT '[]',
    PRIMARY KEY ("id")
);
