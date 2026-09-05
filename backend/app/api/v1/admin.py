import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import or_, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import require_platform_admin
from app.core.database import get_session
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
        target.platform_role = payload.platform_role

    if payload.is_active is not None:
        if target.id == current_user.id and not payload.is_active:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi disattivare il tuo stesso account.")
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
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminBlogOut:
    result = await session.execute(
        select(Blog).options(selectinload(Blog.owner)).where(Blog.id == blog_id)
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")

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

    post.is_hidden = payload.is_hidden
    await session.commit()
    await session.refresh(post)
    return _to_admin_post_out(post, blog, author)
