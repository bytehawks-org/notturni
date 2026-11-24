#!/usr/bin/env bash
# Crea .env da .env.example sostituendo i segnaposto con segreti casuali.
# Uso: scripts/gen-env.sh [--platform] [destinazione]
#   default: .env; rifiuta di sovrascrivere un file esistente.
#   --platform: genera anche i segreti S3/Garage (righe commentate in .env.example) e le decommenta.
set -euo pipefail

platform=0
args=()
for a in "$@"; do
  case "$a" in
    --platform) platform=1 ;;
    *) args+=("$a") ;;
  esac
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="${args[0]:-$root/.env}"
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
  [NOCT_REVALIDATE_SECRET]="$(rand_hex 24)"
)
if [ "$platform" -eq 1 ]; then
  v[NOCT_S3_ACCESS_KEY_ID]="GK$(rand_hex 12)"
  v[NOCT_S3_SECRET_ACCESS_KEY]="$(rand_hex 32)"
  v[NOCT_GARAGE_RPC_SECRET]="$(rand_hex 32)"
  v[NOCT_GARAGE_ADMIN_TOKEN]="$(rand_b64 32)"
fi

cp "$root/.env.example" "$dest"
chmod 600 "$dest"
for k in "${!v[@]}"; do
  # i valori base64 contengono / + = ma mai |
  # also matches the commented platform keys ("# KEY=...") and activates them
  sed -i "s|^#\\{0,1\\}[[:space:]]*$k=.*|$k=${v[$k]}|" "$dest"
done

echo "Creato $dest (permessi 600). Super Admin: superadmin / $admin_pw"
[ "$platform" -eq 1 ] && echo "Generati anche i segreti S3/Garage (--platform)."
echo "Rivedere almeno NOCT_INSTANCE_FQDN, NOCT_SUPER_ADMIN_EMAIL e, fuori da una prova locale, CORS/cookie/SMTP."
