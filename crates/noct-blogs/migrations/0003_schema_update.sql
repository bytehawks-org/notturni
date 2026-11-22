-- noct: generato da `noct-ctl migrate generate` — modulo=blogs versione=3
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum space_kind
CREATE TYPE "space_kind" AS ENUM ('blog', 'wiki', 'docs');

-- aggiunge colonna blogs.kind
ALTER TABLE "blogs" ADD COLUMN "kind" "space_kind" NOT NULL DEFAULT 'blog';
