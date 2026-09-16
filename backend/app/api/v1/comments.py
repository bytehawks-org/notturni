import hashlib
import uuid
from datetime import datetime, timedelta, timezone

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from pydantic import BaseModel, EmailStr
from sqlalchemy import or_, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.core.captcha import verify_turnstile
from app.core.database import get_session
from app.core.http import client_ip
from app.domain import audit
from app.domain.authorization import can_moderate_comments
from app.domain.comments_mode import effective_comments_mode
from app.domain.display_names import resolve_personal_display_name
from app.models.blog import Blog
from app.models.comment import BlogBlockedAuthor, Comment, CommentsMode, CommentStatus
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
    # B4: segnalazione ai moderatori di piattaforma (solo per chi modera).
    reported_to_platform: bool = False
    report_note: str | None = None

    model_config = {"from_attributes": True}


_effective_comments_mode = effective_comments_mode


def _email_hash(email: str) -> str:
    return hashlib.sha256(email.strip().lower().encode()).hexdigest()


async def _is_blocked(session: AsyncSession, *, blog_id: uuid.UUID, user: User | None, email: str | None) -> bool:
    conditions = []
    if user is not None:
        conditions.append(BlogBlockedAuthor.user_id == user.id)
    if email:
        conditions.append(BlogBlockedAuthor.email_hash == _email_hash(email))
    if not conditions:
        return False
    row = await session.execute(
        select(BlogBlockedAuthor.id).where(BlogBlockedAuthor.blog_id == blog_id, or_(*conditions)).limit(1)
    )
    return row.scalar_one_or_none() is not None


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
        reported_to_platform=comment.reported_to_platform,
        report_note=comment.report_note,
    )


async def _comments_out_batch(session: AsyncSession, comments: list[Comment]) -> list[CommentOut]:
    """Come `_comment_out`, ma per una lista: un solo `SELECT ... IN` per gli
    autori registrati invece di uno per commento (N+1 — stesso principio di
    `list_blog_comments`/`_posts_out`)."""
    author_ids = {c.author_id for c in comments if c.author_id is not None}
    authors: dict[uuid.UUID, User] = {}
    if author_ids:
        res = await session.execute(select(User).where(User.id.in_(author_ids)))
        authors = {u.id: u for u in res.scalars()}

    out: list[CommentOut] = []
    for comment in comments:
        display_name = comment.author_display_name
        author = authors.get(comment.author_id) if comment.author_id is not None else None
        if author is not None:
            display_name = resolve_personal_display_name(author)
        out.append(
            CommentOut(
                id=comment.id,
                post_id=comment.post_id,
                parent_id=comment.parent_id,
                author_id=comment.author_id,
                author_display_name=display_name,
                status=comment.status,
                content=comment.content,
                created_at=comment.created_at,
                reported_to_platform=comment.reported_to_platform,
                report_note=comment.report_note,
            )
        )
    return out


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

    # B4: lista dei bloccati del blog (utente registrato o hash dell'email anonima)
    if await _is_blocked(session, blog_id=blog.id, user=current_user, email=payload.author_email):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Non puoi commentare su questo blog.")

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
    return await _comments_out_batch(session, list(result.scalars().all()))


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
    return await _comments_out_batch(session, list(result.scalars().all()))


class BlogCommentOut(CommentOut):
    """Come CommentOut, con il titolo/slug del post di appartenenza: la
    moderazione per-blog (dashboard/blog) mostra i commenti di tutti i post
    insieme e deve sapere a quale post si riferisce ciascuno."""

    post_title: str
    post_slug: str


@router.get("/blogs/{blog_slug}/comments", response_model=list[BlogCommentOut])
async def list_blog_comments(
    blog_slug: str,
    status_filter: CommentStatus | None = Query(CommentStatus.PENDING, alias="status"),
    reported: bool = False,
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
            .where(Post.blog_id == blog.id)
            .where(Comment.reported_to_platform.is_(True) if reported else (Comment.status == status_filter))
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
                reported_to_platform=comment.reported_to_platform,
                report_note=comment.report_note,
                post_title=post_title,
                post_slug=post_slug,
            )
        )
    return out


