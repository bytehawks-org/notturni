import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import get_current_user
from app.core.database import get_session
from app.core.storage import avatar_public_url, delete_avatar, upload_avatar
from app.domain.i18n import validate_locale
from app.domain.profile import validate_country_code, validate_fallback_languages
from app.domain.usernames import validate_username
from app.models.blog import Blog
from app.models.follow import BlogFollow, UserFollow
from app.models.social_link import SocialLink
from app.models.user import PostAuthorNameStyle, User

router = APIRouter()


class ProfileUpdateRequest(BaseModel):
    # todo/USERS.md #7: citabile ovunque come @username (menzioni, permalink
    # profilo /u/{username}); cambiarlo si riflette subito ovunque perché
    # tutto il resto del sistema referenzia l'utente per id, non per
    # username — l'unica eccezione nota sono le @menzioni già scritte nel
    # testo dei post/pagine, salvate come testo semplice: restano invariate
    # e puntano allo username precedente. Assente lascia invariato.
    username: str | None = None
    bio: str | None = None
    first_name: str | None = None
    last_name: str | None = None
    # Alias pubblico globale (todo/BLOG.md #4): "" azzera, assente lascia
    # invariato.
    display_name: str | None = None
    # todo/USERS.md #2: come mostrare il proprio nome sui post quando il blog
    # non impone un alias. Assente lascia invariato.
    post_author_name_style: PostAuthorNameStyle | None = None
    # "" per azzerare uno di questi tre; assente lascia invariato (stesso
    # schema di cover_image_url/default_author_display_name altrove)
    country: str | None = None
    native_language: str | None = None
    # assente: lascia invariate; lista (anche vuota) la sostituisce
    fallback_languages: list[str] | None = None


class SocialLinkCreateRequest(BaseModel):
    label: str
    url: str


class SocialLinkOut(BaseModel):
    id: uuid.UUID
    label: str
    url: str
    position: int

    model_config = {"from_attributes": True}


class AvatarOut(BaseModel):
    avatar_url: str | None


class ProfileOut(BaseModel):
    username: str
    bio: str | None
    first_name: str | None
    last_name: str | None
    display_name: str | None
    post_author_name_style: PostAuthorNameStyle
    country: str | None
    native_language: str | None
    fallback_languages: list[str]
    avatar_url: str | None
    social_links: list[SocialLinkOut]
    created_at: datetime

    model_config = {"from_attributes": True}


class FollowerOut(BaseModel):
    username: str

    model_config = {"from_attributes": True}


class BlogFollowerCountOut(BaseModel):
    blog_slug: str
    blog_title: str
    # Nome sotto cui il blog appare pubblicamente (CLAUDE.md #1): se diverso
    # dallo username di chi lo gestisce, i follower di questo blog non lo
    # associano alla persona reale — vedi FollowStatsOut più sotto.
    alias: str | None
    followers: int


class FollowStatsOut(BaseModel):
    """Solo per il proprietario (`GET /users/me/follow-stats`): l'unico posto
    dove identità reale e alias di blog compaiono fianco a fianco, per
    tenere traccia di quante persone lo seguono in tutto — sotto qualunque
    identità abbia usato per farlo. Il conteggio per singola entità (utente
    reale o blog) resta invece visibile pubblicamente in modo separato,
    tramite gli endpoint /{username}/followers e /blogs/{slug}/followers,
    senza che i due si tocchino mai altrove."""

    user_followers: int
    blogs: list[BlogFollowerCountOut]
    total_followers: int


MAX_SOCIAL_LINKS = 5


async def _get_user_or_404(session: AsyncSession, username: str) -> User:
    result = await session.execute(select(User).where(User.username == username))
    user = result.scalar_one_or_none()
    if user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    return user


async def _get_user_with_profile_or_404(session: AsyncSession, username: str) -> User:
    result = await session.execute(
        select(User).where(User.username == username).options(selectinload(User.social_links))
    )
    user = result.scalar_one_or_none()
    if user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    return user


def _to_profile_out(user: User) -> ProfileOut:
    return ProfileOut(
        username=user.username,
        bio=user.bio,
        first_name=user.first_name,
        last_name=user.last_name,
        display_name=user.display_name,
        post_author_name_style=user.post_author_name_style,
        country=user.country,
        native_language=user.native_language,
        fallback_languages=user.fallback_languages,
        avatar_url=avatar_public_url(user.avatar_object_key) if user.avatar_object_key else None,
        social_links=[SocialLinkOut.model_validate(link) for link in user.social_links],
        created_at=user.created_at,
    )


@router.get("/{username}", response_model=ProfileOut)
async def get_profile(username: str, session: AsyncSession = Depends(get_session)) -> ProfileOut:
    user = await _get_user_with_profile_or_404(session, username)
    return _to_profile_out(user)


