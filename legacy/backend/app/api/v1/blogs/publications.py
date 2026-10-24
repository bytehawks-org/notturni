"""Pubblicazioni (todo/PUBLICATIONS.md, todo/UX_REDESIGN.md B9, mockup
2d/3g): CRUD per blog, indice dei capitoli pubblico, ordine esplicito."""

import re
import uuid
from datetime import datetime, timezone

from fastapi import Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import func, select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.api.v1.blogs._common import _get_blog_or_404, _require_blog_viewable, _require_blog_write_access
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.revalidation import blog_tag, revalidate_frontend
from app.domain.authorization import can_write_posts, is_publicly_visible
from app.domain.permalinks import build_permalink
from app.models.blog import Blog
from app.models.post import Post, PostStatus
from app.models.publication import Publication
from app.models.user import User

NAME_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


class PublicationCreate(BaseModel):
    name: str
    title: str
    description: str | None = None


class PublicationUpdate(BaseModel):
    name: str | None = None
    title: str | None = None
    description: str | None = None


class PublicationOut(BaseModel):
    id: uuid.UUID
    name: str
    title: str
    description: str | None
    chapters_total: int
    chapters_published: int
    created_at: datetime


class ChapterOut(BaseModel):
    n: int
    post_id: uuid.UUID
    slug: str
    title: str
    locale: str
    status: PostStatus
    published_at: datetime | None
    permalink: str
    reading_minutes: int
    is_public: bool


class PublicationDetailOut(PublicationOut):
    chapters: list[ChapterOut]


class OrderRequest(BaseModel):
    post_ids: list[uuid.UUID]


def _validate(name: str | None, title: str | None) -> None:
    if name is not None and (not NAME_RE.fullmatch(name) or len(name) < 2 or len(name) > 60):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il nome può contenere solo lettere minuscole, numeri e trattini (2-60 caratteri).")
    if title is not None and not title.strip():
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il titolo è obbligatorio.")


def _minutes(content: str) -> int:
    return max(1, round(len(content.split()) / 200))


def _chapter_sort_key(post: Post) -> tuple:
    # ordine esplicito prima, poi cronologico dal più vecchio (todo/PUBLICATIONS.md)
    return (post.chapter_order if post.chapter_order is not None else 10**9, post.published_at or post.created_at, post.created_at)


async def _counts(session: AsyncSession, pub_ids: list[uuid.UUID]) -> tuple[dict, dict]:
    if not pub_ids:
        return {}, {}
    total = dict((await session.execute(select(Post.publication_id, func.count()).where(Post.publication_id.in_(pub_ids)).group_by(Post.publication_id))).all())
    published = dict(
        (
            await session.execute(
                select(Post.publication_id, func.count())
                .where(
                    Post.publication_id.in_(pub_ids),
                    Post.status == PostStatus.PUBLISHED,
                    Post.published_at <= datetime.now(timezone.utc),
                    Post.is_hidden.is_(False),
                )
                .group_by(Post.publication_id)
            )
        ).all()
    )
    return total, published


def _out(p: Publication, total: dict, published: dict) -> PublicationOut:
    return PublicationOut(
        id=p.id, name=p.name, title=p.title, description=p.description, chapters_total=int(total.get(p.id, 0)), chapters_published=int(published.get(p.id, 0)), created_at=p.created_at
    )


