"""Immagine di copertina e favicon del blog: identità/branding del blog,
distinte dal contenuto dei post (app/api/v1/blogs/media.py) e dall'avatar
personale dell'utente (app/api/v1/users.py). Entrambe facoltative, solo il
proprietario può gestirle."""

from fastapi import Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import BlogOut, _require_blog_owner, _to_blog_out
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.revalidation import blog_tag, revalidate_frontend
from app.core.storage import (
    content_public_url,
    delete_blog_favicon,
    upload_blog_favicon,
    upload_media,
)
from app.domain.content_media import SENSITIVITY_CATEGORIES
from app.domain.moderation import classify_image
from app.domain.platform_config import get_platform_config
from app.models.media_file import MediaFile
from app.models.user import User


class CoverImageCategoriesUpdate(BaseModel):
    categories: list[str]
    # Assente: lascia invariato l'alt text; presente (anche `null`/`""`): lo
    # azzera o sostituisce — stesso schema tri-state di PostUpdateRequest
    # (app/api/v1/posts.py), model_fields_set in update_blog_cover_image_categories.
    alt_text: str | None = None


@router.post("/{slug}/cover-image", response_model=BlogOut, status_code=status.HTTP_201_CREATED)
async def upload_blog_cover_image(
    slug: str,
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Banner della home pubblica del blog (facoltativo). Stessa moderazione
    automatica delle immagini di contenuto (app/domain/moderation.py); come
    per Post.cover_image_url, sostituire l'immagine non cancella l'oggetto
    precedente su storage (stessa scelta, vedi app/api/v1/posts.py)."""
    blog = await _require_blog_owner(session, current_user, slug)

    content = await file.read()
    try:
        object_key = upload_media(
            user_id=blog.owner_id, blog_id=blog.id, content=content, content_type=file.content_type or ""
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    platform = await get_platform_config(session)
    is_sensitive = await classify_image(
        content, file.filename or "image", file.content_type or "", threshold=platform.moderation_threshold
    )
    url = content_public_url(object_key)

    blog.cover_image_url = url
    blog.cover_image_is_sensitive = is_sensitive
    blog.cover_image_categories = []
    blog.cover_image_alt_text = ""
    # B7/libreria media: a differenza di prima, anche la cover del blog
    # finisce nella libreria (come già la cover di un post, caricata sullo
    # stesso endpoint di app/api/v1/blogs/media.py::upload_blog_media) —
    # altrimenti non vi compariva mai, a differenza di ogni altra immagine.
    session.add(
        MediaFile(
            blog_id=blog.id,
            uploader_id=current_user.id,
            object_key=object_key,
            url=url,
            content_type=file.content_type or "",
            size_bytes=len(content),
            alt_text="",
            is_sensitive=is_sensitive,
        )
    )
    await session.commit()
    await session.refresh(blog)
    await revalidate_frontend([blog_tag(slug)])
    return _to_blog_out(blog, current_user)


@router.patch("/{slug}/cover-image", response_model=BlogOut)
async def update_blog_cover_image_categories(
    slug: str,
    payload: CoverImageCategoriesUpdate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Avviso manuale sui contenuti della cover già caricata (stile Bluesky,
    CLAUDE.md #3), senza ricaricare l'immagine — stesso principio di
    PATCH /posts/{id} quando cambia solo `cover_image_categories`."""
    blog = await _require_blog_owner(session, current_user, slug)
    if not blog.cover_image_url:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il blog non ha una cover da annotare.")
    bad = [c for c in payload.categories if c not in SENSITIVITY_CATEGORIES]
    if bad:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Categorie non valide: {', '.join(bad)}.")
    blog.cover_image_categories = list(dict.fromkeys(payload.categories))
    blog.cover_image_is_sensitive = bool(blog.cover_image_categories)
    if "alt_text" in payload.model_fields_set:
        blog.cover_image_alt_text = payload.alt_text or ""
    await session.commit()
    await session.refresh(blog)
    await revalidate_frontend([blog_tag(slug)])
    return _to_blog_out(blog, current_user)


@router.delete("/{slug}/cover-image", response_model=BlogOut)
async def delete_blog_cover_image(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    blog = await _require_blog_owner(session, current_user, slug)
    blog.cover_image_url = None
    blog.cover_image_is_sensitive = False
    blog.cover_image_categories = []
    blog.cover_image_alt_text = ""
    await session.commit()
    await session.refresh(blog)
    await revalidate_frontend([blog_tag(slug)])
    return _to_blog_out(blog, current_user)


@router.post("/{slug}/favicon", response_model=BlogOut, status_code=status.HTTP_201_CREATED)
async def upload_blog_favicon_route(
    slug: str,
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Favicon dedicata del blog (facoltativa): icona di identità, non
    contenuto — nessuna moderazione, stesso principio dell'avatar utente
    (app/api/v1/users.py). Sostituisce e cancella la precedente, se presente."""
    blog = await _require_blog_owner(session, current_user, slug)

    content = await file.read()
    try:
        object_key = upload_blog_favicon(blog_id=blog.id, content=content, content_type=file.content_type or "")
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    old_key = blog.favicon_object_key
    blog.favicon_object_key = object_key
    await session.commit()
    if old_key is not None:
        delete_blog_favicon(old_key)
    await session.refresh(blog)
    await revalidate_frontend([blog_tag(slug)])
    return _to_blog_out(blog, current_user)


@router.delete("/{slug}/favicon", response_model=BlogOut)
async def delete_blog_favicon_route(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    blog = await _require_blog_owner(session, current_user, slug)
    if blog.favicon_object_key is not None:
        delete_blog_favicon(blog.favicon_object_key)
        blog.favicon_object_key = None
        await session.commit()
        await session.refresh(blog)
        await revalidate_frontend([blog_tag(slug)])
    return _to_blog_out(blog, current_user)
