-- noct: generato da `noct-ctl migrate generate` — modulo=legal versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella legal_documents
CREATE TABLE "legal_documents" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "doc" varchar(16) NOT NULL,
    "locale" varchar(2) NOT NULL,
    "title" varchar(255) NOT NULL,
    "content" text NOT NULL,
    "customized" boolean NOT NULL DEFAULT false,
    "published_hash" varchar(64),
    "published_at" timestamptz,
    "updated_by" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_legal_doc_locale" UNIQUE ("doc", "locale")
);

-- crea tabella legal_settings
CREATE TABLE "legal_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "controller_name" varchar(255),
    "controller_address" varchar(500),
    "controller_email" varchar(255),
    "controller_registry" varchar(255),
    "dpo_contact" varchar(500),
    "hosting_description" varchar(500),
    "processors" varchar(1000),
    "backup_retention" varchar(255),
    "audit_archive_retention" varchar(255),
    "minimum_age" integer,
    "updated_by" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