@router.get("/{slug}/publications", response_model=list[PublicationOut])
async def list_publications(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PublicationOut]:
    """Pubblicazioni del blog con conteggi dei capitoli (totali e pubblicati).
    Segue la visibilità del blog; i lettori vedono solo quelle con almeno un
    capitolo pubblicato, chi scrive tutte."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    pubs = list((await session.execute(select(Publication).where(Publication.blog_id == blog.id).order_by(Publication.created_at))).scalars().all())
    total, published = await _counts(session, [p.id for p in pubs])
    writer = current_user is not None and await can_write_posts(session, user_id=current_user.id, blog=blog)
    return [_out(p, total, published) for p in pubs if writer or published.get(p.id, 0)]


@router.post("/{slug}/publications", response_model=PublicationOut, status_code=status.HTTP_201_CREATED)
async def create_publication(
    slug: str,
    payload: PublicationCreate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PublicationOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    _validate(payload.name, payload.title)
    exists = (await session.execute(select(Publication).where(Publication.blog_id == blog.id, Publication.name == payload.name))).scalar_one_or_none()
    if exists is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una pubblicazione con questo nome.")
    pub = Publication(blog_id=blog.id, name=payload.name, title=payload.title.strip(), description=(payload.description or "").strip() or None)
    session.add(pub)
    await session.commit()
    await session.refresh(pub)
    return _out(pub, {}, {})


async def _get_pub(session: AsyncSession, blog: Blog, ref: str | uuid.UUID) -> Publication:
    stmt = select(Publication).where(Publication.blog_id == blog.id)
    if isinstance(ref, uuid.UUID):
        stmt = stmt.where(Publication.id == ref)
    else:
        try:
            stmt = stmt.where(Publication.id == uuid.UUID(ref))
        except ValueError:
            stmt = stmt.where(Publication.name == ref)
    pub = (await session.execute(stmt)).scalar_one_or_none()
    if pub is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pubblicazione non trovata.")
    return pub


@router.get("/{slug}/publications/{ref}", response_model=PublicationDetailOut)
async def get_publication(
    slug: str,
    ref: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PublicationDetailOut:
    """Indice dei capitoli (`ref` = name o id): per i lettori solo i capitoli
    pubblicati, per chi scrive anche bozze/pianificati (segnalati con
    `is_public=false`)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    pub = await _get_pub(session, blog, ref)
    writer = current_user is not None and await can_write_posts(session, user_id=current_user.id, blog=blog)
    posts = list((await session.execute(select(Post).where(Post.publication_id == pub.id))).scalars().all())
    posts.sort(key=_chapter_sort_key)
    if not writer:
        posts = [p for p in posts if is_publicly_visible(p)]
        if not posts:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "Pubblicazione non trovata.")
    total, published = await _counts(session, [pub.id])
    chapters = [
        ChapterOut(
            n=i + 1, post_id=p.id, slug=p.slug, title=p.title, locale=p.locale, status=p.status, published_at=p.published_at, permalink=build_permalink(blog.slug, p), reading_minutes=_minutes(p.content), is_public=is_publicly_visible(p)
        )
        for i, p in enumerate(posts)
    ]
    return PublicationDetailOut(**_out(pub, total, published).model_dump(), chapters=chapters)


@router.patch("/{slug}/publications/{ref}", response_model=PublicationOut)
async def update_publication(
    slug: str,
    ref: str,
    payload: PublicationUpdate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PublicationOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    pub = await _get_pub(session, blog, ref)
    _validate(payload.name, payload.title)
    if payload.name is not None and payload.name != pub.name:
        exists = (await session.execute(select(Publication).where(Publication.blog_id == blog.id, Publication.name == payload.name))).scalar_one_or_none()
        if exists is not None:
            raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una pubblicazione con questo nome.")
        pub.name = payload.name
    if payload.title is not None:
        pub.title = payload.title.strip()
    if payload.description is not None:
        pub.description = payload.description.strip() or None
    await session.commit()
    await session.refresh(pub)
    await revalidate_frontend([blog_tag(blog.slug)])
    total, published = await _counts(session, [pub.id])
    return _out(pub, total, published)


@router.put("/{slug}/publications/{ref}/order", response_model=PublicationDetailOut)
async def order_publication(
    slug: str,
    ref: str,
    payload: OrderRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PublicationDetailOut:
    """Ordine esplicito dei capitoli (mockup 3g drag-to-order): `post_ids`
    nell'ordine voluto; i post non elencati seguono in coda in ordine
    cronologico."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    pub = await _get_pub(session, blog, ref)
    members = set((await session.execute(select(Post.id).where(Post.publication_id == pub.id))).scalars().all())
    unknown = [str(i) for i in payload.post_ids if i not in members]
    if unknown:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Alcuni post non appartengono a questa pubblicazione.")
    await session.execute(update(Post).where(Post.publication_id == pub.id).values(chapter_order=None))
    for i, post_id in enumerate(dict.fromkeys(payload.post_ids)):
        await session.execute(update(Post).where(Post.id == post_id).values(chapter_order=i + 1))
    await session.commit()
    await revalidate_frontend([blog_tag(blog.slug)])
    return await get_publication(slug, str(pub.id), current_user, session)


@router.delete("/{slug}/publications/{ref}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_publication(
    slug: str,
    ref: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """I post restano, solo senza pubblicazione (FK SET NULL)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    pub = await _get_pub(session, blog, ref)
    await session.delete(pub)
    await session.commit()
    await revalidate_frontend([blog_tag(blog.slug)])
