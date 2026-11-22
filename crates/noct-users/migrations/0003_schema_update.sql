-- noct: generato da `noct-ctl migrate generate` — modulo=users versione=3
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum post_author_name_style
CREATE TYPE "post_author_name_style" AS ENUM ('username', 'full_name', 'display_name');

-- aggiunge colonna users.first_name
ALTER TABLE "users" ADD COLUMN "first_name" varchar(100);

-- aggiunge colonna users.last_name
ALTER TABLE "users" ADD COLUMN "last_name" varchar(100);

-- aggiunge colonna users.avatar_object_key
ALTER TABLE "users" ADD COLUMN "avatar_object_key" varchar(255);

-- aggiunge colonna users.post_author_name_style
ALTER TABLE "users" ADD COLUMN "post_author_name_style" "post_author_name_style" NOT NULL DEFAULT 'username';
