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


def get_s3_client():
    # CLAUDE.md #4: endpoint custom sempre iniettato (se impostato), per
    # compatibilità trasparente tra MinIO locale e AWS/Cloudflare R2 in
    # produzione. Credenziali esplicite solo se entrambe presenti: altrimenti
    # ci si affida alla default credential chain di boto3 (ruolo AWS —
    # instance profile/IRSA, variabili AWS_* d'ambiente, ecc.).
    kwargs: dict = {"config": Config(signature_version="s3v4")}
    if settings.s3_endpoint_url:
        kwargs["endpoint_url"] = settings.s3_endpoint_url
    if settings.s3_region:
        kwargs["region_name"] = settings.s3_region
    if settings.s3_access_key_id and settings.s3_secret_access_key:
        kwargs["aws_access_key_id"] = settings.s3_access_key_id
        kwargs["aws_secret_access_key"] = settings.s3_secret_access_key
    return boto3.client("s3", **kwargs)


def ensure_public_bucket(bucket: str) -> None:
    """Crea il bucket se manca e vi applica una policy public-read: gli
    avatar sono pensati per essere serviti direttamente via URL, non tramite
    presigned URL con scadenza. No-op sul backend "localstorage" (niente
    bucket/policy su filesystem)."""
    if settings.storage_backend != "s3":
        return
    client = get_s3_client()
    try:
        client.head_bucket(Bucket=bucket)
    except Exception:
        client.create_bucket(Bucket=bucket)

    policy = {
        "Version": "2012-10-17",
        "Statement": [
            {
                "Effect": "Allow",
                "Principal": "*",
                "Action": "s3:GetObject",
                "Resource": f"arn:aws:s3:::{bucket}/*",
            }
        ],
    }
    client.put_bucket_policy(Bucket=bucket, Policy=json.dumps(policy))


def ensure_content_bucket(bucket: str) -> None:
    """Crea il bucket se manca, con lettura pubblica solo sul prefisso
    .../media/... (embed nei post) — .../posts/... (backup markdown) resta
    privato: è un fallback interno, non un asset da servire ai visitatori.
    No-op sul backend "localstorage"."""
    if settings.storage_backend != "s3":
        return
    client = get_s3_client()
    try:
        client.head_bucket(Bucket=bucket)
    except Exception:
        client.create_bucket(Bucket=bucket)

    policy = {
        "Version": "2012-10-17",
        "Statement": [
            {
                "Effect": "Allow",
                "Principal": "*",
                "Action": "s3:GetObject",
                "Resource": f"arn:aws:s3:::{bucket}/*/userdata/*/*/media/*",
            }
        ],
    }
    client.put_bucket_policy(Bucket=bucket, Policy=json.dumps(policy))


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


def avatar_public_url(object_key: str) -> str:
    return _public_url(bucket=settings.s3_bucket_avatars, key=object_key)
