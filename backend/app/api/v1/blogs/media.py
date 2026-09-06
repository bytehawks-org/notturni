"""Upload di media per l'editor e autocomplete delle @menzioni."""

from fastapi import Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import _get_blog_or_404, _require_blog_write_access
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.storage import content_public_url, upload_media
from app.domain.moderation import classify_image
from app.models.blog import Blog, BlogMembership
from app.models.follow import BlogFollow
from app.models.user import User


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


class MentionableUserOut(BaseModel):
    username: str
    display_name: str | None

    model_config = {"from_attributes": True}


@router.get("/{slug}/mentionable-users", response_model=list[MentionableUserOut])
async def list_mentionable_users(
    slug: str,
    q: str = "",
    limit: int = 8,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MentionableUserOut]:
    """Suggerimenti per l'autocomplete delle @menzioni nell'editor
    (todo/EDITOR.md): proprietario, collaboratori e follower del blog il cui
    username o nome pubblico inizia/contiene `q`. Richiede accesso in scrittura
    al blog. Se le menzioni sono disattivate sul blog, ritorna lista vuota."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    if not blog.mentions_enabled:
        return []

    limit = min(max(limit, 1), 25)
    prefix = q.strip().lstrip("@").lower()

    related_ids = select(BlogMembership.user_id).where(BlogMembership.blog_id == blog.id).union(
        select(BlogFollow.follower_id).where(BlogFollow.blog_id == blog.id),
        select(Blog.owner_id).where(Blog.id == blog.id),
    )
    stmt = select(User).where(User.id.in_(related_ids))
    if prefix:
        like = f"%{prefix}%"
        stmt = stmt.where(
            func.lower(User.username).like(f"{prefix}%")
            | func.lower(func.coalesce(User.display_name, "")).like(like)
        )
    stmt = stmt.order_by(User.username).limit(limit)
    result = await session.execute(stmt)
    return [
        MentionableUserOut(username=u.username, display_name=u.display_name)
        for u in result.scalars().all()
    ]
