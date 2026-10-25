-- noct: generato da `noct-ctl migrate generate` — modulo=blogs versione=4
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum space_display_mode
CREATE TYPE "space_display_mode" AS ENUM ('feed', 'wiki', 'docs');

-- aggiunge colonna blogs.display_mode
ALTER TABLE "blogs" ADD COLUMN "display_mode" "space_display_mode" NOT NULL DEFAULT 'feed';