@router.patch("/me", response_model=ProfileOut)
async def update_profile(
    payload: ProfileUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> ProfileOut:
    if payload.username is not None:
        new_username = payload.username.strip().lower()
        if new_username != current_user.username:
            try:
                validate_username(new_username)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
            existing = await session.execute(select(User).where(User.username == new_username))
            if existing.scalar_one_or_none() is not None:
                raise HTTPException(status.HTTP_409_CONFLICT, "Username già in uso.")
            current_user.username = new_username
    if payload.bio is not None:
        current_user.bio = payload.bio
    if payload.first_name is not None:
        current_user.first_name = payload.first_name or None
    if payload.last_name is not None:
        current_user.last_name = payload.last_name or None
    if payload.display_name is not None:
        current_user.display_name = payload.display_name.strip() or None
    if payload.post_author_name_style is not None:
        current_user.post_author_name_style = payload.post_author_name_style
    if payload.country is not None:
        country = payload.country.strip().upper()
        if country:
            try:
                validate_country_code(country)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.country = country or None
    if payload.native_language is not None:
        native_language = payload.native_language.strip().lower()
        if native_language:
            try:
                validate_locale(native_language)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.native_language = native_language or None
    if payload.fallback_languages is not None:
        normalized = [loc.strip().lower() for loc in payload.fallback_languages]
        try:
            validate_fallback_languages(normalized)
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.fallback_languages = normalized

    await session.commit()
    await session.refresh(current_user, attribute_names=["social_links"])
    return _to_profile_out(current_user)


@router.post("/me/avatar", response_model=AvatarOut)
async def upload_my_avatar(
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> AvatarOut:
    content = await file.read()
    try:
        object_key = upload_avatar(
            user_id=current_user.id, content=content, content_type=file.content_type or ""
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    old_key = current_user.avatar_object_key
    current_user.avatar_object_key = object_key
    await session.commit()

    if old_key is not None:
        delete_avatar(old_key)

    return AvatarOut(avatar_url=avatar_public_url(object_key))


@router.delete("/me/avatar", status_code=status.HTTP_204_NO_CONTENT)
async def delete_my_avatar(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    if current_user.avatar_object_key is not None:
        delete_avatar(current_user.avatar_object_key)
        current_user.avatar_object_key = None
        await session.commit()


@router.post("/me/social-links", response_model=SocialLinkOut, status_code=status.HTTP_201_CREATED)
async def add_social_link(
    payload: SocialLinkCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> SocialLink:
    count = await session.execute(
        select(SocialLink).where(SocialLink.user_id == current_user.id)
    )
    existing_links = list(count.scalars().all())
    if len(existing_links) >= MAX_SOCIAL_LINKS:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST, f"Massimo {MAX_SOCIAL_LINKS} link social per profilo."
        )

    link = SocialLink(
        user_id=current_user.id,
        label=payload.label,
        url=payload.url,
        position=len(existing_links),
    )
    session.add(link)
    await session.commit()
    await session.refresh(link)
    return link


@router.delete("/me/social-links/{link_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_social_link(
    link_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    link = await session.get(SocialLink, link_id)
    if link is None or link.user_id != current_user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Link non trovato.")
    await session.delete(link)
    await session.commit()


@router.post("/{username}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def follow_user(
    username: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    target = await _get_user_or_404(session, username)
    if target.id == current_user.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi seguire te stesso.")

    existing = await session.execute(
        select(UserFollow).where(
            UserFollow.follower_id == current_user.id, UserFollow.followed_user_id == target.id
        )
    )
    if existing.scalar_one_or_none() is None:
        session.add(UserFollow(follower_id=current_user.id, followed_user_id=target.id))
        await session.commit()


@router.delete("/{username}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def unfollow_user(
    username: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    target = await _get_user_or_404(session, username)
    existing = await session.execute(
        select(UserFollow).where(
            UserFollow.follower_id == current_user.id, UserFollow.followed_user_id == target.id
        )
    )
    follow = existing.scalar_one_or_none()
    if follow is not None:
        await session.delete(follow)
        await session.commit()


@router.get("/{username}/followers", response_model=list[FollowerOut])
async def list_followers(username: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    target = await _get_user_or_404(session, username)
    result = await session.execute(
        select(User)
        .join(UserFollow, UserFollow.follower_id == User.id)
        .where(UserFollow.followed_user_id == target.id)
    )
    return list(result.scalars().all())


@router.get("/{username}/following", response_model=list[FollowerOut])
async def list_following(username: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    target = await _get_user_or_404(session, username)
    result = await session.execute(
        select(User)
        .join(UserFollow, UserFollow.followed_user_id == User.id)
        .where(UserFollow.follower_id == target.id)
    )
    return list(result.scalars().all())


@router.get("/me/follow-stats", response_model=FollowStatsOut)
async def my_follow_stats(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> FollowStatsOut:
    """CLAUDE.md #8: quante persone seguono l'utente in tutto, sommando chi lo
    segue con lo username reale (UserFollow) e chi segue uno qualunque dei
    suoi blog (BlogFollow) — anche quando quel blog appare con un alias che
    non rivela chi lo gestisce. Riservato al proprietario: è l'unico posto
    dove le due cose vengono messe insieme."""
    user_followers = await session.scalar(
        select(func.count()).select_from(UserFollow).where(UserFollow.followed_user_id == current_user.id)
    )

    blog_rows = await session.execute(
        select(
            Blog.slug,
            Blog.title,
            Blog.default_author_display_name,
            func.count(BlogFollow.id),
        )
        .outerjoin(BlogFollow, BlogFollow.blog_id == Blog.id)
        .where(Blog.owner_id == current_user.id)
        .group_by(Blog.id)
        .order_by(Blog.title)
    )
    blogs = [
        BlogFollowerCountOut(blog_slug=slug, blog_title=title, alias=alias, followers=count)
        for slug, title, alias, count in blog_rows.all()
    ]

    total = (user_followers or 0) + sum(b.followers for b in blogs)
    return FollowStatsOut(user_followers=user_followers or 0, blogs=blogs, total_followers=total)
