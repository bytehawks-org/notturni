"""Ciclo di vita del blog (todo/UX_REDESIGN.md B3, mockup 5c "danger zone"):
export completo, cancellazione con tolleranza e purge definitivo.

La cancellazione è in due tempi: `DELETE /blogs/{slug}` imposta
`Blog.deleted_at` (il blog sparisce dalle pagine pubbliche, il proprietario
può ripristinarlo); `purge_deleted_blogs` — eseguito dal worker di
manutenzione — elimina davvero blog e contenuti dopo BLOG_DELETE_GRACE_DAYS."""

import asyncio
import io
import json
import logging
import uuid
import zipfile
from datetime import datetime, timedelta, timezone
from typing import Any

from sqlalchemy import delete, select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.storage import delete_blog_storage
from app.models.blog import Blog, BlogInvitation, BlogMembership
from app.models.blog_config import BlogConfig
from app.models.category import Category
from app.models.comment import Comment
from app.models.follow import BlogFollow
from app.models.page import Page
from app.models.post import Post
from app.models.post_fragment import PostFragment
from app.models.post_link import post_links
from app.models.post_media import post_media
from app.models.post_note import post_notes

logger = logging.getLogger(__name__)

BLOG_DELETE_GRACE_DAYS = 30


def _iso(value: datetime | None) -> str | None:
    return value.isoformat() if value else None


def _front_matter(fields: dict[str, Any]) -> str:
    lines = ["---"]
    for key, value in fields.items():
        if value is None:
            continue
        if isinstance(value, list):
            lines.append(f"{key}: {json.dumps(value, ensure_ascii=False)}")
        else:
            lines.append(f"{key}: {json.dumps(value, ensure_ascii=False) if isinstance(value, str) and ':' in value else value}")
    lines.append("---")
    return "\n".join(lines)


async def build_blog_export_zip(session: AsyncSession, blog: Blog) -> bytes:
    """ZIP con tutto il contenuto del blog in formati aperti: un file Markdown
    per post e per pagina (front matter con metadati, note in coda), più
    `blog.json`, `comments.json`, `categories.json`, `media.json` (URL delle
    immagini citate — i binari restano sullo storage) e `links.json`."""
    posts = (await session.execute(select(Post).where(Post.blog_id == blog.id).order_by(Post.created_at))).scalars().all()
    post_ids = [p.id for p in posts]
    categories = (await session.execute(select(Category).where(Category.blog_id == blog.id))).scalars().all()
    category_name = {c.id: c.name for c in categories}
    pages = (await session.execute(select(Page).where(Page.blog_id == blog.id).order_by(Page.created_at))).scalars().all()

    notes_by_post: dict[uuid.UUID, list[tuple[int, str]]] = {}
    media_rows: list[dict[str, Any]] = []
    link_rows: list[dict[str, Any]] = []
    comments: list[Comment] = []
    if post_ids:
        for post_id, idx, content in (
            await session.execute(select(post_notes.c.post_id, post_notes.c.idx, post_notes.c.content).where(post_notes.c.post_id.in_(post_ids)))
        ).all():
            notes_by_post.setdefault(post_id, []).append((idx, content))
        media_rows = [
            {"post_id": str(r.post_id), "position": r.position, "url": r.url, "alt_text": r.alt_text, "categories": list(r.categories)}
            for r in (await session.execute(select(post_media).where(post_media.c.post_id.in_(post_ids)))).all()
        ]
        link_rows = [
            {"post_id": str(r.post_id), "position": r.position, "url": r.url, "link_text": r.link_text}
            for r in (await session.execute(select(post_links).where(post_links.c.post_id.in_(post_ids)))).all()
        ]
        comments = (
            await session.execute(select(Comment).where(Comment.post_id.in_(post_ids)).order_by(Comment.created_at))
        ).scalars().all()

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(
            "blog.json",
            json.dumps(
                {
                    "slug": blog.slug,
                    "title": blog.title,
                    "subtitle": blog.subtitle,
                    "description": blog.description,
                    "visibility": blog.visibility.value,
                    "default_locale": blog.default_locale,
                    "extra_locales": list(blog.extra_locales or []),
                    "default_author_display_name": blog.default_author_display_name,
                    "created_at": _iso(blog.created_at),
                    "exported_at": _iso(datetime.now(timezone.utc)),
                },
                ensure_ascii=False,
                indent=2,
            ),
        )
        for post in posts:
            notes = sorted(notes_by_post.get(post.id, []))
            body = _front_matter(
                {
                    "title": post.title,
                    "slug": post.slug,
                    "locale": post.locale,
                    "status": post.status.value,
                    "published_at": _iso(post.published_at),
                    "author": post.author_display_name,
                    "category": category_name.get(post.category_id) if post.category_id else None,
                    "tags": list(post.manual_tags or []),
                    "cover_image_url": post.cover_image_url,
                    "translation_group_id": str(post.translation_group_id),
                }
            )
            body += "\n\n" + post.content.rstrip() + "\n"
            if notes:
                body += "\n## Note\n\n" + "\n".join(f"[^{idx}]: {content}" for idx, content in notes) + "\n"
            zf.writestr(f"posts/{post.locale}-{post.slug}.md", body)
        for page in pages:
            body = _front_matter({"title": page.title, "slug": page.slug, "locale": page.locale, "is_published": page.is_published})
            zf.writestr(f"pages/{page.locale}-{page.slug}.md", body + "\n\n" + page.content.rstrip() + "\n")
        zf.writestr(
            "comments.json",
            json.dumps(
                [
                    {
                        "id": str(c.id),
                        "post_id": str(c.post_id),
                        "parent_id": str(c.parent_id) if c.parent_id else None,
                        "author": c.author_display_name,
                        "status": c.status.value,
                        "content": c.content,
                        "created_at": _iso(c.created_at),
                    }
                    for c in comments
                ],
                ensure_ascii=False,
                indent=2,
            ),
        )
        zf.writestr("categories.json", json.dumps([{"name": c.name, "slug": c.slug} for c in categories], ensure_ascii=False, indent=2))
        zf.writestr("media.json", json.dumps(media_rows, ensure_ascii=False, indent=2))
        zf.writestr("links.json", json.dumps(link_rows, ensure_ascii=False, indent=2))
    return buffer.getvalue()


