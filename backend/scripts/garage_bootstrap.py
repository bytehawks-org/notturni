"""Bootstrap idempotente di Garage (storage S3-compatible): layout del nodo,
chiave applicativa e bucket.

Un Garage appena avviato non è utilizzabile: il nodo non ha ruolo nel layout
(l'admin API /health risponde 503), non esiste nessuna chiave e i bucket
creati via S3 hanno solo un alias *locale* alla chiave, che il web endpoint
(letture pubbliche) non risolve. Questo script porta Garage allo stato
atteso dall'applicazione, ed è sicuro da rieseguire a ogni avvio:

1. assegna il nodo al layout (zona `dc1`, capacità `NOCT_GARAGE_NODE_CAPACITY_BYTES`)
   se non ha ancora un ruolo;
2. importa la chiave `NOCT_S3_ACCESS_KEY_ID`/`NOCT_S3_SECRET_ACCESS_KEY`
   (formato Garage: `GK` + 24 hex / 64 hex);
3. crea i bucket con alias globale, concede alla chiave read/write/owner e
   abilita il website access sui soli bucket pubblici (avatar, contenuti).

Uso (dalla directory backend/, con il venv attivo):
    python -m scripts.garage_bootstrap
"""

import logging
import sys
import time

import httpx

from app.core.config import settings

logging.basicConfig(level=logging.INFO, format="%(message)s")
logging.getLogger("httpx").setLevel(logging.WARNING)
logger = logging.getLogger("garage_bootstrap")

LAYOUT_ZONE = "dc1"
READY_TIMEOUT_SECONDS = 60


def _bucket_plan() -> list[tuple[str, bool]]:
    """(nome bucket, pubblico) di tutti i bucket applicativi."""
    plan = [
        (settings.s3_bucket_avatars, True),
        (settings.s3_bucket_content, True),
        (settings.s3_bucket_audit, False),
    ]
    # Destinazione di backup sullo stesso Garage (sviluppo locale, vedi
    # compose.yaml): in produzione punta a un provider esterno e non ci
    # riguarda.
    if settings.backup_s3_bucket and settings.backup_s3_endpoint_url == settings.s3_endpoint_url:
        plan.append((settings.backup_s3_bucket, False))
    return plan


def _wait_ready(client: httpx.Client) -> dict:
    deadline = time.monotonic() + READY_TIMEOUT_SECONDS
    while True:
        try:
            response = client.get("/v2/GetClusterStatus")
            if response.status_code == 200:
                return response.json()
            if response.status_code in (401, 403):
                sys.exit("Garage admin API: token rifiutato (NOCT_GARAGE_ADMIN_TOKEN).")
        except httpx.TransportError:
            pass
        if time.monotonic() > deadline:
            sys.exit(f"Garage admin API non raggiungibile su {settings.garage_admin_url} dopo {READY_TIMEOUT_SECONDS}s.")
        time.sleep(1)


def ensure_layout(client: httpx.Client, status: dict) -> None:
    node = next((n for n in status["nodes"] if n.get("isUp")), None)
    if node is None:
        sys.exit("Garage: nessun nodo attivo nel cluster.")
    if node.get("role") is not None:
        logger.info("Layout: nodo %s già assegnato.", node["id"][:16])
        return
    client.post(
        "/v2/UpdateClusterLayout",
        json={
            "roles": [
                {
                    "id": node["id"],
                    "zone": LAYOUT_ZONE,
                    "capacity": settings.garage_node_capacity_bytes,
                    "tags": [],
                }
            ]
        },
    ).raise_for_status()
    client.post("/v2/ApplyClusterLayout", json={"version": status["layoutVersion"] + 1}).raise_for_status()
    logger.info("Layout: nodo %s assegnato (%d byte).", node["id"][:16], settings.garage_node_capacity_bytes)


def ensure_key(client: httpx.Client) -> None:
    key_id, secret = settings.s3_access_key_id, settings.s3_secret_access_key
    if not key_id or not secret:
        sys.exit("NOCT_S3_ACCESS_KEY_ID/NOCT_S3_SECRET_ACCESS_KEY obbligatorie per il bootstrap di Garage.")
    if client.get("/v2/GetKeyInfo", params={"id": key_id}).status_code == 200:
        logger.info("Chiave %s già presente.", key_id)
        return
    response = client.post(
        "/v2/ImportKey", json={"accessKeyId": key_id, "secretAccessKey": secret, "name": settings.site_slug}
    )
    # più repliche del backend possono eseguire il bootstrap in parallelo
    # (k8s/backend.yaml, initContainer): se un'altra l'ha importata nel
    # frattempo la chiave c'è, ed è ciò che serve.
    if response.status_code >= 400 and client.get("/v2/GetKeyInfo", params={"id": key_id}).status_code != 200:
        sys.exit(
            f"Import chiave rifiutato da Garage ({response.status_code}): {response.text}\n"
            "Formato richiesto: NOCT_S3_ACCESS_KEY_ID = 'GK' + 24 caratteri hex, "
            "NOCT_S3_SECRET_ACCESS_KEY = 64 caratteri hex."
        )
    logger.info("Chiave %s importata.", key_id)


def ensure_bucket(client: httpx.Client, name: str, *, public: bool) -> None:
    response = client.get("/v2/GetBucketInfo", params={"globalAlias": name})
    if response.status_code == 404:
        response = client.post("/v2/CreateBucket", json={"globalAlias": name})
        if response.status_code >= 400:  # creato nel frattempo da un'altra istanza dello script
            response = client.get("/v2/GetBucketInfo", params={"globalAlias": name})
    response.raise_for_status()
    bucket_id = response.json()["id"]

    client.post(
        "/v2/AllowBucketKey",
        json={
            "bucketId": bucket_id,
            "accessKeyId": settings.s3_access_key_id,
            "permissions": {"read": True, "write": True, "owner": True},
        },
    ).raise_for_status()
    client.post(
        "/v2/UpdateBucket",
        params={"id": bucket_id},
        json={"websiteAccess": {"enabled": True, "indexDocument": "index.html"} if public else {"enabled": False}},
    ).raise_for_status()
    logger.info("Bucket %s pronto (%s).", name, "pubblico" if public else "privato")


def main() -> None:
    if not settings.garage_admin_token:
        sys.exit("NOCT_GARAGE_ADMIN_TOKEN obbligatorio.")
    with httpx.Client(
        base_url=settings.garage_admin_url,
        headers={"Authorization": f"Bearer {settings.garage_admin_token}"},
        timeout=15,
    ) as client:
        status = _wait_ready(client)
        ensure_layout(client, status)
        ensure_key(client)
        for name, public in _bucket_plan():
            ensure_bucket(client, name, public=public)


if __name__ == "__main__":
    main()
