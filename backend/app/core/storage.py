import json
import uuid
from pathlib import Path

import boto3
from botocore.client import Config

from app.core.config import settings

ALLOWED_AVATAR_CONTENT_TYPES = {
    "image/png": "png",
    "image/jpeg": "jpg",
    "image/webp": "webp",
}
MAX_AVATAR_SIZE_BYTES = 2 * 1024 * 1024  # 2 MiB


def build_s3_client(
    *,
    endpoint_url: str | None,
    region: str | None,
    access_key_id: str | None,
    secret_access_key: str | None,
):
    # CLAUDE.md #3: endpoint custom sempre iniettato (se impostato), per
    # compatibilità trasparente tra Garage locale e AWS/Cloudflare R2 in
    # produzione. Credenziali esplicite solo se entrambe presenti: altrimenti
    # ci si affida alla default credential chain di boto3 (ruolo AWS —
    # instance profile/IRSA, variabili AWS_* d'ambiente, ecc.). Parametrizzata
    # (invece di leggere `settings` direttamente) così il backup su S3
    # esterno (app/workers/backup.py) può costruire un client verso un
    # provider diverso da quello applicativo, con le stesse regole.
    # path-style esplicito: Garage (e gli S3-compatible self-hosted in genere)
    # non risolvono `bucket.host` in DNS. Checksum solo quando richiesti dalla
    # singola operazione: i default di boto3 >= 1.36 aggiungono CRC32 a ogni
    # put_object, che non tutti gli S3-compatible accettano.
    kwargs: dict = {
        "config": Config(
            signature_version="s3v4",
            s3={"addressing_style": "path"},
            request_checksum_calculation="when_required",
            response_checksum_validation="when_required",
        )
    }
    if endpoint_url:
        kwargs["endpoint_url"] = endpoint_url
    if region:
        kwargs["region_name"] = region
    if access_key_id and secret_access_key:
        kwargs["aws_access_key_id"] = access_key_id
        kwargs["aws_secret_access_key"] = secret_access_key
    return boto3.client("s3", **kwargs)


def get_s3_client():
    return build_s3_client(
        endpoint_url=settings.s3_endpoint_url,
        region=settings.s3_region,
        access_key_id=settings.s3_access_key_id,
        secret_access_key=settings.s3_secret_access_key,
    )


# Bucket già verificati/configurati in questo processo: ensure_* girava ad
# ogni upload (head_bucket + put_bucket_policy = 2 round trip verso lo storage
# in più per ogni file caricato), ma la configurazione di un bucket non cambia
# durante la vita del processo.
_ensured_buckets: set[tuple[str, str]] = set()


def _ensure_bucket(client, bucket: str) -> None:
    try:
        client.head_bucket(Bucket=bucket)
    except Exception:
        client.create_bucket(Bucket=bucket)


def _make_public_readable(client, bucket: str, *, resource: str) -> None:
    """Lettura anonima dei soli oggetti che corrispondono a `resource`
    (ARN S3), secondo `settings.s3_public_access`."""
    if settings.s3_public_access == "website":
        # Garage: niente bucket policy (PutBucketPolicy -> 501). L'hosting
        # statico rende il bucket servibile dal web endpoint; la restrizione
        # per prefisso (`resource`) è applicata dal proxy di lettura
        # (s3-public), non dallo storage. Il bucket deve avere un alias
        # globale (garage_bootstrap.py): quelli creati via S3 hanno solo un
        # alias locale alla chiave e il web endpoint non li risolve.
        client.put_bucket_website(Bucket=bucket, WebsiteConfiguration={"IndexDocument": {"Suffix": "index.html"}})
        return
    policy = {
        "Version": "2012-10-17",
        "Statement": [
            {
                "Effect": "Allow",
                "Principal": "*",
                "Action": "s3:GetObject",
                "Resource": resource,
            }
        ],
    }
    client.put_bucket_policy(Bucket=bucket, Policy=json.dumps(policy))


