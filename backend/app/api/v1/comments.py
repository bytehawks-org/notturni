import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from pydantic import BaseModel, EmailStr
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.core.captcha import verify_turnstile
from app.core.database import get_session
from app.core.http import client_ip
from app.domain import audit
from app.domain.authorization import can_moderate_comments
from app.domain.display_names import resolve_personal_display_name
from app.models.blog import Blog
from app.models.comment import Comment, CommentsMode, CommentStatus
from app.models.post import Post
from app.models.user import User

router = APIRouter()


class CommentCreateRequest(BaseModel):
    content: str
    # risposta a un altro commento dello stesso post (thread); assente/None
    # per un commento di primo livello
    parent_id: uuid.UUID | None = None
    # richiesti solo se il commento non è di un utente autenticato
    author_display_name: str | None = None
    author_email: EmailStr | None = None
    # richiesto solo per un commento non registrato su un post/blog con
    # comments_mode "everyone" (CLAUDE.md #1) — token del widget Cloudflare
    # Turnstile, verificato server-side prima di accettare il commento.
    captcha_token: str | None = None


class CommentOut(BaseModel):
    id: uuid.UUID
    post_id: uuid.UUID
    parent_id: uuid.UUID | None
    author_id: uuid.UUID | None
    author_display_name: str
    status: CommentStatus
    content: str
    created_at: datetime

    model_config = {"from_attributes": True}


def _effective_comments_mode(post: Post, blog: Blog) -> CommentsMode:
    return post.comments_mode or blog.comments_mode


async def _get_post_and_blog_or_404(session: AsyncSession, post_id: uuid.UUID) -> tuple[Post, Blog]:
    post = await session.get(Post, post_id)
    if post is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    return post, blog


async def _comment_out(session: AsyncSession, comment: Comment) -> CommentOut:
    # Ricalcolato ad ogni lettura per i commenti di utenti registrati (non
    # dalla colonna, che resta solo l'ultimo valore scritto): un cambio di
    # username o di preferenza di visualizzazione (dashboard/profilo) si
    # riflette subito anche sui commenti passati (CLAUDE.md #1). I commenti
    # anonimi restano invece il nome libero indicato da chi ha commentato,
    # senza un account a cui risalire.
    display_name = comment.author_display_name
    if comment.author_id is not None:
        author = await session.get(User, comment.author_id)
        if author is not None:
            display_name = resolve_personal_display_name(author)
    return CommentOut(
        id=comment.id,
        post_id=comment.post_id,
        parent_id=comment.parent_id,
        author_id=comment.author_id,
        author_display_name=display_name,
        status=comment.status,
        content=comment.content,
        created_at=comment.created_at,
    )


@router.post("/posts/{post_id}/comments", response_model=CommentOut, status_code=status.HTTP_201_CREATED)
async def create_comment(
    post_id: uuid.UUID,
    payload: CommentCreateRequest,
    request: Request,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> CommentOut:
    post, blog = await _get_post_and_blog_or_404(session, post_id)
    mode = _effective_comments_mode(post, blog)

    if mode == CommentsMode.CLOSED:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "I commenti sono chiusi per questo post.")

    if payload.parent_id is not None:
        parent = await session.get(Comment, payload.parent_id)
        if parent is None or parent.post_id != post_id:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Commento a cui rispondere non valido.")

    if current_user is not None:
        # CLAUDE.md #1: utenti registrati, nessuna moderazione obbligatoria
        comment = Comment(
            post_id=post_id,
            parent_id=payload.parent_id,
            author_id=current_user.id,
            author_display_name=current_user.username,
            content=payload.content,
            status=CommentStatus.APPROVED,
        )
    else:
        if mode == CommentsMode.MEMBERS:
            raise HTTPException(
                status.HTTP_401_UNAUTHORIZED, "Questo blog accetta commenti solo da utenti registrati."
            )
        if not payload.author_display_name or not payload.author_email:
            raise HTTPException(
                status.HTTP_400_BAD_REQUEST,
                "Nome e email sono richiesti per commentare senza account.",
            )
        # CommentsMode.EVERYONE: verifica captcha per contenere lo spam
        # (ROADMAP.md §1) — nessun bypass se il servizio non è raggiungibile,
        # a differenza della moderazione immagini (qui l'obiettivo è proprio
        # bloccare i bot, non un aiuto best-effort).
        if not payload.captcha_token or not await verify_turnstile(payload.captcha_token, client_ip(request)):
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Verifica captcha non superata.")
        # CLAUDE.md #1: commenti non registrati sempre moderati prima della pubblicazione
        comment = Comment(
            post_id=post_id,
            parent_id=payload.parent_id,
            author_id=None,
            author_display_name=payload.author_display_name,
            author_email=payload.author_email,
            content=payload.content,
            status=CommentStatus.PENDING,
        )

    session.add(comment)
    await session.commit()
    await session.refresh(comment)
    return await _comment_out(session, comment)


