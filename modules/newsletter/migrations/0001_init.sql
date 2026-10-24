-- noct: generato da `noct-ctl migrate generate` — modulo=newsletter versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum newsletter_subscriber_status
CREATE TYPE "newsletter_subscriber_status" AS ENUM ('pending', 'confirmed', 'unsubscribed');

-- crea enum newsletter_campaign_kind
CREATE TYPE "newsletter_campaign_kind" AS ENUM ('post_notification', 'manual');

-- crea enum newsletter_campaign_status
CREATE TYPE "newsletter_campaign_status" AS ENUM ('draft', 'scheduled', 'sending', 'sent', 'canceled', 'failed');

-- crea tabella newsletter_blog_settings
CREATE TABLE "newsletter_blog_settings" (
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "auto_notify_enabled" boolean NOT NULL DEFAULT true,
    "sender_name" varchar(120),
    "banner_url" varchar(2048),
    "banner_alt_text" varchar(300) NOT NULL DEFAULT '',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("blog_id")
);

-- crea tabella newsletter_campaigns
CREATE TABLE "newsletter_campaigns" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "blog_id" uuid REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "kind" "newsletter_campaign_kind" NOT NULL,
    "post_id" uuid REFERENCES "posts" ("id") ON DELETE CASCADE,
    "created_by_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "subject" varchar(255) NOT NULL,
    "body_markdown" text,
    "status" "newsletter_campaign_status" NOT NULL DEFAULT 'draft',
    "scheduled_at" timestamptz,
    "sent_at" timestamptz,
    "recipient_count" integer NOT NULL DEFAULT 0,
    "failed_count" integer NOT NULL DEFAULT 0,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_newsletter_campaign_post" UNIQUE ("post_id")
);
CREATE INDEX "ix_newsletter_campaigns_blog_id" ON "newsletter_campaigns" ("blog_id");
CREATE INDEX "ix_newsletter_campaigns_status_scheduled_at" ON "newsletter_campaigns" ("status", "scheduled_at");

-- crea tabella newsletter_platform_settings
CREATE TABLE "newsletter_platform_settings" (
    "id" integer NOT NULL DEFAULT 1,
    "sender_name" varchar(120),
    "banner_url" varchar(2048),
    "banner_alt_text" varchar(300) NOT NULL DEFAULT '',
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id")
);

-- crea tabella newsletter_subscribers
CREATE TABLE "newsletter_subscribers" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "list_key" varchar(64) NOT NULL,
    "blog_id" uuid REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "email" varchar(255) NOT NULL,
    "locale" varchar(2),
    "status" "newsletter_subscriber_status" NOT NULL DEFAULT 'pending',
    "user_id" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "confirm_nonce" uuid,
    "confirm_expires_at" timestamptz,
    "consent_ip" varchar(64),
    "consent_user_agent" varchar(512),
    "confirmed_at" timestamptz,
    "unsubscribed_at" timestamptz,
    "unsubscribe_reason" varchar(500),
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_newsletter_subscriber_list_email" UNIQUE ("list_key", "email")
);
CREATE INDEX "ix_newsletter_subscribers_blog_id_status" ON "newsletter_subscribers" ("blog_id", "status");
CREATE INDEX "ix_newsletter_subscribers_user_id" ON "newsletter_subscribers" ("user_id");

-- crea tabella newsletter_deliveries
CREATE TABLE "newsletter_deliveries" (
    "campaign_id" uuid NOT NULL REFERENCES "newsletter_campaigns" ("id") ON DELETE CASCADE,
    "subscriber_id" uuid NOT NULL REFERENCES "newsletter_subscribers" ("id") ON DELETE CASCADE,
    "sent_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("campaign_id", "subscriber_id")
);
