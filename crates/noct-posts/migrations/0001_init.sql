-- noct: generato da `noct-ctl migrate generate` — modulo=posts versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum post_status
CREATE TYPE "post_status" AS ENUM ('draft', 'pending_review', 'published');

-- crea tabella posts
CREATE TABLE "posts" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "author_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE RESTRICT,
    "author_display_name" varchar(255) NOT NULL,
    "locale" varchar(2) NOT NULL DEFAULT 'it',
    "translation_group_id" uuid NOT NULL,
    "title" varchar(255) NOT NULL,
    "slug" varchar(255) NOT NULL,
    "content" text NOT NULL,
    "cover_image_url" varchar(2048),
    "cover_image_is_sensitive" boolean NOT NULL DEFAULT false,
    "cover_image_categories" text[] NOT NULL DEFAULT '{}',
    "cover_image_alt_text" varchar(300) NOT NULL DEFAULT '',
    "manual_tags" text[] NOT NULL DEFAULT '{}',
    "status" "post_status" NOT NULL DEFAULT 'draft',
    "published_at" timestamptz,
    "is_hidden" boolean NOT NULL DEFAULT false,
    "search_indexing_enabled" boolean,
    "ai_crawling_enabled" boolean,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_post_blog_slug_locale" UNIQUE ("blog_id", "slug", "locale"),
    CONSTRAINT "uq_post_translation_group_locale" UNIQUE ("translation_group_id", "locale")
);
CREATE INDEX "ix_posts_author_id" ON "posts" ("author_id");
CREATE INDEX "ix_posts_translation_group_id" ON "posts" ("translation_group_id");
CREATE INDEX "ix_posts_status_published_at" ON "posts" ("status", "published_at");

-- crea tabella tags
CREATE TABLE "tags" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "name" varchar(30) NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_tags_name" UNIQUE ("name")
);

-- crea tabella post_links
CREATE TABLE "post_links" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "position" integer NOT NULL,
    "url" text NOT NULL,
    "link_text" text NOT NULL DEFAULT '',
    PRIMARY KEY ("post_id", "position")
);

-- crea tabella post_media
CREATE TABLE "post_media" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "position" integer NOT NULL,
    "url" text NOT NULL,
    "alt_text" text NOT NULL DEFAULT '',
    "categories" text[] NOT NULL DEFAULT '{}',
    "is_sensitive" boolean NOT NULL DEFAULT false,
    PRIMARY KEY ("post_id", "position")
);
CREATE INDEX "ix_post_media_url" ON "post_media" ("url");

-- crea tabella post_tags
CREATE TABLE "post_tags" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "tag_id" uuid NOT NULL REFERENCES "tags" ("id") ON DELETE CASCADE,
    PRIMARY KEY ("post_id", "tag_id")
);
CREATE INDEX "ix_post_tags_tag_id" ON "post_tags" ("tag_id");
