-- noct: generato da `noct-ctl migrate generate` — modulo=git_sync versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- aggiunge colonna git_sync_configs.auth_method
ALTER TABLE "git_sync_configs" ADD COLUMN "auth_method" varchar(8) NOT NULL DEFAULT 'https';

-- aggiunge colonna git_sync_configs.ssh_public_key
ALTER TABLE "git_sync_configs" ADD COLUMN "ssh_public_key" text;

-- aggiunge colonna git_sync_configs.ssh_host_key
ALTER TABLE "git_sync_configs" ADD COLUMN "ssh_host_key" text;

-- aggiunge colonna git_sync_configs.ssh_host_fingerprints
ALTER TABLE "git_sync_configs" ADD COLUMN "ssh_host_fingerprints" text;