def ensure_public_bucket(bucket: str) -> None:
    """Crea il bucket se manca e lo rende leggibile pubblicamente: gli
    avatar sono pensati per essere serviti direttamente via URL, non tramite
    presigned URL con scadenza. No-op sul backend "localstorage" (niente
    bucket/policy su filesystem)."""
    if settings.storage_backend != "s3" or ("public", bucket) in _ensured_buckets:
        return
    client = get_s3_client()
    _ensure_bucket(client, bucket)
    _make_public_readable(client, bucket, resource=f"arn:aws:s3:::{bucket}/*")
    _ensured_buckets.add(("public", bucket))


def ensure_content_bucket(bucket: str) -> None:
    """Crea il bucket se manca, con lettura pubblica solo sul prefisso
    .../media/... (embed nei post) — .../posts/... (backup markdown) resta
    privato: è un fallback interno, non un asset da servire ai visitatori.
    No-op sul backend "localstorage"."""
    if settings.storage_backend != "s3" or ("content", bucket) in _ensured_buckets:
        return
    client = get_s3_client()
    _ensure_bucket(client, bucket)
    _make_public_readable(client, bucket, resource=f"arn:aws:s3:::{bucket}/*/userdata/*/*/media/*")
    _ensured_buckets.add(("content", bucket))


def ensure_private_bucket(bucket: str) -> None:
    """Crea il bucket se manca, senza alcuna policy di lettura pubblica: gli
    archivi di audit non vanno mai serviti ai visitatori. No-op sul backend
    "localstorage" (niente bucket/policy su filesystem)."""
    if settings.storage_backend != "s3" or ("private", bucket) in _ensured_buckets:
        return
    _ensure_bucket(get_s3_client(), bucket)
    _ensured_buckets.add(("private", bucket))


def _userdata_prefix(user_id: str, blog_id: str) -> str:
    return f"{settings.site_slug}/userdata/{user_id}/{blog_id}"


def _put_object(*, bucket: str, key: str, content: bytes, content_type: str) -> None:
    if settings.storage_backend == "localstorage":
        path = Path(settings.local_storage_base_path) / bucket / key
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return
    get_s3_client().put_object(Bucket=bucket, Key=key, Body=content, ContentType=content_type)


def _delete_object(*, bucket: str, key: str) -> None:
    if settings.storage_backend == "localstorage":
        (Path(settings.local_storage_base_path) / bucket / key).unlink(missing_ok=True)
        return
    get_s3_client().delete_object(Bucket=bucket, Key=key)


def _public_url(*, bucket: str, key: str) -> str:
    if settings.storage_backend == "localstorage":
        return f"{settings.local_storage_public_url}/{bucket}/{key}"
    return f"{settings.s3_public_base_url}/{bucket}/{key}"


ALLOWED_MEDIA_CONTENT_TYPES = {
    "image/png": "png",
    "image/jpeg": "jpg",
    "image/webp": "webp",
    "image/gif": "gif",
}
MAX_MEDIA_SIZE_BYTES = 10 * 1024 * 1024  # 10 MiB


def upload_media(*, user_id: uuid.UUID, blog_id: uuid.UUID, content: bytes, content_type: str) -> str:
    """{bucket}/{site_slug}/userdata/{user_id}/{blog_id}/media/{uuid}.{ext}
    — immagini incorporabili nel Markdown dei post. Pubblico in lettura."""
    if content_type not in ALLOWED_MEDIA_CONTENT_TYPES:
        raise ValueError("Formato immagine non supportato (usare PNG, JPEG, WEBP o GIF).")
    if len(content) > MAX_MEDIA_SIZE_BYTES:
        raise ValueError("Il file supera la dimensione massima di 10 MiB.")

    extension = ALLOWED_MEDIA_CONTENT_TYPES[content_type]
    object_key = f"{_userdata_prefix(str(user_id), str(blog_id))}/media/{uuid.uuid4()}.{extension}"

    ensure_content_bucket(settings.s3_bucket_content)
    _put_object(bucket=settings.s3_bucket_content, key=object_key, content=content, content_type=content_type)
    return object_key


