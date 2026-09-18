"""Upload di media per l'editor e autocomplete delle @menzioni."""

import uuid
from datetime import datetime

from fastapi import Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import _get_blog_or_404, _require_blog_write_access
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.storage import avatar_public_url, content_public_url, delete_content_object, upload_media
from app.domain.authorization import get_membership_role
from app.domain.content_media import SENSITIVITY_CATEGORIES
from app.domain.moderation import classify_image
from app.domain.platform_config import get_platform_config
from app.models.blog import Blog, BlogMembership
from app.models.follow import BlogFollow
from app.models.media_file import MediaFile
from app.models.post import Post
from app.models.post_media import post_media
from app.models.user import User


class MediaOut(BaseModel):
    url: str
    # Risultato della moderazione automatica (app/domain/moderation.py):
    # False anche se il servizio è disattivato/irraggiungibile — mai bloccante.
    is_sensitive: bool
    # B7: riga della libreria media creata per questo upload
    media_id: uuid.UUID | None = None


@router.post("/{slug}/media", response_model=MediaOut, status_code=status.HTTP_201_CREATED)
async def upload_blog_media(
    slug: str,
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MediaOut:
    """Immagine da incorporare nel Markdown di un post (CLAUDE.md #4:
    s3://{bucket}/{site_slug}/userdata/{user}/{blog}/media/...). Richiede
    accesso in scrittura al blog (proprietario/autore/co-autore)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)

    content = await file.read()
    try:
        object_key = upload_media(
            user_id=blog.owner_id,
            blog_id=blog.id,
            content=content,
            content_type=file.content_type or "",
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    platform = await get_platform_config(session)
    is_sensitive = await classify_image(
        content, file.filename or "image", file.content_type or "", threshold=platform.moderation_threshold
    )
    url = content_public_url(object_key)
    # B7: la libreria media tiene traccia di ogni upload (alt/didascalia
    # modificabili dopo, spazio occupato, "usata in")
    media = MediaFile(
        blog_id=blog.id,
        uploader_id=current_user.id,
        object_key=object_key,
        url=url,
        content_type=file.content_type or "",
        size_bytes=len(content),
        alt_text="",
        is_sensitive=is_sensitive,
    )
    session.add(media)
    await session.commit()
    await session.refresh(media)
    return MediaOut(url=url, is_sensitive=is_sensitive, media_id=media.id)


# ---------------------------------------------------------------------------
# Libreria media (todo/UX_REDESIGN.md B7, mockup 3c)
# ---------------------------------------------------------------------------


class MediaUsageOut(BaseModel):
    post_id: uuid.UUID
    post_slug: str
    post_title: str
    permalink: str


class MediaFileOut(BaseModel):
    id: uuid.UUID
    url: str
    content_type: str
    size_bytes: int
    alt_text: str
    caption: str | None
    categories: list[str]
    is_sensitive: bool
    uploader_username: str | None
    created_at: datetime
    used_in: list[MediaUsageOut]


class MediaLibraryOut(BaseModel):
    items: list[MediaFileOut]
    total_bytes: int


class MediaFileUpdate(BaseModel):
    alt_text: str | None = None
    caption: str | None = None
    categories: list[str] | None = None


async def _require_blog_member(session: AsyncSession, user: User, blog: Blog) -> None:
    if blog.owner_id == user.id:
        return
    if await get_membership_role(session, user_id=user.id, blog_id=blog.id) is None:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario o collaboratore del blog.")


async def _usages(session: AsyncSession, blog: Blog, urls: list[str]) -> dict[str, list[MediaUsageOut]]:
    if not urls:
        return {}
    rows = (
        await session.execute(
            select(post_media.c.url, Post.id, Post.slug, Post.title)
            .join(Post, Post.id == post_media.c.post_id)
            .where(Post.blog_id == blog.id, post_media.c.url.in_(urls))
        )
    ).all()
    out: dict[str, list[MediaUsageOut]] = {}
    seen: set[tuple[str, uuid.UUID]] = set()
    for url, post_id, slug, title in rows:
        if (url, post_id) in seen:
            continue
        seen.add((url, post_id))
        out.setdefault(url, []).append(MediaUsageOut(post_id=post_id, post_slug=slug, post_title=title, permalink=f"/{blog.slug}/{slug}"))
    return out


async def _media_out(session: AsyncSession, blog: Blog, files: list[MediaFile]) -> list[MediaFileOut]:
    usages = await _usages(session, blog, [f.url for f in files])
    uploader_ids = {f.uploader_id for f in files if f.uploader_id}
    names: dict[uuid.UUID, str] = {}
    if uploader_ids:
        names = dict((await session.execute(select(User.id, User.username).where(User.id.in_(uploader_ids)))).all())
    return [
        MediaFileOut(
            id=f.id,
            url=f.url,
            content_type=f.content_type,
            size_bytes=f.size_bytes,
            alt_text=f.alt_text,
            caption=f.caption,
            categories=list(f.categories or []),
            is_sensitive=f.is_sensitive,
            uploader_username=names.get(f.uploader_id) if f.uploader_id else None,
            created_at=f.created_at,
            used_in=usages.get(f.url, []),
        )
        for f in files
    ]


@router.get("/{slug}/media", response_model=MediaLibraryOut)
async def list_blog_media(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MediaLibraryOut:
    """Libreria media del blog (proprietario e collaboratori): ogni immagine
    caricata con alt/didascalia/avviso, chi l'ha caricata, i post che la
    usano, più il totale dei byte."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_member(session, current_user, blog)
    files = list((await session.execute(select(MediaFile).where(MediaFile.blog_id == blog.id).order_by(MediaFile.created_at.desc()))).scalars().all())
    return MediaLibraryOut(items=await _media_out(session, blog, files), total_bytes=sum(f.size_bytes for f in files))


@router.post("/{slug}/media/sync", response_model=MediaLibraryOut)
async def sync_blog_media(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MediaLibraryOut:
    """Importa nella libreria le immagini citate nei post (`post_media`) che
    non hanno ancora una riga: per i blog con contenuti precedenti alla
    libreria. Dimensione e caricatore restano ignoti."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    known = set((await session.execute(select(MediaFile.url).where(MediaFile.blog_id == blog.id))).scalars().all())
    rows = (
        await session.execute(
            select(post_media.c.url, post_media.c.alt_text, post_media.c.categories, post_media.c.is_sensitive)
            .join(Post, Post.id == post_media.c.post_id)
            .where(Post.blog_id == blog.id)
        )
    ).all()
    # La stessa immagine può ricorrere in più post con `is_sensitive`
    # diverso (nessun ORDER BY qui, ordine non deterministico): raggruppare
    # per URL prima di inserire, così una sola citazione sensibile basta a
    # marcare l'intera riga importata, invece di dipendere da quale post
    # viene incontrato per primo (bug segnalato dalla review Copilot).
    to_import: dict[str, tuple[str, list[str], bool]] = {}
    for url, alt_text, categories, is_sensitive in rows:
        if url in known:
            continue
        existing = to_import.get(url)
        if existing is None:
            to_import[url] = (alt_text or "", list(categories or []), is_sensitive)
        elif is_sensitive and not existing[2]:
            to_import[url] = (existing[0], existing[1], True)
    for url, (alt_text, categories, is_sensitive) in to_import.items():
        session.add(
            MediaFile(blog_id=blog.id, uploader_id=None, object_key=None, url=url, content_type="", size_bytes=0, alt_text=alt_text, categories=categories, is_sensitive=is_sensitive)
        )
    await session.commit()
    return await list_blog_media(slug, current_user, session)


async def _get_media_or_404(session: AsyncSession, blog: Blog, media_id: uuid.UUID) -> MediaFile:
    media = await session.get(MediaFile, media_id)
    if media is None or media.blog_id != blog.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Media non trovato.")
    return media


@router.patch("/{slug}/media/{media_id}", response_model=MediaFileOut)
async def update_blog_media(
    slug: str,
    media_id: uuid.UUID,
    payload: MediaFileUpdate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MediaFileOut:
    """Alt text, didascalia e avviso sui contenuti dalla libreria (mockup
    3c). Nota: i post già scritti portano alt/avviso nel proprio Markdown e
    non vengono riscritti — la libreria è il default per gli usi futuri."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    media = await _get_media_or_404(session, blog, media_id)
    if payload.alt_text is not None:
        media.alt_text = payload.alt_text.strip()[:500]
    if payload.caption is not None:
        media.caption = payload.caption.strip()[:500] or None
    if payload.categories is not None:
        bad = [c for c in payload.categories if c not in SENSITIVITY_CATEGORIES]
        if bad:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Categorie non valide: {', '.join(bad)}.")
        media.categories = list(dict.fromkeys(payload.categories))
    await session.commit()
    await session.refresh(media)
    return (await _media_out(session, blog, [media]))[0]


@router.delete("/{slug}/media/{media_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_blog_media(
    slug: str,
    media_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """Cancella un'immagine non usata da nessun post (`409` altrimenti):
    rimuove la riga e, se caricata tramite la libreria, l'oggetto su storage."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    media = await _get_media_or_404(session, blog, media_id)
    used = (await session.execute(select(func.count()).select_from(post_media).join(Post, Post.id == post_media.c.post_id).where(Post.blog_id == blog.id, post_media.c.url == media.url))).scalar_one()
    if used:
        raise HTTPException(status.HTTP_409_CONFLICT, "L'immagine è ancora usata in un post: rimuovila prima dal contenuto.")
    if media.object_key:
        try:
            delete_content_object(media.object_key)
        except Exception:  # noqa: BLE001 — l'oggetto può essere già sparito; la riga va comunque via
            pass
    await session.delete(media)
    await session.commit()


class MentionableUserOut(BaseModel):
    username: str
    display_name: str | None
    avatar_url: str | None

    model_config = {"from_attributes": True}


@router.get("/{slug}/mentionable-users", response_model=list[MentionableUserOut])
async def list_mentionable_users(
    slug: str,
    q: str = "",
    limit: int = 8,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MentionableUserOut]:
    """Suggerimenti per l'autocomplete delle @menzioni nell'editor
    (todo/EDITOR.md): proprietario, collaboratori e follower del blog il cui
    username o nome pubblico inizia/contiene `q`. Richiede accesso in scrittura
    al blog. Se le menzioni sono disattivate sul blog, ritorna lista vuota."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    if not blog.mentions_enabled:
        return []

    limit = min(max(limit, 1), 25)
    prefix = q.strip().lstrip("@").lower()

    related_ids = select(BlogMembership.user_id).where(BlogMembership.blog_id == blog.id).union(
        select(BlogFollow.follower_id).where(BlogFollow.blog_id == blog.id),
        select(Blog.owner_id).where(Blog.id == blog.id),
    )
    stmt = select(User).where(User.id.in_(related_ids))
    if prefix:
        like = f"%{prefix}%"
        stmt = stmt.where(
            func.lower(User.username).like(f"{prefix}%")
            | func.lower(func.coalesce(User.display_name, "")).like(like)
        )
    stmt = stmt.order_by(User.username).limit(limit)
    result = await session.execute(stmt)
    return [
        MentionableUserOut(
            username=u.username,
            display_name=u.display_name,
            avatar_url=avatar_public_url(u.avatar_object_key) if u.avatar_object_key else None,
        )
        for u in result.scalars().all()
    ]
