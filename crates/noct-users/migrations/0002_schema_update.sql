-- noct: generato da `noct-ctl migrate generate` — modulo=users versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- aggiunge colonna users.hashed_password
ALTER TABLE "users" ADD COLUMN "hashed_password" varchar(255);

-- aggiunge colonna users.credentials_changed_at
ALTER TABLE "users" ADD COLUMN "credentials_changed_at" timestamptz;

-- aggiunge colonna users.mfa_enabled
ALTER TABLE "users" ADD COLUMN "mfa_enabled" boolean NOT NULL DEFAULT false;

-- aggiunge colonna users.display_name
ALTER TABLE "users" ADD COLUMN "display_name" varchar(255);

-- aggiunge colonna users.ui_locale
ALTER TABLE "users" ADD COLUMN "ui_locale" varchar(2);