def content_public_url(object_key: str) -> str:
    return _public_url(bucket=settings.s3_bucket_content, key=object_key)


def upload_post_backup(*, user_id: str, blog_id: str, post_id: str, content: str) -> None:
    """{bucket}/{site_slug}/userdata/{user_id}/{blog_id}/posts/{post_id}.md
    — copia di backup/fallback del Markdown, non servita pubblicamente (il
    contenuto "vero" resta il database; questa è una copia di sicurezza)."""
    object_key = f"{_userdata_prefix(user_id, blog_id)}/posts/{post_id}.md"
    ensure_content_bucket(settings.s3_bucket_content)
    _put_object(
        bucket=settings.s3_bucket_content,
        key=object_key,
        content=content.encode("utf-8"),
        content_type="text/markdown; charset=utf-8",
    )


def audit_archive_key(week_label: str) -> str:
    """Chiave dell'oggetto per l'archivio di una settimana ISO (es.
    "2026w36" -> "{site_slug}/audit-archive/2026w36.jsonl.gz")."""
    return f"{settings.site_slug}/audit-archive/{week_label}.jsonl.gz"


def upload_audit_archive(*, week_label: str, content: bytes) -> str:
    """Scrive l'archivio NDJSON gzippato di una settimana di audit nel bucket
    dedicato (privato). Ritorna la chiave dell'oggetto."""
    key = audit_archive_key(week_label)
    ensure_private_bucket(settings.s3_bucket_audit)
    _put_object(
        bucket=settings.s3_bucket_audit,
        key=key,
        content=content,
        content_type="application/gzip",
    )
    return key


def get_audit_archive(*, week_label: str) -> bytes:
    """Rilegge l'archivio di una settimana (per il restore su database)."""
    key = audit_archive_key(week_label)
    if settings.storage_backend == "localstorage":
        return (Path(settings.local_storage_base_path) / settings.s3_bucket_audit / key).read_bytes()
    return get_s3_client().get_object(Bucket=settings.s3_bucket_audit, Key=key)["Body"].read()


def upload_avatar(*, user_id: uuid.UUID, content: bytes, content_type: str) -> str:
    if content_type not in ALLOWED_AVATAR_CONTENT_TYPES:
        raise ValueError("Formato immagine non supportato (usare PNG, JPEG o WEBP).")
    if len(content) > MAX_AVATAR_SIZE_BYTES:
        raise ValueError("L'immagine supera la dimensione massima di 2 MiB.")

    extension = ALLOWED_AVATAR_CONTENT_TYPES[content_type]
    object_key = f"{user_id}/{uuid.uuid4()}.{extension}"

    ensure_public_bucket(settings.s3_bucket_avatars)
    _put_object(bucket=settings.s3_bucket_avatars, key=object_key, content=content, content_type=content_type)
    return object_key


def delete_avatar(object_key: str) -> None:
    _delete_object(bucket=settings.s3_bucket_avatars, key=object_key)


MAX_FAVICON_SIZE_BYTES = 512 * 1024  # 512 KiB — icona, non contenuto


def upload_blog_favicon(*, blog_id: uuid.UUID, content: bytes, content_type: str) -> str:
    """{bucket_avatars}/favicons/{blog_id}/{uuid}.{ext} — stesso bucket
    pubblico degli avatar (icona di identità, non contenuto: nessuna
    moderazione automatica, a differenza di upload_media)."""
    if content_type not in ALLOWED_AVATAR_CONTENT_TYPES:
        raise ValueError("Formato immagine non supportato (usare PNG, JPEG o WEBP).")
    if len(content) > MAX_FAVICON_SIZE_BYTES:
        raise ValueError("L'immagine supera la dimensione massima di 512 KiB.")

    extension = ALLOWED_AVATAR_CONTENT_TYPES[content_type]
    object_key = f"favicons/{blog_id}/{uuid.uuid4()}.{extension}"

    ensure_public_bucket(settings.s3_bucket_avatars)
    _put_object(bucket=settings.s3_bucket_avatars, key=object_key, content=content, content_type=content_type)
    return object_key


