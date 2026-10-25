-- noct: generato da `noct-ctl migrate generate` — modulo=git_sync versione=3
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella git_sync_known_hosts
CREATE TABLE "git_sync_known_hosts" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "host" varchar(255) NOT NULL,
    "port" integer NOT NULL DEFAULT 22,
    "fingerprints" text NOT NULL,
    "note" varchar(500),
    "updated_by" uuid REFERENCES "users" ("id") ON DELETE SET NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_git_sync_known_host" UNIQUE ("host", "port")
);

-- impronte iniziali dei provider più usati, come le pubblicano (verificate il 2026-10-25):
--   github.com  https://api.github.com/meta (ssh_key_fingerprints)
--   gitlab.com  https://docs.gitlab.com/user/gitlab_com/#ssh-known_hosts-entries
-- Gli amministratori le aggiornano (o le tolgono) da /admin/git-sync quando un provider ruota le chiavi.
INSERT INTO "git_sync_known_hosts" ("host", "port", "fingerprints", "note") VALUES
    ('github.com', 22,
     E'SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU\nSHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM\nSHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s',
     'GitHub (ed25519, ecdsa, rsa) — https://api.github.com/meta'),
    ('gitlab.com', 22,
     E'SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8\nSHA256:HbW3g8zUjNSksFbqTiUWPWg2Bq1x8xdGUrliXFzSnUw\nSHA256:ROQFvPThGrW4RuWLoL9tq9I9zJ42fK4XywyRtbOz/EQ',
     'GitLab.com (ed25519, ecdsa, rsa) — docs.gitlab.com')
ON CONFLICT ("host", "port") DO NOTHING;
