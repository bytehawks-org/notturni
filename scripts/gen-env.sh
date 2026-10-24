#!/usr/bin/env bash
# Crea .env da .env.example sostituendo i segnaposto con segreti casuali.
# Uso: scripts/gen-env.sh [destinazione]   (default: .env; rifiuta di sovrascrivere un file esistente)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="${1:-$root/.env}"
[ -e "$dest" ] && { echo "$dest esiste già: non lo sovrascrivo" >&2; exit 1; }
command -v openssl >/dev/null || { echo "serve openssl" >&2; exit 1; }

rand_b64() { openssl rand -base64 "$1" | tr -d '\n'; }
rand_hex() { openssl rand -hex "$1" | tr -d '\n'; }
admin_pw="$(openssl rand -base64 18 | tr -d '/+=\n')"

declare -A v=(
  [NOCT_JWT_SECRET]="$(rand_b64 48)"
  [NOCT_SECRETS_KEY]="$(rand_b64 32)"
  [NOCT_SUPER_ADMIN_PASSWORD]="$admin_pw"
  [NOCT_POSTGRES_PASSWORD]="$(rand_hex 16)"
  [NOCT_S3_ACCESS_KEY_ID]="GK$(rand_hex 12)"
  [NOCT_S3_SECRET_ACCESS_KEY]="$(rand_hex 32)"
  [NOCT_GARAGE_RPC_SECRET]="$(rand_hex 32)"
  [NOCT_GARAGE_ADMIN_TOKEN]="$(rand_b64 32)"
  [NOCT_REVALIDATE_SECRET]="$(rand_hex 24)"
)

cp "$root/.env.example" "$dest"
chmod 600 "$dest"
for k in "${!v[@]}"; do
  # i valori base64 contengono / + = ma mai |
  sed -i "s|^$k=.*|$k=${v[$k]}|" "$dest"
done

echo "Creato $dest (permessi 600). Super Admin: superadmin / $admin_pw"
echo "Rivedere almeno NOCT_INSTANCE_FQDN, NOCT_SUPER_ADMIN_EMAIL e, fuori da una prova locale, CORS/cookie/SMTP."