def delete_blog_favicon(object_key: str) -> None:
    _delete_object(bucket=settings.s3_bucket_avatars, key=object_key)


def blog_favicon_public_url(object_key: str) -> str:
    return _public_url(bucket=settings.s3_bucket_avatars, key=object_key)


def avatar_public_url(object_key: str) -> str:
    return _public_url(bucket=settings.s3_bucket_avatars, key=object_key)


def blog_storage_bytes(*, user_ids: list[str], blog_id: str) -> int:
    """Byte occupati dai file di un blog (media + backup Markdown) sotto i
    prefissi `{site}/userdata/{user}/{blog}/` di ogni utente indicato
    (proprietario e collaboratori: il prefisso è per chi ha caricato). Solo
    lettura, per la card "Spazio" della panoramica (mockup 5a). Bloccante:
    chiamare da `asyncio.to_thread`."""
    total = 0
    bucket = settings.s3_bucket_content
    if settings.storage_backend == "localstorage":
        for user_id in user_ids:
            root = Path(settings.local_storage_base_path) / bucket / _userdata_prefix(user_id, blog_id)
            if root.is_dir():
                total += sum(p.stat().st_size for p in root.rglob("*") if p.is_file())
        return total
    client = get_s3_client()
    try:
        client.head_bucket(Bucket=bucket)
    except Exception:  # noqa: BLE001 — bucket mai creato: nessun upload ancora fatto, 0 byte
        return 0
    paginator = client.get_paginator("list_objects_v2")
    for user_id in user_ids:
        prefix = f"{_userdata_prefix(user_id, blog_id)}/"
        for page in paginator.paginate(Bucket=bucket, Prefix=prefix):
            for obj in page.get("Contents", []) or []:
                total += int(obj.get("Size", 0))
    return total


def delete_blog_storage(*, user_ids: list[str], blog_id: str) -> int:
    """Cancella tutti gli oggetti di un blog (media + backup Markdown) sotto i
    prefissi `{site}/userdata/{user}/{blog}/` degli utenti indicati — usato
    dalla cancellazione definitiva (app/domain/blog_lifecycle.py). Ritorna il
    numero di oggetti rimossi. Bloccante: chiamare da `asyncio.to_thread`."""
    removed = 0
    bucket = settings.s3_bucket_content
    if settings.storage_backend == "localstorage":
        import shutil

        for user_id in user_ids:
            root = Path(settings.local_storage_base_path) / bucket / _userdata_prefix(user_id, blog_id)
            if root.is_dir():
                removed += sum(1 for p in root.rglob("*") if p.is_file())
                shutil.rmtree(root, ignore_errors=True)
        return removed
    client = get_s3_client()
    try:
        client.head_bucket(Bucket=bucket)
    except Exception:  # noqa: BLE001
        return 0
    paginator = client.get_paginator("list_objects_v2")
    for user_id in user_ids:
        prefix = f"{_userdata_prefix(user_id, blog_id)}/"
        for page in paginator.paginate(Bucket=bucket, Prefix=prefix):
            for obj in page.get("Contents", []) or []:
                client.delete_object(Bucket=bucket, Key=obj["Key"])
                removed += 1
    return removed


def delete_content_object(object_key: str) -> None:
    """Rimuove un singolo media dal bucket dei contenuti (libreria media, B7)."""
    _delete_object(bucket=settings.s3_bucket_content, key=object_key)
