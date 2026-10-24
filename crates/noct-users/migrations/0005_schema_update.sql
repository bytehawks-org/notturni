-- noct: generato da `noct-ctl migrate generate` — modulo=users versione=5
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- enum post_author_name_style: aggiunge il valore verified_domain
ALTER TYPE "post_author_name_style" ADD VALUE IF NOT EXISTS 'verified_domain';
