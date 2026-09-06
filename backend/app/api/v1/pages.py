import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import or_, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import PLATFORM_ADMIN_ROLES, get_optional_current_user, require_platform_admin
from app.core.database import get_session
from app.core.revalidation import platform_page_tag, platform_pages_tag, revalidate_frontend
from app.domain.i18n import validate_locale
from app.domain.pages import build_platform_page_permalink
from app.models.page import Page
from app.models.user import User

router = APIRouter()


class PageCreateRequest(BaseModel):
    slug: str
    locale: str
    title: str
    content: str
    is_published: bool = False


class PageTranslationRequest(BaseModel):
    slug: str
    locale: str
    title: str
    content: str
    is_published: bool = False


class PageUpdateRequest(BaseModel):
    slug: str | None = None
    title: str | None = None
    content: str | None = None
    is_published: bool | None = None


class PageOut(BaseModel):
    id: uuid.UUID
    blog_id: uuid.UUID | None = None
    slug: str
    locale: str
    translation_group_id: uuid.UUID
    title: str
    content: str
    is_published: bool
    created_at: datetime
    permalink: str | None = None
    # Per il rendering pubblico lato frontend, senza una fetch separata del
    # blog: sempre True per le pagine di piattaforma (niente blog da cui
    # ereditarlo), mirror di Blog.mentions_enabled per le pagine di blog —
    # vedi app/api/v1/blogs.py::_to_page_out (stesso pattern di Post.mentions_enabled).
    mentions_enabled: bool = True

    model_config = {"from_attributes": True}


def _to_out(page: Page) -> PageOut:
    out = PageOut.model_validate(page)
    out.permalink = build_platform_page_permalink(page)
    return out


@router.post("", response_model=PageOut, status_code=status.HTTP_201_CREATED)
async def create_page(
    payload: PageCreateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    try:
        validate_locale(payload.locale)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(
            Page.blog_id.is_(None), Page.slug == payload.slug, Page.locale == payload.locale
        )
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso per questa lingua.")

    page = Page(
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
    await revalidate_frontend([platform_pages_tag(), platform_page_tag(page.slug)])
    return _to_out(page)


@router.post(
    "/{page_id}/translations", response_model=PageOut, status_code=status.HTTP_201_CREATED
)
async def add_page_translation(
    page_id: uuid.UUID,
    payload: PageTranslationRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    original = await session.get(Page, page_id)
    if original is None or original.blog_id is not None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")

    try:
        validate_locale(payload.locale)
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
    await revalidate_frontend(
        [platform_pages_tag(), platform_page_tag(translation.slug)]
    )
    return _to_out(translation)


class PageTranslationSummaryOut(BaseModel):
    id: uuid.UUID
    locale: str
    slug: str
    is_published: bool

    model_config = {"from_attributes": True}


@router.get("/{page_id}/translations", response_model=list[PageTranslationSummaryOut])
async def list_page_translations(
    page_id: uuid.UUID, session: AsyncSession = Depends(get_session)
) -> list[Page]:
    """Per il selettore di lingua lato frontend, stesso pattern di
    app/api/v1/posts.py::list_post_translations — solo le pubblicate, il
    chiamante mostra sempre la pagina corrente a parte."""
    original = await session.get(Page, page_id)
    if original is None or original.blog_id is not None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    result = await session.execute(
        select(Page).where(Page.translation_group_id == original.translation_group_id)
    )
    return [p for p in result.scalars().all() if p.is_published]


def _is_admin(user: User | None) -> bool:
    return user is not None and user.platform_role in PLATFORM_ADMIN_ROLES


@router.get("/{slug}", response_model=PageOut)
async def get_page(
    slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Pubblico: solo pagine pubblicate. Un amministratore vede anche le bozze
    (per poterle rivedere/modificare prima della pubblicazione)."""
    stmt = select(Page).where(Page.blog_id.is_(None), Page.slug == slug, Page.locale == locale)
    if not _is_admin(current_user):
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    page = result.scalar_one_or_none()
    if page is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return _to_out(page)


@router.get("", response_model=list[PageOut])
async def list_pages(
    locale: str,
    q: str | None = None,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PageOut]:
    stmt = select(Page).where(Page.blog_id.is_(None), Page.locale == locale)
    if not _is_admin(current_user):
        stmt = stmt.where(Page.is_published.is_(True))
    if q:
        needle = f"%{q}%"
        stmt = stmt.where(or_(Page.title.ilike(needle), Page.slug.ilike(needle)))
    result = await session.execute(stmt)
    return [_to_out(page) for page in result.scalars().all()]


@router.patch("/{page_id}", response_model=PageOut)
async def update_page(
    page_id: uuid.UUID,
    payload: PageUpdateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    page = await session.get(Page, page_id)
    if page is None or page.blog_id is not None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")

    if payload.slug is not None:
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
    await revalidate_frontend([platform_pages_tag(), platform_page_tag(page.slug)])
    return _to_out(page)
