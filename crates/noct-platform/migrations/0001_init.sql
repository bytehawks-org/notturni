-- noct: generato da `noct-ctl migrate generate` — modulo=platform versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella platform_config
CREATE TABLE "platform_config" (
    "id" integer NOT NULL DEFAULT 1,
    "default_locale" varchar(2) NOT NULL DEFAULT 'it',
    "registration_mode" varchar(10) NOT NULL DEFAULT 'open',
    "reserved_blog_names" text[] NOT NULL DEFAULT '{}',
    "moderation_threshold" double precision NOT NULL DEFAULT 0.8,
    "max_blogs_per_user" integer NOT NULL DEFAULT 5,
    "max_blog_storage_mb" integer,
    "audit_retention_days" integer NOT NULL DEFAULT 105,
    "footer_column1_markdown" text,
    "footer_column2_markdown" text,
    "footer_column3_markdown" text,
    "footer_bottom_bar_markdown" text,
    "updated_at" timestamptz,
    "updated_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    PRIMARY KEY ("id")
);
