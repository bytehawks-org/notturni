-- noct: generato da `noct-ctl migrate generate` — modulo=discovery versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella blog_follows
CREATE TABLE "blog_follows" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "follower_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_blog_follow_pair" UNIQUE ("follower_id", "blog_id")
);
CREATE INDEX "ix_blog_follows_blog_id" ON "blog_follows" ("blog_id");

-- crea tabella post_reads_daily
CREATE TABLE "post_reads_daily" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "day" date NOT NULL,
    "reads" integer NOT NULL DEFAULT 0,
    PRIMARY KEY ("post_id", "day")
);

-- crea tabella user_follows
CREATE TABLE "user_follows" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "follower_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "followed_user_id" uuid NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_user_follow_pair" UNIQUE ("follower_id", "followed_user_id")
);
CREATE INDEX "ix_user_follows_followed_user_id" ON "user_follows" ("followed_user_id");
