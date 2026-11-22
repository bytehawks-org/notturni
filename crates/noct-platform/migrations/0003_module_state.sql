-- noct: generato da `noct-ctl migrate generate` — modulo=platform versione=3
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella noct_module_state
CREATE TABLE "noct_module_state" (
    "id" integer NOT NULL DEFAULT 1,
    "version" bigint NOT NULL DEFAULT 0,
    "last_error" text,
    PRIMARY KEY ("id")
);

-- crea tabella noct_modules
CREATE TABLE "noct_modules" (
    "name" varchar(100) NOT NULL,
    "enabled" boolean NOT NULL,
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    "updated_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    PRIMARY KEY ("name")
);
