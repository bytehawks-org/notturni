-- noct: generato da `noct-ctl migrate generate` — modulo=events versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum outbox_kind
CREATE TYPE "outbox_kind" AS ENUM ('event', 'job');

-- crea tabella noct_outbox
CREATE TABLE "noct_outbox" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "kind" "outbox_kind" NOT NULL,
    "subject" varchar(200) NOT NULL,
    "payload" jsonb NOT NULL DEFAULT '{}'::jsonb,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "published_at" timestamptz,
    "attempts" integer NOT NULL DEFAULT 0,
    "last_error" text,
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_noct_outbox_published_at_created_at" ON "noct_outbox" ("published_at", "created_at");
