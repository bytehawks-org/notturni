-- noct: generato da `noct-ctl migrate generate` — modulo=users versione=4
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- aggiunge colonna users.bio
ALTER TABLE "users" ADD COLUMN "bio" text;

-- aggiunge colonna users.country
ALTER TABLE "users" ADD COLUMN "country" varchar(2);

-- aggiunge colonna users.native_language
ALTER TABLE "users" ADD COLUMN "native_language" varchar(2);

-- aggiunge colonna users.fallback_languages
ALTER TABLE "users" ADD COLUMN "fallback_languages" text[] NOT NULL DEFAULT '{}';

-- aggiunge colonna users.interests
ALTER TABLE "users" ADD COLUMN "interests" text[] NOT NULL DEFAULT '{}';

-- aggiunge colonna users.directory_listed
ALTER TABLE "users" ADD COLUMN "directory_listed" boolean NOT NULL DEFAULT true;

-- aggiunge colonna users.username_changed_at
ALTER TABLE "users" ADD COLUMN "username_changed_at" timestamptz;

-- aggiunge colonna users.verified_domain
ALTER TABLE "users" ADD COLUMN "verified_domain" varchar(255);
