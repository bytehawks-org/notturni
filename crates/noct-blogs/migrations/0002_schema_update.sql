-- noct: generato da `noct-ctl migrate generate` — modulo=blogs versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum blog_invitation_status
CREATE TYPE "blog_invitation_status" AS ENUM ('pending', 'accepted', 'declined', 'revoked');

-- crea tabella blog_configs
CREATE TABLE "blog_configs" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "config" jsonb NOT NULL DEFAULT '{}',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_blog_configs_blog" UNIQUE ("blog_id")
);

-- crea tabella blog_invitations
CREATE TABLE "blog_invitations" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "invited_user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "invited_by_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "role" "blog_role" NOT NULL,
    "status" "blog_invitation_status" NOT NULL DEFAULT 'pending',
    "responded_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_invitation_blog_user" UNIQUE ("blog_id", "invited_user_id")
);
CREATE INDEX "ix_blog_invitations_invited_user_id" ON "blog_invitations" ("invited_user_id");

-- aggiunge colonna blogs.subtitle
ALTER TABLE "blogs" ADD COLUMN "subtitle" varchar(64);

-- aggiunge colonna blogs.description
ALTER TABLE "blogs" ADD COLUMN "description" varchar(256);

-- aggiunge colonna blogs.default_locale
ALTER TABLE "blogs" ADD COLUMN "default_locale" varchar(2) NOT NULL DEFAULT 'it';

-- aggiunge colonna blogs.extra_locales
ALTER TABLE "blogs" ADD COLUMN "extra_locales" text[] NOT NULL DEFAULT '{}';

-- aggiunge colonna blogs.default_author_display_name
ALTER TABLE "blogs" ADD COLUMN "default_author_display_name" varchar(255);

-- aggiunge colonna blogs.mentions_enabled
ALTER TABLE "blogs" ADD COLUMN "mentions_enabled" boolean NOT NULL DEFAULT true;

-- aggiunge colonna blogs.search_indexing_enabled
ALTER TABLE "blogs" ADD COLUMN "search_indexing_enabled" boolean NOT NULL DEFAULT true;

-- aggiunge colonna blogs.ai_crawling_enabled
ALTER TABLE "blogs" ADD COLUMN "ai_crawling_enabled" boolean NOT NULL DEFAULT true;

-- aggiunge colonna blogs.cover_image_url
ALTER TABLE "blogs" ADD COLUMN "cover_image_url" varchar(2048);

-- aggiunge colonna blogs.cover_image_is_sensitive
ALTER TABLE "blogs" ADD COLUMN "cover_image_is_sensitive" boolean NOT NULL DEFAULT false;

-- aggiunge colonna blogs.cover_image_categories
ALTER TABLE "blogs" ADD COLUMN "cover_image_categories" text[] NOT NULL DEFAULT '{}';

-- aggiunge colonna blogs.cover_image_alt_text
ALTER TABLE "blogs" ADD COLUMN "cover_image_alt_text" varchar(300) NOT NULL DEFAULT '';

-- aggiunge colonna blogs.favicon_object_key
ALTER TABLE "blogs" ADD COLUMN "favicon_object_key" varchar(255);

-- aggiunge colonna blog_memberships.author_display_name
ALTER TABLE "blog_memberships" ADD COLUMN "author_display_name" varchar(255);

-- crea indice ix_blog_memberships_user_id
CREATE INDEX "ix_blog_memberships_user_id" ON "blog_memberships" ("user_id");
