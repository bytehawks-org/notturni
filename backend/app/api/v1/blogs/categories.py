"""Tassonomia per-blog: CRUD delle categorie dei post."""

import uuid

from fastapi import Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.api.v1.blogs._common import (
    _get_blog_or_404,
    _require_blog_viewable,
    _require_blog_write_access,
)
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.categories import validate_category_name, validate_category_slug
from app.models.category import Category
from app.models.user import User


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


async def _get_category_or_404(session: AsyncSession, blog_id: uuid.UUID, category_id: uuid.UUID) -> Category:
    category = await session.get(Category, category_id)
    if category is None or category.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Categoria non trovata.")
    return category


@router.get("/{slug}/categories", response_model=list[CategoryOut])
async def list_categories(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Category]:
    """La tassonomia di un blog serve a orientarsi tra i contenuti (CLAUDE.md);
    segue la visibilità del blog (todo/BLOG.md #2)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
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
