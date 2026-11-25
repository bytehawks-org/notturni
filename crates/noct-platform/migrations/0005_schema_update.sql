-- noct: generato da `noct-ctl migrate generate` — modulo=platform versione=5
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- aggiunge colonna platform_config.primary_blog_id
ALTER TABLE "platform_config" ADD COLUMN "primary_blog_id" uuid;
