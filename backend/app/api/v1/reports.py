"""Segnalazioni di blog e post dai lettori (todo/UX_REDESIGN.md B5, mockup
5e "reports"): solo utenti registrati, una per bersaglio, rate limit per
utente. Vengono lette e chiuse dagli admin in `app/api/v1/admin.py`."""

import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.core.database import get_session
from app.domain.authorization import is_blog_publicly_readable, is_publicly_visible
from app.domain.rate_limit import enforce_rate_limit
from app.models.blog import Blog
from app.models.content_report import ContentReport, ReportReason, ReportStatus, ReportTargetType
from app.models.post import Post
from app.models.user import User

router = APIRouter()

REPORTS_PER_HOUR = 10


class ReportRequest(BaseModel):
    reason: ReportReason
    note: str | None = None


class ReportOut(BaseModel):
    id: uuid.UUID
    target_type: ReportTargetType
    target_id: uuid.UUID
    reason: ReportReason
    note: str | None
    status: ReportStatus
    created_at: datetime

    model_config = {"from_attributes": True}


async def _create_report(
    session: AsyncSession, *, user: User, target_type: ReportTargetType, target_id: uuid.UUID, blog: Blog, payload: ReportRequest
) -> ContentReport:
    await enforce_rate_limit(
        f"report:{user.id}", limit=REPORTS_PER_HOUR, window_seconds=3600, message="Troppe segnalazioni in poco tempo, riprova più tardi."
    )
    existing = (
        await session.execute(
            select(ContentReport).where(
                ContentReport.reporter_id == user.id,
                ContentReport.target_type == target_type,
                ContentReport.target_id == target_id,
            )
        )
    ).scalar_one_or_none()
    if existing is not None:
        # idempotente: una sola segnalazione per lettore e bersaglio
        return existing
    report = ContentReport(
        reporter_id=user.id,
        target_type=target_type,
        target_id=target_id,
        blog_id=blog.id,
        reason=payload.reason,
        note=(payload.note or "").strip()[:500] or None,
        status=ReportStatus.OPEN,
    )
    session.add(report)
    await session.commit()
    await session.refresh(report)
    return report


@router.post("/blogs/{slug}/report", response_model=ReportOut, status_code=status.HTTP_201_CREATED)
async def report_blog(
    slug: str,
    payload: ReportRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> ContentReport:
    blog = (await session.execute(select(Blog).where(Blog.slug == slug))).scalar_one_or_none()
    if blog is None or not is_blog_publicly_readable(blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if blog.owner_id == current_user.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi segnalare il tuo stesso blog.")
    return await _create_report(session, user=current_user, target_type=ReportTargetType.BLOG, target_id=blog.id, blog=blog, payload=payload)


@router.post("/posts/{post_id}/report", response_model=ReportOut, status_code=status.HTTP_201_CREATED)
async def report_post(
    post_id: uuid.UUID,
    payload: ReportRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> ContentReport:
    post = await session.get(Post, post_id)
    if post is None or not is_publicly_visible(post):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    blog = await session.get(Blog, post.blog_id)
    if blog is None or not is_blog_publicly_readable(blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    if post.author_id == current_user.id or blog.owner_id == current_user.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi segnalare un tuo contenuto.")
    return await _create_report(session, user=current_user, target_type=ReportTargetType.POST, target_id=post.id, blog=blog, payload=payload)
