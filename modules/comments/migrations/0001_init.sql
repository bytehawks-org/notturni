-- noct: generato da `noct-ctl migrate generate` — modulo=comments versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum comment_status
CREATE TYPE "comment_status" AS ENUM ('pending', 'approved', 'rejected');

-- crea enum comments_mode
CREATE TYPE "comments_mode" AS ENUM ('everyone', 'members', 'closed');

-- crea tabella blog_blocked_authors
CREATE TABLE "blog_blocked_authors" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "user_id" uuid REFERENCES "users" ("id") ON DELETE CASCADE,
    "email_hash" varchar(64),
    "label" varchar(255) NOT NULL,
    "note" varchar(255),
    "created_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_blog_blocked_authors_blog_id" ON "blog_blocked_authors" ("blog_id");

-- crea tabella comment_blog_settings
CREATE TABLE "comment_blog_settings" (
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "comments_mode" "comments_mode" NOT NULL DEFAULT 'members',
    "comments_auto_close_days" integer,
    PRIMARY KEY ("blog_id")
);

-- crea tabella comment_post_settings
CREATE TABLE "comment_post_settings" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "comments_mode" "comments_mode" NOT NULL,
    PRIMARY KEY ("post_id")
);

-- crea tabella comments
CREATE TABLE "comments" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "parent_id" uuid REFERENCES "comments" ("id") ON DELETE CASCADE,
    "author_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "author_display_name" varchar(255) NOT NULL,
    "author_email" varchar(255),
    "content" text NOT NULL,
    "status" "comment_status" NOT NULL DEFAULT 'pending',
    "reported_to_platform" boolean NOT NULL DEFAULT false,
    "report_note" text,
    "reported_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);
CREATE INDEX "ix_comments_post_id_status" ON "comments" ("post_id", "status");
CREATE INDEX "ix_comments_parent_id" ON "comments" ("parent_id");

-- crea tabella comments_platform_settings
CREATE TABLE "comments_platform_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "anonymous_comments_allowed" boolean NOT NULL DEFAULT true,
    PRIMARY KEY ("id")
);
