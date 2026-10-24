-- noct: generato da `noct-ctl migrate generate` — modulo=pages versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella blog_pages
CREATE TABLE "blog_pages" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "slug" varchar(255) NOT NULL,
    "locale" varchar(2) NOT NULL,
    "translation_group_id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "title" varchar(255) NOT NULL,
    "content" text NOT NULL,
    "is_published" boolean NOT NULL DEFAULT false,
    "updated_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_blog_page_slug_locale" UNIQUE ("blog_id", "slug", "locale"),
    CONSTRAINT "blog_page_translation_locale" UNIQUE ("translation_group_id", "locale")
);
CREATE INDEX "ix_blog_pages_blog_id" ON "blog_pages" ("blog_id");

-- crea tabella blog_pages_settings
CREATE TABLE "blog_pages_settings" (
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "static_pages_enabled" boolean NOT NULL DEFAULT false,
    PRIMARY KEY ("blog_id")
);

-- crea tabella platform_pages
CREATE TABLE "platform_pages" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "slug" varchar(255) NOT NULL,
    "locale" varchar(2) NOT NULL,
    "translation_group_id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "title" varchar(255) NOT NULL,
    "content" text NOT NULL,
    "is_published" boolean NOT NULL DEFAULT false,
    "updated_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_platform_page_slug_locale" UNIQUE ("slug", "locale"),
    CONSTRAINT "platform_page_translation_locale" UNIQUE ("translation_group_id", "locale")
);
