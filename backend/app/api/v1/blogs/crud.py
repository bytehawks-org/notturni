"""Blog: creazione, elenco, dettaglio, modifica, follow."""

from fastapi import Depends, HTTPException, status
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.api.v1.blogs._common import (
    BlogCreateRequest,
    BlogOut,
    BlogUpdateRequest,
    FollowerOut,
    MembershipBlogOut,
    _get_blog_or_404,
    _require_blog_viewable,
    _to_blog_out,
)
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.revalidation import blog_tag, feed_tag, revalidate_frontend
from app.domain.blog_rules import (
    assert_can_create_blog,
    validate_blog_description,
    validate_blog_slug,
    validate_blog_subtitle,
)
from app.domain.i18n import validate_locale
from app.models.blog import Blog, BlogMembership
from app.models.follow import BlogFollow
from app.models.user import User


@router.post("", response_model=BlogOut, status_code=status.HTTP_201_CREATED)
async def create_blog(
    payload: BlogCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    try:
        validate_blog_slug(payload.slug)
        validate_locale(payload.default_locale)
        if payload.subtitle:
            validate_blog_subtitle(payload.subtitle)
        if payload.description:
            validate_blog_description(payload.description)
        await assert_can_create_blog(session, current_user.id)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(select(Blog).where(Blog.slug == payload.slug))
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso.")

    blog = Blog(
        slug=payload.slug,
        title=payload.title,
        subtitle=(payload.subtitle or None),
        description=(payload.description or None),
        visibility=payload.visibility,
        default_locale=payload.default_locale,
        default_author_display_name=(payload.default_author_display_name or None),
        owner_id=current_user.id,
    )
    session.add(blog)
    await session.commit()
    await session.refresh(blog)
    return blog


@router.get("/mine", response_model=list[BlogOut])
async def list_my_blogs(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Blog]:
    """Solo i blog di proprietà. I blog su cui l'utente ha una membership
    (co-autore/mediatore/...) sono su `GET /api/v1/blogs/member-of`."""
    result = await session.execute(select(Blog).where(Blog.owner_id == current_user.id))
    return list(result.scalars().all())


@router.get("/member-of", response_model=list[MembershipBlogOut])
async def list_blogs_i_belong_to(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MembershipBlogOut]:
    """Blog altrui su cui l'utente corrente ha una membership attiva
    (accettando un invito, vedi sotto)."""
    result = await session.execute(
        select(BlogMembership, Blog)
        .join(Blog, Blog.id == BlogMembership.blog_id)
        .where(BlogMembership.user_id == current_user.id)
        .order_by(Blog.title)
    )
    return [
        MembershipBlogOut(
            blog=BlogOut.model_validate(blog),
            role=membership.role,
            author_display_name=membership.author_display_name,
        )
        for membership, blog in result.all()
    ]


@router.get("/{slug}", response_model=BlogOut)
async def get_blog(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    return _to_blog_out(blog, current_user)


@router.patch("/{slug}", response_model=BlogOut)
async def update_blog(
    slug: str,
    payload: BlogUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare il blog.")

    try:
        if payload.subtitle:
            validate_blog_subtitle(payload.subtitle)
        if payload.description:
            validate_blog_description(payload.description)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.title is not None:
        blog.title = payload.title
    if payload.subtitle is not None:
        blog.subtitle = payload.subtitle or None
    if payload.description is not None:
        blog.description = payload.description or None
    if payload.visibility is not None:
        blog.visibility = payload.visibility
    if payload.allow_anonymous_comments is not None:
        blog.allow_anonymous_comments = payload.allow_anonymous_comments
    if payload.mentions_enabled is not None:
        blog.mentions_enabled = payload.mentions_enabled
    if payload.static_pages_enabled is not None:
        blog.static_pages_enabled = payload.static_pages_enabled
    if payload.default_author_display_name is not None:
        blog.default_author_display_name = payload.default_author_display_name or None

    await session.commit()
    await session.refresh(blog)
    # titolo/sottotitolo/visibilità/menzioni cambiano le pagine pubbliche del
    # blog e la sua presenza nel feed della homepage.
    await revalidate_frontend([blog_tag(blog.slug), feed_tag()])
    return blog


@router.post("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def follow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    if existing.scalar_one_or_none() is None:
        session.add(BlogFollow(follower_id=current_user.id, blog_id=blog.id))
        await session.commit()


@router.delete("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def unfollow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    follow = existing.scalar_one_or_none()
    if follow is not None:
        await session.delete(follow)
        await session.commit()


@router.get("/{slug}/followers", response_model=list[FollowerOut])
async def list_blog_followers(slug: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    blog = await _get_blog_or_404(session, slug)
    result = await session.execute(
        select(User).join(BlogFollow, BlogFollow.follower_id == User.id).where(BlogFollow.blog_id == blog.id)
    )
    return list(result.scalars().all())
