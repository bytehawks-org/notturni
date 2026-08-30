import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import PLATFORM_ADMIN_ROLES, get_optional_current_user, require_platform_admin
from app.core.database import get_session
from app.domain.i18n import validate_locale
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
    slug: str
    locale: str
    translation_group_id: uuid.UUID
    title: str
    content: str
    is_published: bool
    created_at: datetime

    model_config = {"from_attributes": True}


@router.post("", response_model=PageOut, status_code=status.HTTP_201_CREATED)
async def create_page(
    payload: PageCreateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> Page:
    try:
        validate_locale(payload.locale)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(Page.slug == payload.slug, Page.locale == payload.locale)
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
    return page


@router.post(
    "/{page_id}/translations", response_model=PageOut, status_code=status.HTTP_201_CREATED
)
async def add_page_translation(
    page_id: uuid.UUID,
    payload: PageTranslationRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> Page:
    original = await session.get(Page, page_id)
    if original is None:
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
    return translation


def _is_admin(user: User | None) -> bool:
    return user is not None and user.platform_role in PLATFORM_ADMIN_ROLES


@router.get("/{slug}", response_model=PageOut)
async def get_page(
    slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> Page:
    """Pubblico: solo pagine pubblicate. Un amministratore vede anche le bozze
    (per poterle rivedere/modificare prima della pubblicazione)."""
    stmt = select(Page).where(Page.slug == slug, Page.locale == locale)
    if not _is_admin(current_user):
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    page = result.scalar_one_or_none()
    if page is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return page


@router.get("", response_model=list[PageOut])
async def list_pages(
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Page]:
    stmt = select(Page).where(Page.locale == locale)
    if not _is_admin(current_user):
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    return list(result.scalars().all())


@router.patch("/{page_id}", response_model=PageOut)
async def update_page(
    page_id: uuid.UUID,
    payload: PageUpdateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> Page:
    page = await session.get(Page, page_id)
    if page is None:
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
    return page
