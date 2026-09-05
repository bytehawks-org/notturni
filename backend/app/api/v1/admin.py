import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, Request, status
from pydantic import BaseModel, field_validator
from sqlalchemy import or_, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import require_platform_admin
from app.core.database import get_session
from app.domain import audit
from app.models.audit_log import AuditActorType, AuditLog
from app.models.blog import Blog, BlogVisibility
from app.models.post import Post, PostStatus
from app.models.user import PlatformRole, User

router = APIRouter()

# Solo un Super Admin può promuovere/retrocedere da e verso questi ruoli:
# un Amministratore non deve poter creare altri amministratori o super admin.
PRIVILEGED_ROLES = {PlatformRole.AMMINISTRATORE, PlatformRole.SUPER_ADMIN}


class AdminUserOut(BaseModel):
    id: uuid.UUID
    username: str
    email: str
    platform_role: PlatformRole
    is_active: bool
    mfa_enabled: bool
    created_at: datetime

    model_config = {"from_attributes": True}


class AdminUserUpdateRequest(BaseModel):
    platform_role: PlatformRole | None = None
    is_active: bool | None = None


@router.get("/users", response_model=list[AdminUserOut])
async def list_users(
    q: str | None = None,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[User]:
    stmt = select(User).order_by(User.created_at)
    if q:
        needle = f"%{q}%"
        stmt = stmt.where(or_(User.username.ilike(needle), User.email.ilike(needle)))
    result = await session.execute(stmt)
    return list(result.scalars().all())


@router.patch("/users/{user_id}", response_model=AdminUserOut)
async def update_user(
    user_id: uuid.UUID,
    payload: AdminUserUpdateRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> User:
    target = await session.get(User, user_id)
    if target is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")

    if payload.platform_role is not None:
        touches_privileged_tier = (
            payload.platform_role in PRIVILEGED_ROLES or target.platform_role in PRIVILEGED_ROLES
        )
        if touches_privileged_tier and current_user.platform_role != PlatformRole.SUPER_ADMIN:
            raise HTTPException(
                status.HTTP_403_FORBIDDEN,
                "Solo un Super Admin può assegnare o rimuovere ruoli di amministrazione.",
            )
        if payload.platform_role != target.platform_role:
            await audit.record(
                session,
                action="user.role_change",
                actor=current_user,
                target_type="user",
                target_id=target.id,
                request=request,
                payload={"from": target.platform_role.value, "to": payload.platform_role.value},
            )
        target.platform_role = payload.platform_role

    if payload.is_active is not None:
        if target.id == current_user.id and not payload.is_active:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi disattivare il tuo stesso account.")
        if payload.is_active != target.is_active:
            await audit.record(
                session,
                action="user.activated" if payload.is_active else "user.deactivated",
                actor=current_user,
                target_type="user",
                target_id=target.id,
                request=request,
            )
        target.is_active = payload.is_active

    await session.commit()
    await session.refresh(target)
    return target


class AdminBlogOut(BaseModel):
    id: uuid.UUID
    slug: str
    title: str
    owner_username: str
    visibility: BlogVisibility
    is_suspended: bool
    created_at: datetime

    model_config = {"from_attributes": True}


def _to_admin_blog_out(blog: Blog) -> AdminBlogOut:
    return AdminBlogOut(
        id=blog.id,
        slug=blog.slug,
        title=blog.title,
        owner_username=blog.owner.username,
        visibility=blog.visibility,
        is_suspended=blog.is_suspended,
        created_at=blog.created_at,
    )


class AdminBlogUpdateRequest(BaseModel):
    is_suspended: bool


@router.get("/blogs", response_model=list[AdminBlogOut])
async def list_blogs(
    q: str | None = None,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AdminBlogOut]:
    stmt = select(Blog).options(selectinload(Blog.owner)).order_by(Blog.created_at)
    if q:
        needle = f"%{q}%"
        stmt = stmt.join(User, Blog.owner_id == User.id).where(
            or_(Blog.slug.ilike(needle), Blog.title.ilike(needle), User.username.ilike(needle))
        )
    result = await session.execute(stmt)
    return [_to_admin_blog_out(blog) for blog in result.scalars().all()]


@router.patch("/blogs/{blog_id}", response_model=AdminBlogOut)
async def update_blog(
    blog_id: uuid.UUID,
    payload: AdminBlogUpdateRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminBlogOut:
    result = await session.execute(
        select(Blog).options(selectinload(Blog.owner)).where(Blog.id == blog_id)
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")

    if payload.is_suspended != blog.is_suspended:
        await audit.record(
            session,
            action="blog.suspended" if payload.is_suspended else "blog.unsuspended",
            actor=current_user,
            target_type="blog",
            target_id=blog.id,
            blog_id=blog.id,
            request=request,
            payload={"slug": blog.slug},
        )
    blog.is_suspended = payload.is_suspended
    await session.commit()
    await session.refresh(blog, attribute_names=["owner"])
    return _to_admin_blog_out(blog)


class AdminPostOut(BaseModel):
    id: uuid.UUID
    title: str
    slug: str
    blog_slug: str
    blog_title: str
    author_username: str
    status: PostStatus
    is_hidden: bool
    published_at: datetime | None
    created_at: datetime

    model_config = {"from_attributes": True}


def _to_admin_post_out(post: Post, blog: Blog, author: User) -> AdminPostOut:
    return AdminPostOut(
        id=post.id,
        title=post.title,
        slug=post.slug,
        blog_slug=blog.slug,
        blog_title=blog.title,
        author_username=author.username,
        status=post.status,
        is_hidden=post.is_hidden,
        published_at=post.published_at,
        created_at=post.created_at,
    )


class AdminPostUpdateRequest(BaseModel):
    is_hidden: bool


@router.get("/posts", response_model=list[AdminPostOut])
async def list_posts(
    q: str | None = None,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AdminPostOut]:
    stmt = (
        select(Post, Blog, User)
        .join(Blog, Post.blog_id == Blog.id)
        .join(User, Post.author_id == User.id)
        .order_by(Post.created_at.desc())
    )
    if q:
        needle = f"%{q}%"
        stmt = stmt.where(
            or_(
                Post.title.ilike(needle),
                Post.slug.ilike(needle),
                Blog.slug.ilike(needle),
                User.username.ilike(needle),
            )
        )
    result = await session.execute(stmt)
    return [_to_admin_post_out(post, blog, author) for post, blog, author in result.all()]


@router.patch("/posts/{post_id}", response_model=AdminPostOut)
async def update_post(
    post_id: uuid.UUID,
    payload: AdminPostUpdateRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminPostOut:
    result = await session.execute(
        select(Post, Blog, User)
        .join(Blog, Post.blog_id == Blog.id)
        .join(User, Post.author_id == User.id)
        .where(Post.id == post_id)
    )
    row = result.first()
    if row is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    post, blog, author = row

    if payload.is_hidden != post.is_hidden:
        await audit.record(
            session,
            action="post.hidden" if payload.is_hidden else "post.unhidden",
            actor=current_user,
            target_type="post",
            target_id=post.id,
            blog_id=post.blog_id,
            request=request,
            payload={"slug": post.slug, "blog_slug": blog.slug},
        )
    post.is_hidden = payload.is_hidden
    await session.commit()
    await session.refresh(post)
    return _to_admin_post_out(post, blog, author)


class AuditLogOut(BaseModel):
    id: uuid.UUID
    occurred_at: datetime
    actor_type: AuditActorType
    actor_id: uuid.UUID | None
    actor_label: str | None
    action: str
    target_type: str | None
    target_id: uuid.UUID | None
    blog_id: uuid.UUID | None
    ip: str | None
    user_agent: str | None
    payload: dict

    model_config = {"from_attributes": True}

    @field_validator("ip", mode="before")
    @classmethod
    def _ip_to_str(cls, v: object) -> str | None:
        # asyncpg restituisce la colonna INET come oggetto ipaddress, non str
        return str(v) if v is not None else None


@router.get("/audit-log", response_model=list[AuditLogOut])
async def list_audit_log(
    action: str | None = None,
    actor_id: uuid.UUID | None = None,
    target_id: uuid.UUID | None = None,
    blog_id: uuid.UUID | None = None,
    since: datetime | None = None,
    until: datetime | None = None,
    limit: int = 100,
    offset: int = 0,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AuditLog]:
    """Consultazione del registro di audit (append-only). Filtri opzionali per
    azione, attore, oggetto, blog e intervallo temporale; ordine dal più
    recente. Gli eventi oltre la retention non sono qui ma negli archivi su
    storage (vedi `app/workers/audit_maintenance.py`)."""
    stmt = select(AuditLog).order_by(AuditLog.occurred_at.desc())
    if action is not None:
        stmt = stmt.where(AuditLog.action == action)
    if actor_id is not None:
        stmt = stmt.where(AuditLog.actor_id == actor_id)
    if target_id is not None:
        stmt = stmt.where(AuditLog.target_id == target_id)
    if blog_id is not None:
        stmt = stmt.where(AuditLog.blog_id == blog_id)
    if since is not None:
        stmt = stmt.where(AuditLog.occurred_at >= since)
    if until is not None:
        stmt = stmt.where(AuditLog.occurred_at < until)
    stmt = stmt.limit(min(max(limit, 1), 500)).offset(max(offset, 0))
    result = await session.execute(stmt)
    return list(result.scalars().all())
