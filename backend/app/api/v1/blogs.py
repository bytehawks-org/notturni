import uuid
from datetime import datetime
from typing import Any

from fastapi import APIRouter, Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import get_current_user
from app.core.database import get_session
from app.core.storage import content_public_url, upload_media
from app.domain.authorization import can_write_posts
from app.domain.blog_config import DEFAULT_BLOG_CONFIG, validate_blog_config
from app.domain.blog_rules import assert_can_create_blog, validate_blog_slug
from app.domain.categories import validate_category_name, validate_category_slug
from app.domain.i18n import DEFAULT_LOCALE, validate_locale
from app.domain.moderation import classify_image
from app.models.blog import Blog
from app.models.blog_config import BlogConfig
from app.models.category import Category
from app.models.follow import BlogFollow
from app.models.user import User

router = APIRouter()


class BlogCreateRequest(BaseModel):
    slug: str
    title: str
    default_locale: str = DEFAULT_LOCALE


class BlogUpdateRequest(BaseModel):
    title: str | None = None
    allow_anonymous_comments: bool | None = None
    # "" per tornare al default (username di chi scrive), qualsiasi altro
    # valore lo imposta; assente lascia invariato — stesso schema di
    # Post.cover_image_url in PATCH /posts/{id}.
    default_author_display_name: str | None = None


class BlogOut(BaseModel):
    id: uuid.UUID
    slug: str
    title: str
    custom_domain: str | None
    allow_anonymous_comments: bool
    default_locale: str
    default_author_display_name: str | None
    owner_id: uuid.UUID
    created_at: datetime

    model_config = {"from_attributes": True}


class FollowerOut(BaseModel):
    username: str

    model_config = {"from_attributes": True}


async def _get_blog_or_404(session: AsyncSession, slug: str) -> Blog:
    result = await session.execute(select(Blog).where(Blog.slug == slug))
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog


async def _require_blog_write_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_write_posts(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


@router.post("", response_model=BlogOut, status_code=status.HTTP_201_CREATED)
async def create_blog(
    payload: BlogCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    try:
        validate_blog_slug(payload.slug)
        validate_locale(payload.default_locale)
        await assert_can_create_blog(session, current_user.id)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(select(Blog).where(Blog.slug == payload.slug))
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso.")

    blog = Blog(
        slug=payload.slug,
        title=payload.title,
        default_locale=payload.default_locale,
        owner_id=current_user.id,
    )
    session.add(blog)
    await session.commit()
    await session.refresh(blog)
    return blog


@router.get("/mine", response_model=list[BlogOut])
async def list_my_blogs(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Blog]:
    """Solo i blog di proprietà: l'elenco che include anche le membership su
    blog altrui è rimandato a un secondo momento."""
    result = await session.execute(select(Blog).where(Blog.owner_id == current_user.id))
    return list(result.scalars().all())


@router.get("/{slug}", response_model=BlogOut)
async def get_blog(slug: str, session: AsyncSession = Depends(get_session)) -> Blog:
    return await _get_blog_or_404(session, slug)


@router.patch("/{slug}", response_model=BlogOut)
async def update_blog(
    slug: str,
    payload: BlogUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare il blog.")

    if payload.title is not None:
        blog.title = payload.title
    if payload.allow_anonymous_comments is not None:
        blog.allow_anonymous_comments = payload.allow_anonymous_comments
    if payload.default_author_display_name is not None:
        blog.default_author_display_name = payload.default_author_display_name or None

    await session.commit()
    await session.refresh(blog)
    return blog


@router.post("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def follow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    if existing.scalar_one_or_none() is None:
        session.add(BlogFollow(follower_id=current_user.id, blog_id=blog.id))
        await session.commit()


@router.delete("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def unfollow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    follow = existing.scalar_one_or_none()
    if follow is not None:
        await session.delete(follow)
        await session.commit()


@router.get("/{slug}/followers", response_model=list[FollowerOut])
async def list_blog_followers(slug: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    blog = await _get_blog_or_404(session, slug)
    result = await session.execute(
        select(User).join(BlogFollow, BlogFollow.follower_id == User.id).where(BlogFollow.blog_id == blog.id)
    )
    return list(result.scalars().all())


@router.get("/{slug}/config")
async def get_blog_config(slug: str, session: AsyncSession = Depends(get_session)) -> dict[str, Any]:
    """Configurazione di presentazione (palette/tipografia/layout). Pubblica:
    serve a renderizzare la pagina pubblica del blog. Se il proprietario non
    ha ancora salvato nulla, ritorna il default della piattaforma."""
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog.config.config if blog.config is not None else DEFAULT_BLOG_CONFIG


@router.put("/{slug}/config")
async def update_blog_config(
    slug: str,
    payload: dict[str, Any],
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict[str, Any]:
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare la configurazione.")

    try:
        validate_blog_config(payload)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if blog.config is None:
        blog.config = BlogConfig(blog_id=blog.id, config=payload)
        session.add(blog.config)
    else:
        blog.config.config = payload

    await session.commit()
    return payload


class MediaOut(BaseModel):
    url: str
    # Risultato della moderazione automatica (app/domain/moderation.py):
    # False anche se il servizio è disattivato/irraggiungibile — mai bloccante.
    is_sensitive: bool


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

    is_sensitive = await classify_image(content, file.filename or "image", file.content_type or "")
    return MediaOut(url=content_public_url(object_key), is_sensitive=is_sensitive)


class CategoryCreateRequest(BaseModel):
    name: str
    slug: str


class CategoryUpdateRequest(BaseModel):
    name: str | None = None
    slug: str | None = None


class CategoryOut(BaseModel):
    id: uuid.UUID
    name: str
    slug: str

    model_config = {"from_attributes": True}


@router.get("/{slug}/categories", response_model=list[CategoryOut])
async def list_categories(slug: str, session: AsyncSession = Depends(get_session)) -> list[Category]:
    """Pubblico: la tassonomia di un blog è visibile a chiunque, serve a
    orientarsi tra i contenuti (CLAUDE.md)."""
    blog = await _get_blog_or_404(session, slug)
    result = await session.execute(
        select(Category).where(Category.blog_id == blog.id).order_by(Category.name)
    )
    return list(result.scalars().all())


@router.post("/{slug}/categories", response_model=CategoryOut, status_code=status.HTTP_201_CREATED)
async def create_category(
    slug: str,
    payload: CategoryCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Category:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)

    try:
        validate_category_name(payload.name)
        validate_category_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Category).where(Category.blog_id == blog.id, Category.slug == payload.slug)
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una categoria con questo slug.")

    category = Category(blog_id=blog.id, name=payload.name, slug=payload.slug)
    session.add(category)
    await session.commit()
    await session.refresh(category)
    return category


async def _get_category_or_404(session: AsyncSession, blog_id: uuid.UUID, category_id: uuid.UUID) -> Category:
    category = await session.get(Category, category_id)
    if category is None or category.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Categoria non trovata.")
    return category


@router.patch("/{slug}/categories/{category_id}", response_model=CategoryOut)
async def update_category(
    slug: str,
    category_id: uuid.UUID,
    payload: CategoryUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Category:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    category = await _get_category_or_404(session, blog.id, category_id)

    try:
        if payload.name is not None:
            validate_category_name(payload.name)
        if payload.slug is not None:
            validate_category_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.slug is not None and payload.slug != category.slug:
        existing = await session.execute(
            select(Category).where(Category.blog_id == blog.id, Category.slug == payload.slug)
        )
        if existing.scalar_one_or_none() is not None:
            raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una categoria con questo slug.")
        category.slug = payload.slug

    if payload.name is not None:
        category.name = payload.name

    await session.commit()
    await session.refresh(category)
    return category


@router.delete("/{slug}/categories/{category_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_category(
    slug: str,
    category_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """I post con questa categoria non vengono cancellati, restano solo
    senza categoria (FK ondelete SET NULL, vedi app/models/post.py)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    category = await _get_category_or_404(session, blog.id, category_id)

    await session.delete(category)
    await session.commit()
