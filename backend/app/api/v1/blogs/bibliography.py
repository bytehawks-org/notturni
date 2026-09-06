"""Bibliografie automatiche del blog: note a piè di pagina, media e link
citati nel corpo dei post pubblicati (CLAUDE.md #4, todo/EDITOR.md)."""

from datetime import datetime, timezone

from fastapi import Depends
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_optional_current_user
from app.api.v1.blogs._common import _get_blog_or_404, _require_blog_viewable
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.permalinks import build_permalink
from app.models.post import Post, PostStatus
from app.models.post_link import post_links
from app.models.post_media import post_media
from app.models.post_note import post_notes
from app.models.user import User


class BibliographyCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    idx: int


class BibliographyEntryOut(BaseModel):
    content: str
    citations: list[BibliographyCitationOut]


@router.get("/{slug}/bibliography", response_model=list[BibliographyEntryOut])
async def get_blog_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BibliographyEntryOut]:
    """todo/EDITOR.md: bibliografia automatica del blog — tutte le note a piè
    di pagina dei post pubblicati, raggruppate per testo identico e con
    l'elenco dei post che le citano. Segue la visibilità del blog."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_notes.c.idx, post_notes.c.content)
        .join(post_notes, post_notes.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
            Post.is_hidden.is_(False),
        )
        .order_by(Post.published_at.desc(), post_notes.c.idx.asc())
    )

    entries: dict[str, BibliographyEntryOut] = {}
    for post, idx, content in rows.all():
        key = " ".join(content.split()).casefold()
        entry = entries.get(key)
        if entry is None:
            entry = BibliographyEntryOut(content=content, citations=[])
            entries[key] = entry
        entry.citations.append(
            BibliographyCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                idx=idx,
            )
        )
    return list(entries.values())


class ContentCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    used_at: datetime | None


class MediaBibliographyEntryOut(BaseModel):
    url: str
    alt_text: str
    categories: list[str]
    citations: list[ContentCitationOut]


@router.get("/{slug}/media-bibliography", response_model=list[MediaBibliographyEntryOut])
async def get_blog_media_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MediaBibliographyEntryOut]:
    """CLAUDE.md #4: come la bibliografia delle note (sopra), ma per i media
    (oggi solo immagini) citati nel corpo dei post pubblicati — raggruppati
    per URL identico, con l'elenco dei post che li usano e la data di
    pubblicazione di ciascuno. Segue la visibilità del blog."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_media.c.alt_text, post_media.c.categories, post_media.c.url)
        .join(post_media, post_media.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
            Post.is_hidden.is_(False),
        )
        .order_by(Post.published_at.desc(), post_media.c.position.asc())
    )

    entries: dict[str, MediaBibliographyEntryOut] = {}
    for post, alt_text, categories, url in rows.all():
        entry = entries.get(url)
        if entry is None:
            entry = MediaBibliographyEntryOut(url=url, alt_text=alt_text, categories=categories, citations=[])
            entries[url] = entry
        entry.citations.append(
            ContentCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                used_at=post.published_at,
            )
        )
    return list(entries.values())


class LinkBibliographyEntryOut(BaseModel):
    url: str
    link_text: str
    citations: list[ContentCitationOut]


@router.get("/{slug}/links-bibliography", response_model=list[LinkBibliographyEntryOut])
async def get_blog_links_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[LinkBibliographyEntryOut]:
    """CLAUDE.md #4: come la bibliografia dei media sopra, ma per i link
    citati nel corpo dei post pubblicati."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_links.c.link_text, post_links.c.url)
        .join(post_links, post_links.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
            Post.is_hidden.is_(False),
        )
        .order_by(Post.published_at.desc(), post_links.c.position.asc())
    )

    entries: dict[str, LinkBibliographyEntryOut] = {}
    for post, link_text, url in rows.all():
        entry = entries.get(url)
        if entry is None:
            entry = LinkBibliographyEntryOut(url=url, link_text=link_text, citations=[])
            entries[url] = entry
        entry.citations.append(
            ContentCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                used_at=post.published_at,
            )
        )
    return list(entries.values())
