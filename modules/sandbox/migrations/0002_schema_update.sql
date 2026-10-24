-- noct: generato da `noct-ctl migrate generate` — modulo=sandbox versione=2
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea tabella sandbox_blog_marks
CREATE TABLE "sandbox_blog_marks" (
    "blog_id" uuid NOT NULL REFERENCES "blogs" ("id") ON DELETE CASCADE,
    "theme" varchar(20) NOT NULL,
    PRIMARY KEY ("blog_id")
);

-- crea tabella sandbox_post_marks
CREATE TABLE "sandbox_post_marks" (
    "post_id" uuid NOT NULL REFERENCES "posts" ("id") ON DELETE CASCADE,
    "mood" varchar(20) NOT NULL,
    PRIMARY KEY ("post_id")
);
