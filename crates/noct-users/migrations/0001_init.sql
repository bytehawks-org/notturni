-- noct: generato da `noct-ctl migrate generate` — modulo=users versione=1
-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).

-- crea enum platform_role
CREATE TYPE "platform_role" AS ENUM ('super_admin', 'amministratore', 'moderatore', 'utente');

-- crea tabella users
CREATE TABLE "users" (
    "id" uuid NOT NULL DEFAULT gen_random_uuid(),
    "username" varchar(32) NOT NULL,
    "email" varchar(255) NOT NULL,
    "platform_role" "platform_role" NOT NULL DEFAULT 'utente',
    "is_active" boolean NOT NULL DEFAULT true,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY ("id"),
    CONSTRAINT "uq_users_username" UNIQUE ("username"),
    CONSTRAINT "uq_users_email" UNIQUE ("email")
);