async def hard_delete_blog(session: AsyncSession, blog: Blog) -> None:
    """Elimina definitivamente blog e contenuti, nell'ordine imposto dalle
    chiavi esterne senza ON DELETE CASCADE (frammenti e commenti dei post,
    post prima delle categorie, ...). Gli oggetti su storage vengono rimossi
    per ultimi, fuori dalla transazione, best-effort."""
    post_ids = select(Post.id).where(Post.blog_id == blog.id)
    member_ids = (await session.execute(select(BlogMembership.user_id).where(BlogMembership.blog_id == blog.id))).scalars().all()
    uploader_ids = [str(blog.owner_id)] + [str(uid) for uid in member_ids if uid != blog.owner_id]

    await session.execute(delete(PostFragment).where(PostFragment.post_id.in_(post_ids)))
    await session.execute(delete(Comment).where(Comment.post_id.in_(post_ids)))
    await session.execute(update(Post).where(Post.blog_id == blog.id).values(category_id=None))
    await session.execute(delete(Post).where(Post.blog_id == blog.id))
    await session.execute(delete(Category).where(Category.blog_id == blog.id))
    await session.execute(delete(Page).where(Page.blog_id == blog.id))
    await session.execute(delete(BlogInvitation).where(BlogInvitation.blog_id == blog.id))
    await session.execute(delete(BlogMembership).where(BlogMembership.blog_id == blog.id))
    await session.execute(delete(BlogFollow).where(BlogFollow.blog_id == blog.id))
    await session.execute(delete(BlogConfig).where(BlogConfig.blog_id == blog.id))
    blog_id = str(blog.id)
    await session.execute(delete(Blog).where(Blog.id == blog.id))
    await session.commit()
    try:
        removed = await asyncio.to_thread(delete_blog_storage, user_ids=uploader_ids, blog_id=blog_id)
        logger.info("Blog %s eliminato definitivamente, %d oggetti rimossi dallo storage", blog_id, removed)
    except Exception:  # noqa: BLE001
        logger.warning("Blog %s eliminato dal database ma storage non ripulito", blog_id, exc_info=True)


async def purge_deleted_blogs(session: AsyncSession, *, grace_days: int = BLOG_DELETE_GRACE_DAYS) -> int:
    """Elimina i blog con `deleted_at` più vecchio di `grace_days`. Idempotente,
    pensato per il worker di manutenzione (un giro al giorno)."""
    cutoff = datetime.now(timezone.utc) - timedelta(days=grace_days)
    blogs = (await session.execute(select(Blog).where(Blog.deleted_at.is_not(None), Blog.deleted_at <= cutoff))).scalars().all()
    for blog in blogs:
        await hard_delete_blog(session, blog)
    return len(blogs)
