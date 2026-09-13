"""Panoramica del blog per la dashboard (todo/UX_REDESIGN.md B1, mockup 5a):
conteggi aggregati, nessuna tabella nuova. Riservata a proprietario e membri."""

from datetime import datetime, timezone

from fastapi import Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import _get_blog_or_404
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.authorization import get_membership_role
from app.models.blog import BlogMembership
from app.models.comment import Comment, CommentStatus
from app.models.follow import BlogFollow
from app.models.post import Post, PostStatus
from app.models.post_media import post_media
from app.models.user import User


class BlogOverviewOut(BaseModel):
    posts_total: int
    posts_published: int
    posts_scheduled: int
    posts_draft: int
    posts_in_review: int
    followers: int
    members: int
    pending_comments: int
    approved_comments: int
    media: int
    last_published_at: datetime | None


async def _count(session: AsyncSession, stmt) -> int:
    return int((await session.execute(stmt)).scalar_one() or 0)


@router.get("/{slug}/overview", response_model=BlogOverviewOut)
async def blog_overview(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOverviewOut:
    """Conteggi per la tab Panoramica del blog. Solo proprietario e membri
    (qualunque ruolo): sono numeri di gestione, non pubblici — i follower
    pubblici restano su `GET /blogs/{slug}/followers`."""
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != current_user.id:
        role = await get_membership_role(session, user_id=current_user.id, blog_id=blog.id)
        if role is None:
            raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario o membro del blog.")

    now = datetime.now(timezone.utc)
    blog_posts = select(Post.id).where(Post.blog_id == blog.id)
    posts_by = lambda *conds: select(func.count()).select_from(Post).where(Post.blog_id == blog.id, *conds)  # noqa: E731

    last_published = (
        await session.execute(
            select(func.max(Post.published_at)).where(
                Post.blog_id == blog.id, Post.status == PostStatus.PUBLISHED, Post.published_at <= now
            )
        )
    ).scalar_one()

    return BlogOverviewOut(
        posts_total=await _count(session, posts_by()),
        posts_published=await _count(session, posts_by(Post.status == PostStatus.PUBLISHED, Post.published_at <= now)),
        posts_scheduled=await _count(session, posts_by(Post.status == PostStatus.PUBLISHED, Post.published_at > now)),
        posts_draft=await _count(session, posts_by(Post.status == PostStatus.DRAFT)),
        posts_in_review=await _count(session, posts_by(Post.status == PostStatus.PENDING_REVIEW)),
        followers=await _count(session, select(func.count()).select_from(BlogFollow).where(BlogFollow.blog_id == blog.id)),
        members=await _count(
            session, select(func.count()).select_from(BlogMembership).where(BlogMembership.blog_id == blog.id)
        ),
        pending_comments=await _count(
            session,
            select(func.count())
            .select_from(Comment)
            .where(Comment.post_id.in_(blog_posts), Comment.status == CommentStatus.PENDING),
        ),
        approved_comments=await _count(
            session,
            select(func.count())
            .select_from(Comment)
            .where(Comment.post_id.in_(blog_posts), Comment.status == CommentStatus.APPROVED),
        ),
        media=await _count(session, select(func.count()).select_from(post_media).where(post_media.c.post_id.in_(blog_posts))),
        last_published_at=last_published,
    )

