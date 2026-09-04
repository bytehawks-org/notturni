import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.core.database import get_session
from app.domain.authorization import can_view_blog, is_publicly_visible
from app.domain.fragments import validate_fragment_text
from app.domain.permalinks import build_permalink
from app.models.blog import Blog
from app.models.post import Post
from app.models.post_fragment import PostFragment
from app.models.user import User

router = APIRouter()


class FragmentCreateRequest(BaseModel):
    text: str


class FragmentOut(BaseModel):
    id: uuid.UUID
    post_id: uuid.UUID
    text: str
    created_at: datetime

    model_config = {"from_attributes": True}


class FragmentCollectionOut(BaseModel):
    id: uuid.UUID
    text: str
    created_at: datetime
    post_title: str
    author_display_name: str
    permalink: str


async def _get_post_and_blog_or_404(session: AsyncSession, post_id: uuid.UUID) -> tuple[Post, Blog]:
    post = await session.get(Post, post_id)
    if post is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    return post, blog


async def _require_fragmentable(session: AsyncSession, current_user: User, post: Post, blog: Blog) -> None:
    """Un frammento si salva solo su un post pubblicato e visibile all'utente
    corrente (stessa regola d'accesso del permalink pubblico) — mai su una
    bozza, che può ancora cambiare da un momento all'altro."""
    if not is_publicly_visible(post) or not await can_view_blog(session, user_id=current_user.id, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")


@router.post("/posts/{post_id}/fragments", response_model=FragmentOut, status_code=status.HTTP_201_CREATED)
async def create_fragment(
    post_id: uuid.UUID,
    payload: FragmentCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> FragmentOut:
    post, blog = await _get_post_and_blog_or_404(session, post_id)
    await _require_fragmentable(session, current_user, post, blog)

    try:
        text = validate_fragment_text(payload.text, post.content)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    async def _existing() -> PostFragment | None:
        result = await session.execute(
            select(PostFragment).where(
                PostFragment.user_id == current_user.id,
                PostFragment.post_id == post_id,
                PostFragment.text == text,
            )
        )
        return result.scalar_one_or_none()

    fragment = await _existing()
    if fragment is not None:
        # stesso frammento già salvato: idempotente, non un errore (evita che
        # un doppio click sul menu contestuale del frontend produca un 409).
        return FragmentOut.model_validate(fragment)

    fragment = PostFragment(user_id=current_user.id, post_id=post_id, text=text)
    session.add(fragment)
    try:
        await session.commit()
    except IntegrityError:
        # corsa tra due richieste identiche in parallelo: la seconda trova il
        # vincolo unique(user, post, text) già violato dalla prima.
        await session.rollback()
        fragment = await _existing()
        assert fragment is not None
        return FragmentOut.model_validate(fragment)
    await session.refresh(fragment)
    return FragmentOut.model_validate(fragment)


@router.get("/posts/{post_id}/fragments", response_model=list[FragmentOut])
async def list_post_fragments(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[FragmentOut]:
    """Frammenti già salvati dall'utente corrente su questo post — il
    frontend li usa per ri-evidenziarli ad ogni lettura, indipendentemente
    dal fatto che si sia arrivati al post dalla pagina di raccolta."""
    result = await session.execute(
        select(PostFragment).where(PostFragment.user_id == current_user.id, PostFragment.post_id == post_id)
    )
    return [FragmentOut.model_validate(f) for f in result.scalars().all()]


@router.get("/users/me/fragments", response_model=list[FragmentCollectionOut])
async def list_my_fragments(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[FragmentCollectionOut]:
    """Raccolta unificata dei frammenti salvati dall'utente, più recenti prima."""
    result = await session.execute(
        select(PostFragment, Post, Blog)
        .join(Post, Post.id == PostFragment.post_id)
        .join(Blog, Blog.id == Post.blog_id)
        .where(PostFragment.user_id == current_user.id)
        .order_by(PostFragment.created_at.desc())
    )
    return [
        FragmentCollectionOut(
            id=fragment.id,
            text=fragment.text,
            created_at=fragment.created_at,
            post_title=post.title,
            # Colonna già risolta al salvataggio/ultima modifica del post
            # (Post.author_display_name), non ricalcolata da un alias
            # cambiato dopo come fa invece PostOut: semplificazione accettata
            # per una vista derivata/secondaria, non canonica come il post stesso.
            author_display_name=post.author_display_name,
            permalink=build_permalink(blog.slug, post),
        )
        for fragment, post, blog in result.all()
    ]


@router.delete("/fragments/{fragment_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_fragment(
    fragment_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    fragment = await session.get(PostFragment, fragment_id)
    if fragment is None or fragment.user_id != current_user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Frammento non trovato.")
    await session.delete(fragment)
    await session.commit()
