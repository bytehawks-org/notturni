-- noct: generato da `noct-ctl migrate generate` — modulo=platform versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- aggiunge colonna platform_config.mfa_required_for_admins
ALTER TABLE "platform_config" ADD COLUMN "mfa_required_for_admins" boolean NOT NULL DEFAULT false;
