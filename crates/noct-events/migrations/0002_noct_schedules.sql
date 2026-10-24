-- noct: generato da `noct-ctl migrate generate` — modulo=events versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella noct_schedules
CREATE TABLE "noct_schedules" (
    "name" varchar(200) NOT NULL,
    "last_run_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("name")
);
