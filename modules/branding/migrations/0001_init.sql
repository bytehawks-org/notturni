-- noct: generato da `noct-ctl migrate generate` — modulo=branding versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella site_branding
CREATE TABLE "site_branding" (
    "id" integer NOT NULL DEFAULT 1,
    "site_name" varchar(100),
    "icon_key" varchar(255),
    "icon_sizes" varchar(64),
    "social_image_key" varchar(255),
    "noindex" boolean NOT NULL DEFAULT false,
    "google_verification" varchar(200),
    "bing_verification" varchar(200),
    "yandex_verification" varchar(200),
    "twitter_site" varchar(50),
    "theme_color" varchar(7),
    "extra_meta" jsonb NOT NULL DEFAULT '[]',
    "updated_by" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);

-- crea tabella site_seo
CREATE TABLE "site_seo" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "locale" varchar(2) NOT NULL,
    "title" varchar(120),
    "description" varchar(400),
    "keywords" varchar(300),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_site_seo_locale" UNIQUE ("locale")
);
