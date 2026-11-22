-- noct: generato da `noct-ctl migrate generate` — modulo=blogs versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum blog_visibility
CREATE TYPE "blog_visibility" AS ENUM ('public', 'members', 'private');

-- crea enum blog_role
CREATE TYPE "blog_role" AS ENUM ('autore', 'co_autore', 'revisore', 'mediatore');

-- crea tabella blogs
CREATE TABLE "blogs" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "slug" varchar(63) NOT NULL,
    "title" varchar(255) NOT NULL,
    "owner_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE RESTRICT,
    "visibility" "blog_visibility" NOT NULL DEFAULT 'public',
    "is_suspended" boolean NOT NULL DEFAULT false,
    "is_paused" boolean NOT NULL DEFAULT false,
    "deleted_at" timestamptz,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_blogs_slug" UNIQUE ("slug")
);
CREATE INDEX "ix_blogs_owner_id" ON "blogs" ("owner_id");

-- crea tabella blog_memberships
CREATE TABLE "blog_memberships" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "role" "blog_role" NOT NULL DEFAULT 'autore',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_membership_blog_user" UNIQUE ("blog_id", "user_id")
);