async def _comment_with_blog(session: AsyncSession, comment_id: uuid.UUID, user: User) -> tuple[Comment, Post, Blog]:
    comment = await session.get(Comment, comment_id)
    if comment is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Commento non trovato.")
    post = await session.get(Post, comment.post_id)
    assert post is not None
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    if not await can_moderate_comments(session, user=user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")
    return comment, post, blog


class ReportRequest(BaseModel):
    note: str


@router.post("/comments/{comment_id}/report", response_model=CommentOut)
async def report_comment(
    comment_id: uuid.UUID,
    payload: ReportRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CommentOut:
    """B4 (mockup 5b "Report to platform"): il proprietario/mediatore segnala
    il commento ai moderatori di piattaforma con una nota obbligatoria; il
    commento compare in `GET /admin/comments?reported=true`."""
    note = payload.note.strip()
    if not note:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "La nota della segnalazione è obbligatoria.")
    comment, post, blog = await _comment_with_blog(session, comment_id, current_user)
    comment.reported_to_platform = True
    comment.report_note = note[:1000]
    comment.reported_at = datetime.now(timezone.utc)
    await audit.record(
        session,
        action="comment.reported",
        actor=current_user,
        target_type="comment",
        target_id=comment.id,
        blog_id=blog.id,
        request=request,
        payload={"post_slug": post.slug, "blog_slug": blog.slug, "note": note[:1000]},
    )
    await session.commit()
    await session.refresh(comment)
    return await _comment_out(session, comment)


class BlockRequest(BaseModel):
    note: str | None = None


class BlockedAuthorOut(BaseModel):
    id: uuid.UUID
    label: str
    is_anonymous: bool
    note: str | None
    created_at: datetime


def _blocked_out(row: BlogBlockedAuthor) -> BlockedAuthorOut:
    return BlockedAuthorOut(id=row.id, label=row.label, is_anonymous=row.user_id is None, note=row.note, created_at=row.created_at)


@router.post("/comments/{comment_id}/block-author", response_model=BlockedAuthorOut)
async def block_comment_author(
    comment_id: uuid.UUID,
    payload: BlockRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlockedAuthorOut:
    """B4 (mockup 5b "Block author"): aggiunge l'autore del commento alla
    lista dei bloccati del blog (utente registrato, oppure hash dell'email
    per un anonimo — mai l'email in chiaro) e nasconde il commento. Non si
    può bloccare il proprietario del blog."""
    comment, post, blog = await _comment_with_blog(session, comment_id, current_user)
    if comment.author_id == blog.owner_id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi bloccare il proprietario del blog.")
    if comment.author_id is None and not comment.author_email:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Autore non identificabile.")
    existing = (
        await session.execute(
            select(BlogBlockedAuthor).where(
                BlogBlockedAuthor.blog_id == blog.id,
                (BlogBlockedAuthor.user_id == comment.author_id)
                if comment.author_id is not None
                else (BlogBlockedAuthor.email_hash == _email_hash(comment.author_email or "")),
            )
        )
    ).scalar_one_or_none()
    if existing is None:
        label = comment.author_display_name
        if comment.author_id is not None:
            author = await session.get(User, comment.author_id)
            if author is not None:
                label = f"@{author.username}"
        existing = BlogBlockedAuthor(
            blog_id=blog.id,
            user_id=comment.author_id,
            email_hash=None if comment.author_id is not None else _email_hash(comment.author_email or ""),
            label=label,
            note=(payload.note or "").strip()[:255] or None,
            created_by_id=current_user.id,
        )
        session.add(existing)
    if comment.status != CommentStatus.REJECTED:
        comment.status = CommentStatus.REJECTED
    await audit.record(
        session,
        action="comment.author_blocked",
        actor=current_user,
        target_type="comment",
        target_id=comment.id,
        blog_id=blog.id,
        request=request,
        payload={"post_slug": post.slug, "blog_slug": blog.slug, "anonymous": comment.author_id is None},
    )
    await session.commit()
    await session.refresh(existing)
    return _blocked_out(existing)


@router.get("/blogs/{blog_slug}/blocked", response_model=list[BlockedAuthorOut])
async def list_blocked_authors(
    blog_slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BlockedAuthorOut]:
    blog = (await session.execute(select(Blog).where(Blog.slug == blog_slug))).scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if not await can_moderate_comments(session, user=current_user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")
    rows = (
        await session.execute(
            select(BlogBlockedAuthor).where(BlogBlockedAuthor.blog_id == blog.id).order_by(BlogBlockedAuthor.created_at.desc())
        )
    ).scalars().all()
    return [_blocked_out(r) for r in rows]


@router.delete("/blogs/{blog_slug}/blocked/{block_id}", status_code=status.HTTP_204_NO_CONTENT)
async def unblock_author(
    blog_slug: str,
    block_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = (await session.execute(select(Blog).where(Blog.slug == blog_slug))).scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if not await can_moderate_comments(session, user=current_user, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o mediatore.")
    row = await session.get(BlogBlockedAuthor, block_id)
    if row is None or row.blog_id != blog.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blocco non trovato.")
    await session.delete(row)
    await session.commit()


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