@router.get("/posts/{post_id}/comments", response_model=list[CommentOut])
async def list_approved_comments(post_id: uuid.UUID, session: AsyncSession = Depends(get_session)) -> list[CommentOut]:
    await _get_post_and_blog_or_404(session, post_id)
    result = await session.execute(
        select(Comment).where(Comment.post_id == post_id, Comment.status == CommentStatus.APPROVED)
    )
    return [await _comment_out(session, c) for c in result.scalars().all()]


@router.get("/posts/{post_id}/comments/pending", response_model=list[CommentOut])
async def list_pending_comments(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[CommentOut]:
    _post, blog = await _get_post_and_blog_or_404(session, post_id)
    if not await can_moderate_comments(session, user=current_user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")

    result = await session.execute(
        select(Comment).where(Comment.post_id == post_id, Comment.status == CommentStatus.PENDING)
    )
    return [await _comment_out(session, c) for c in result.scalars().all()]


class BlogCommentOut(CommentOut):
    """Come CommentOut, con il titolo/slug del post di appartenenza: la
    moderazione per-blog (dashboard/blog) mostra i commenti di tutti i post
    insieme e deve sapere a quale post si riferisce ciascuno."""

    post_title: str
    post_slug: str


@router.get("/blogs/{blog_slug}/comments", response_model=list[BlogCommentOut])
async def list_blog_comments(
    blog_slug: str,
    status_filter: CommentStatus = Query(CommentStatus.PENDING, alias="status"),
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BlogCommentOut]:
    """Commenti di *tutti* i post di un blog con lo stato indicato (default
    `pending`), dal più recente — per la moderazione trasversale nel
    dashboard senza una fetch per ogni post (N+1). Riservato a
    proprietario/mediatore del blog."""
    blog = (
        await session.execute(select(Blog).where(Blog.slug == blog_slug))
    ).scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if not await can_moderate_comments(session, user=current_user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")

    rows = (
        await session.execute(
            select(Comment, Post.title, Post.slug)
            .join(Post, Post.id == Comment.post_id)
            .where(Post.blog_id == blog.id, Comment.status == status_filter)
            .order_by(Comment.created_at.desc())
        )
    ).all()

    # display name degli autori registrati ricalcolato in blocco (un solo
    # SELECT invece di uno per commento — stesso principio di _posts_out).
    author_ids = {c.author_id for c, _t, _s in rows if c.author_id is not None}
    authors: dict[uuid.UUID, User] = {}
    if author_ids:
        res = await session.execute(select(User).where(User.id.in_(author_ids)))
        authors = {u.id: u for u in res.scalars()}

    out: list[BlogCommentOut] = []
    for comment, post_title, post_slug in rows:
        display_name = comment.author_display_name
        author = authors.get(comment.author_id) if comment.author_id is not None else None
        if author is not None:
            display_name = resolve_personal_display_name(author)
        out.append(
            BlogCommentOut(
                id=comment.id,
                post_id=comment.post_id,
                parent_id=comment.parent_id,
                author_id=comment.author_id,
                author_display_name=display_name,
                status=comment.status,
                content=comment.content,
                created_at=comment.created_at,
                post_title=post_title,
                post_slug=post_slug,
            )
        )
    return out


async def _moderate(
    comment_id: uuid.UUID,
    new_status: CommentStatus,
    current_user: User,
    request: Request,
    session: AsyncSession,
) -> CommentOut:
    comment = await session.get(Comment, comment_id)
    if comment is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Commento non trovato.")
    post = await session.get(Post, comment.post_id)
    assert post is not None
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None

    if not await can_moderate_comments(session, user=current_user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")

    comment.status = new_status
    # tracciato in audit log (ROADMAP.md §3): prima un gap noto, la
    # moderazione commenti — per-blog o trasversale via /admin/comments — non
    # lasciava nessuna traccia. blog_alias: alias pubblico del blog in
    # questione (non dell'attore), utile a capire sotto quale identità
    # pubblica è comparso il commento moderato.
    await audit.record(
        session,
        action=f"comment.{new_status.value}",
        actor=current_user,
        target_type="comment",
        target_id=comment.id,
        blog_id=blog.id,
        request=request,
        payload={
            "post_id": str(post.id),
            "post_slug": post.slug,
            "blog_slug": blog.slug,
            "blog_alias": blog.default_author_display_name,
        },
    )
    await session.commit()
    await session.refresh(comment)
    return await _comment_out(session, comment)


@router.post("/comments/{comment_id}/approve", response_model=CommentOut)
async def approve_comment(
    comment_id: uuid.UUID,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CommentOut:
    return await _moderate(comment_id, CommentStatus.APPROVED, current_user, request, session)


@router.post("/comments/{comment_id}/reject", response_model=CommentOut)
async def reject_comment(
    comment_id: uuid.UUID,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CommentOut:
    return await _moderate(comment_id, CommentStatus.REJECTED, current_user, request, session)
