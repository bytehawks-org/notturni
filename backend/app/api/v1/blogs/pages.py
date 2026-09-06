"""Pagine statiche del blog: feature opt-in (CLAUDE.md #1, todo/BLOG.md).
Blog.static_pages_enabled, disattiva di default — sempre attiva invece per
le pagine di piattaforma (app/api/v1/pages.py). Niente tag/categorie/
pubblicazioni su queste pagine: solo titolo/slug/lingua/contenuto."""

import uuid

from fastapi import Depends, HTTPException, status
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.api.v1.blogs._common import (
    _get_blog_or_404,
    _require_blog_viewable,
    _require_blog_write_access,
)
from app.api.v1.blogs._router import router
from app.api.v1.pages import (
    PageCreateRequest,
    PageOut,
    PageTranslationRequest,
    PageTranslationSummaryOut,
    PageUpdateRequest,
)
from app.core.database import get_session
from app.core.revalidation import blog_page_tag, blog_tag, revalidate_frontend
from app.domain.authorization import can_write_posts
from app.domain.i18n import validate_locale
from app.domain.pages import build_page_permalink, validate_page_slug
from app.models.blog import Blog
from app.models.page import Page
from app.models.user import User


def _require_static_pages_enabled(blog: Blog) -> None:
    if not blog.static_pages_enabled:
        raise HTTPException(
            status.HTTP_403_FORBIDDEN, "Le pagine statiche non sono attive per questo blog."
        )


def _to_page_out(blog: Blog, page: Page) -> PageOut:
    out = PageOut.model_validate(page)
    out.permalink = build_page_permalink(blog.slug, page)
    out.mentions_enabled = blog.mentions_enabled
    return out


async def _get_blog_page_or_404(session: AsyncSession, blog_id: uuid.UUID, page_id: uuid.UUID) -> Page:
    page = await session.get(Page, page_id)
    if page is None or page.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return page


@router.get("/{slug}/pages", response_model=list[PageOut])
async def list_blog_pages(
    slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PageOut]:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    stmt = select(Page).where(Page.blog_id == blog.id, Page.locale == locale)
    can_write = current_user is not None and await can_write_posts(
        session, user_id=current_user.id, blog=blog
    )
    if not can_write:
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    return [_to_page_out(blog, page) for page in result.scalars().all()]


@router.get("/{slug}/pages/by-id/{page_id}", response_model=PageOut)
async def get_blog_page_by_id(
    slug: str,
    page_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Per l'editor di dashboard (bozza inclusa): richiede accesso in
    scrittura al blog, non la sola visibilità pubblica — a differenza di
    ``GET /{slug}/pages/{page_slug}`` sotto, pensato per la risoluzione del
    permalink pubblico. Registrata prima di quella rotta perché "by-id" non
    collida con `{page_slug}`."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)
    return _to_page_out(blog, page)


@router.get("/{slug}/pages/{page_slug}", response_model=PageOut)
async def get_blog_page(
    slug: str,
    page_slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Pubblico: risolve il permalink /{blog_slug}/pagina/{page_slug} (solo
    pagine pubblicate, a meno che il chiamante non abbia accesso in scrittura
    sul blog — vedi app/domain/pages.py)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    stmt = select(Page).where(Page.blog_id == blog.id, Page.slug == page_slug, Page.locale == locale)
    can_write = current_user is not None and await can_write_posts(
        session, user_id=current_user.id, blog=blog
    )
    if not can_write:
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    page = result.scalar_one_or_none()
    if page is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return _to_page_out(blog, page)


@router.post("/{slug}/pages", response_model=PageOut, status_code=status.HTTP_201_CREATED)
async def create_blog_page(
    slug: str,
    payload: PageCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    _require_static_pages_enabled(blog)

    try:
        validate_locale(payload.locale)
        validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(
            Page.blog_id == blog.id, Page.slug == payload.slug, Page.locale == payload.locale
        )
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso per questa lingua.")

    page = Page(
        blog_id=blog.id,
        slug=payload.slug,
        locale=payload.locale,
        title=payload.title,
        content=payload.content,
        is_published=payload.is_published,
        updated_by_id=current_user.id,
    )
    session.add(page)
    await session.commit()
    await session.refresh(page)
    await revalidate_frontend([blog_tag(slug), blog_page_tag(slug, page.slug)])
    return _to_page_out(blog, page)


@router.post(
    "/{slug}/pages/{page_id}/translations", response_model=PageOut, status_code=status.HTTP_201_CREATED
)
async def add_blog_page_translation(
    slug: str,
    page_id: uuid.UUID,
    payload: PageTranslationRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    _require_static_pages_enabled(blog)
    original = await _get_blog_page_or_404(session, blog.id, page_id)

    try:
        validate_locale(payload.locale)
        validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(
            Page.translation_group_id == original.translation_group_id, Page.locale == payload.locale
        )
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Traduzione già presente per questa lingua.")

    translation = Page(
        blog_id=blog.id,
        slug=payload.slug,
        locale=payload.locale,
        translation_group_id=original.translation_group_id,
        title=payload.title,
        content=payload.content,
        is_published=payload.is_published,
        updated_by_id=current_user.id,
    )
    session.add(translation)
    await session.commit()
    await session.refresh(translation)
    await revalidate_frontend([blog_tag(slug), blog_page_tag(slug, translation.slug)])
    return _to_page_out(blog, translation)


@router.get("/{slug}/pages/{page_id}/translations", response_model=list[PageTranslationSummaryOut])
async def list_blog_page_translations(
    slug: str,
    page_id: uuid.UUID,
    session: AsyncSession = Depends(get_session),
) -> list[Page]:
    """Per il selettore di lingua lato frontend, stesso pattern di
    app/api/v1/pages.py::list_page_translations (e di
    app/api/v1/posts.py::list_post_translations) — solo le pubblicate."""
    blog = await _get_blog_or_404(session, slug)
    original = await _get_blog_page_or_404(session, blog.id, page_id)
    result = await session.execute(
        select(Page).where(Page.translation_group_id == original.translation_group_id)
    )
    return [p for p in result.scalars().all() if p.is_published]


@router.patch("/{slug}/pages/{page_id}", response_model=PageOut)
async def update_blog_page(
    slug: str,
    page_id: uuid.UUID,
    payload: PageUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Modifica/rimozione restano possibili anche a feature disattivata
    (`static_pages_enabled=False` blocca solo la creazione di pagine/
    traduzioni nuove, non la gestione di quelle esistenti)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)

    try:
        if payload.slug is not None:
            validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.slug is not None and payload.slug != page.slug:
        existing = await session.execute(
            select(Page).where(
                Page.blog_id == blog.id, Page.slug == payload.slug, Page.locale == page.locale
            )
        )
        if existing.scalar_one_or_none() is not None:
            raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso per questa lingua.")
        page.slug = payload.slug
    if payload.title is not None:
        page.title = payload.title
    if payload.content is not None:
        page.content = payload.content
    if payload.is_published is not None:
        page.is_published = payload.is_published
    page.updated_by_id = current_user.id

    await session.commit()
    await session.refresh(page)
    await revalidate_frontend([blog_tag(slug), blog_page_tag(slug, page.slug)])
    return _to_page_out(blog, page)


@router.delete("/{slug}/pages/{page_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_blog_page(
    slug: str,
    page_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)
    page_slug = page.slug

    await session.delete(page)
    await session.commit()
    await revalidate_frontend([blog_tag(slug), blog_page_tag(slug, page_slug)])
