import asyncio
import uuid
from datetime import datetime, timedelta, timezone
from typing import Literal

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from pydantic import BaseModel, field_validator, model_validator
from sqlalchemy import update as sa_update
import httpx
from sqlalchemy import func, or_, select, text
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import require_platform_admin, require_platform_moderator, require_super_admin
from app.core.config import settings
from app.core.database import get_session
from app.core.redis import get_redis
from app.core.revalidation import blog_tag, feed_tag, platform_footer_tag, post_tag, revalidate_frontend
from app.domain import audit
from app.domain.display_names import resolve_personal_display_name
from app.domain.gdpr import anonymize_and_deactivate_user, export_user_data
from app.domain.gdpr_queue import new_request
from app.domain.interests import validate_interest_list
from app.domain.password_reset import request_password_reset
from app.domain.rate_limit import enforce_rate_limit
from app.domain.verification import DOMAIN_PATTERN, sync_verification_tier
from app.domain.platform_config import (
    MAX_AUDIT_RETENTION_DAYS,
    MAX_FOOTER_MARKDOWN_LENGTH,
    MIN_AUDIT_RETENTION_DAYS,
    REGISTRATION_MODES,
    SSO_PROVIDER_NAMES,
    SUPPORTED_LOCALES,
    get_platform_config,
    touch,
)
from app.models.gdpr_request import GdprRequest, GdprRequestStatus, GdprRequestType
from app.models.platform_config import PlatformConfig
from app.core.oauth import configured_providers
from app.domain.blog_rules import RESERVED_BLOG_SLUGS, SLUG_PATTERN
from app.models.audit_log import AuditActorType, AuditLog
from app.models.blog import Blog, BlogVisibility
from app.models.comment import Comment, CommentStatus
from app.models.content_report import ContentReport, ReportStatus, ReportTargetType
from app.models.post import Post, PostStatus
from app.models.user import PlatformRole, User
from app.models.user_session import UserSession

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
    # todo/UX_REDESIGN.md B1 (mockup 1g): blog di proprietà e ultimo accesso
    # (ultimo uso di una sessione di refresh, `user_sessions.last_used_at`).
    blogs_count: int = 0
    last_seen_at: datetime | None = None

    model_config = {"from_attributes": True}


async def _admin_users_out(session: AsyncSession, users: list[User]) -> list[AdminUserOut]:
    if not users:
        return []
    ids = [u.id for u in users]
    blog_counts = dict(
        (await session.execute(select(Blog.owner_id, func.count()).where(Blog.owner_id.in_(ids)).group_by(Blog.owner_id))).all()
    )
    last_seen = dict(
        (
            await session.execute(
                select(UserSession.user_id, func.max(UserSession.last_used_at))
                .where(UserSession.user_id.in_(ids))
                .group_by(UserSession.user_id)
            )
        ).all()
    )
    return [
        AdminUserOut.model_validate(u).model_copy(
            update={"blogs_count": int(blog_counts.get(u.id, 0)), "last_seen_at": last_seen.get(u.id)}
        )
        for u in users
    ]


def _require_note(note: str | None) -> str:
    """todo/UX_REDESIGN.md B5 (mockup 5e): ogni azione admin che cambia lo
    stato di un utente/blog/post richiede una nota, che finisce nel registro
    di audit (`payload.note`)."""
    cleaned = (note or "").strip()
    if len(cleaned) < 3:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Serve una nota per il registro (almeno 3 caratteri).")
    return cleaned[:1000]


class AdminUserUpdateRequest(BaseModel):
    platform_role: PlatformRole | None = None
    is_active: bool | None = None
    # obbligatoria se cambia il ruolo o l'attivazione (B5)
    note: str | None = None


@router.get("/users", response_model=list[AdminUserOut])
async def list_users(
    q: str | None = None,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AdminUserOut]:
    stmt = select(User).order_by(User.created_at)
    if q:
        needle = f"%{q}%"
        stmt = stmt.where(or_(User.username.ilike(needle), User.email.ilike(needle)))
    result = await session.execute(stmt)
    return await _admin_users_out(session, list(result.scalars().all()))


@router.patch("/users/{user_id}", response_model=AdminUserOut)
async def update_user(
    user_id: uuid.UUID,
    payload: AdminUserUpdateRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminUserOut:
    target = await session.get(User, user_id)
    if target is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    changes = (payload.platform_role is not None and payload.platform_role != target.platform_role) or (
        payload.is_active is not None and payload.is_active != target.is_active
    )
    note = _require_note(payload.note) if changes else None

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
                payload={"from": target.platform_role.value, "to": payload.platform_role.value, "note": note},
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
                payload={"note": note},
            )
        target.is_active = payload.is_active

    await session.commit()
    await session.refresh(target)
    return (await _admin_users_out(session, [target]))[0]


# Riusa lo stesso limite del self-service (auth.py) come ordine di
# grandezza, ma per-attore invece che per-IP/email: qui il richiedente è un
# admin autenticato, non un anonimo da tenere a bada — la protezione serve
# solo contro un account admin compromesso che spamma reset su molti utenti.
ADMIN_PASSWORD_RESET_RATE_LIMIT = 20
ADMIN_PASSWORD_RESET_RATE_LIMIT_WINDOW_SECONDS = 300


@router.post("/users/{user_id}/reset-password", status_code=status.HTTP_202_ACCEPTED)
async def trigger_password_reset(
    user_id: uuid.UUID,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> None:
    """Reset forzoso della password (CLAUDE.md "lato amministrazione"): innesca
    verso l'utente lo stesso ciclo email del self-service "password
    dimenticata" (`app/domain/password_reset.py::request_password_reset`,
    identica funzione di dominio — nessuna duplicazione di codice email/DB),
    ma avviato dall'admin invece che dall'utente. Nessun rate limit per
    IP/email del richiedente (quello di `POST /auth/password/forgot` è
    pensato per un anonimo che enumera indirizzi, non per un admin
    autenticato che agisce su un utente specifico già noto): solo un limite
    più permissivo per attore, contro un account admin compromesso."""
    target = await session.get(User, user_id)
    if target is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    if not target.is_active:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "L'utente non è attivo.")

    await enforce_rate_limit(
        f"ratelimit:admin-password-reset:actor:{current_user.id}",
        limit=ADMIN_PASSWORD_RESET_RATE_LIMIT,
        window_seconds=ADMIN_PASSWORD_RESET_RATE_LIMIT_WINDOW_SECONDS,
        message="Troppi reset avviati di recente. Riprova tra qualche minuto.",
    )

    await audit.record(
        session,
        action="user.password_reset_triggered",
        actor=current_user,
        target_type="user",
        target_id=target.id,
        request=request,
    )
    await session.commit()
    await request_password_reset(session, target.email)


class ServiceStatus(BaseModel):
    name: str
    status: str  # "ok" | "down" | "unconfigured"
    detail: str | None = None


class AdminOverviewOut(BaseModel):
    users_total: int
    users_new_7d: int
    blogs_total: int
    blogs_suspended: int
    posts_published: int
    queue_pending_comments: int
    queue_posts_in_review: int
    queue_hidden_posts: int
    queue_open_reports: int
    audit_today: int
    services: list[ServiceStatus]
    deployment_mode: str


async def _tcp_check(name: str, host: str, port: int) -> ServiceStatus:
    try:
        _, writer = await asyncio.wait_for(asyncio.open_connection(host, port), timeout=2)
        writer.close()
        return ServiceStatus(name=name, status="ok")
    except Exception as exc:  # noqa: BLE001 — qualunque errore di rete è "giù"
        return ServiceStatus(name=name, status="down", detail=type(exc).__name__)


async def _check_services(session: AsyncSession) -> list[ServiceStatus]:
    """Salute dei servizi di piattaforma (mockup 5d). Controlli leggeri e
    con timeout: `SELECT 1` su Postgres, `PING` su Redis, connessione TCP
    per RabbitMQ e storage S3, `GET /health` sul servizio di moderazione."""
    services: list[ServiceStatus] = []
    try:
        await session.execute(text("SELECT 1"))
        services.append(ServiceStatus(name="postgres", status="ok"))
    except Exception as exc:  # noqa: BLE001
        services.append(ServiceStatus(name="postgres", status="down", detail=type(exc).__name__))
    try:
        await asyncio.wait_for(get_redis().ping(), timeout=2)
        services.append(ServiceStatus(name="redis", status="ok"))
    except Exception as exc:  # noqa: BLE001
        services.append(ServiceStatus(name="redis", status="down", detail=type(exc).__name__))
    services.append(await _tcp_check("rabbitmq", settings.rabbitmq_host, settings.rabbitmq_port))
    if settings.storage_backend == "s3" and settings.s3_endpoint_url:
        url = httpx.URL(settings.s3_endpoint_url)
        services.append(await _tcp_check("storage", url.host, url.port or (443 if url.scheme == "https" else 80)))
    else:
        services.append(ServiceStatus(name="storage", status="ok", detail="localstorage"))
    if settings.moderation_service_url:
        try:
            async with httpx.AsyncClient(timeout=2) as client:
                res = await client.get(f"{settings.moderation_service_url}/health")
            services.append(ServiceStatus(name="moderation", status="ok" if res.status_code == 200 else "down"))
        except Exception as exc:  # noqa: BLE001
            services.append(ServiceStatus(name="moderation", status="down", detail=type(exc).__name__))
    else:
        services.append(ServiceStatus(name="moderation", status="unconfigured"))
    return services


@router.get("/overview", response_model=AdminOverviewOut)
async def admin_overview(
    current_user: User = Depends(require_platform_moderator),
    session: AsyncSession = Depends(get_session),
) -> AdminOverviewOut:
    """Panoramica di piattaforma (todo/UX_REDESIGN.md B1, mockup 5d): KPI,
    code aperte, audit di oggi, salute dei servizi. Un Moderatore vede gli
    stessi numeri (sono aggregati, nessun dato personale)."""
    now = datetime.now(timezone.utc)
    today = now.replace(hour=0, minute=0, second=0, microsecond=0)

    async def count(stmt) -> int:
        return int((await session.execute(stmt)).scalar_one() or 0)

    return AdminOverviewOut(
        users_total=await count(select(func.count()).select_from(User)),
        users_new_7d=await count(select(func.count()).select_from(User).where(User.created_at >= now - timedelta(days=7))),
        blogs_total=await count(select(func.count()).select_from(Blog)),
        blogs_suspended=await count(select(func.count()).select_from(Blog).where(Blog.is_suspended.is_(True))),
        posts_published=await count(
            select(func.count()).select_from(Post).where(Post.status == PostStatus.PUBLISHED, Post.published_at <= now)
        ),
        queue_pending_comments=await count(
            select(func.count()).select_from(Comment).where(Comment.status == CommentStatus.PENDING)
        ),
        queue_posts_in_review=await count(
            select(func.count()).select_from(Post).where(Post.status == PostStatus.PENDING_REVIEW)
        ),
        queue_hidden_posts=await count(select(func.count()).select_from(Post).where(Post.is_hidden.is_(True))),
        queue_open_reports=await count(
            select(func.count()).select_from(ContentReport).where(ContentReport.status == ReportStatus.OPEN)
        ),
        audit_today=await count(select(func.count()).select_from(AuditLog).where(AuditLog.occurred_at >= today)),
        services=await _check_services(session),
        deployment_mode=settings.deployment_mode,
    )


class AdminBlogOut(BaseModel):
    id: uuid.UUID
    slug: str
    title: str
    owner_username: str
    visibility: BlogVisibility
    is_suspended: bool
    is_paused: bool = False
    deleted_at: datetime | None = None
    created_at: datetime
    posts_count: int = 0
    reports_open: int = 0

    model_config = {"from_attributes": True}


def _to_admin_blog_out(blog: Blog, *, posts_count: int = 0, reports_open: int = 0) -> AdminBlogOut:
    return AdminBlogOut(
        id=blog.id,
        slug=blog.slug,
        title=blog.title,
        owner_username=blog.owner.username,
        visibility=blog.visibility,
        is_suspended=blog.is_suspended,
        is_paused=blog.is_paused,
        deleted_at=blog.deleted_at,
        created_at=blog.created_at,
        posts_count=posts_count,
        reports_open=reports_open,
    )


async def _blog_counters(session: AsyncSession, blog_ids: list[uuid.UUID]) -> tuple[dict, dict]:
    if not blog_ids:
        return {}, {}
    posts = dict((await session.execute(select(Post.blog_id, func.count()).where(Post.blog_id.in_(blog_ids)).group_by(Post.blog_id))).all())
    reports = dict(
        (
            await session.execute(
                select(ContentReport.blog_id, func.count())
                .where(ContentReport.blog_id.in_(blog_ids), ContentReport.status == ReportStatus.OPEN)
                .group_by(ContentReport.blog_id)
            )
        ).all()
    )
    return posts, reports


class AdminBlogUpdateRequest(BaseModel):
    is_suspended: bool
    # obbligatoria (B5): finisce nel registro di audit
    note: str | None = None


@router.get("/blogs", response_model=list[AdminBlogOut])
async def list_blogs(
    q: str | None = None,
    visibility: BlogVisibility | None = None,
    state: Literal["active", "suspended", "paused", "deleted", "reported"] | None = None,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AdminBlogOut]:
    """Tutti i blog con contatori (post, segnalazioni aperte). Filtri: `q`
    (slug/titolo/proprietario), `visibility`, `state` (`active`, `suspended`,
    `paused`, `deleted`, `reported` = con segnalazioni aperte)."""
    stmt = select(Blog).options(selectinload(Blog.owner)).order_by(Blog.created_at)
    if q:
        needle = f"%{q}%"
        stmt = stmt.join(User, Blog.owner_id == User.id).where(
            or_(Blog.slug.ilike(needle), Blog.title.ilike(needle), User.username.ilike(needle))
        )
    if visibility is not None:
        stmt = stmt.where(Blog.visibility == visibility)
    if state == "suspended":
        stmt = stmt.where(Blog.is_suspended.is_(True))
    elif state == "paused":
        stmt = stmt.where(Blog.is_paused.is_(True))
    elif state == "deleted":
        stmt = stmt.where(Blog.deleted_at.is_not(None))
    elif state == "active":
        stmt = stmt.where(Blog.is_suspended.is_(False), Blog.is_paused.is_(False), Blog.deleted_at.is_(None))
    elif state == "reported":
        stmt = stmt.where(
            Blog.id.in_(select(ContentReport.blog_id).where(ContentReport.status == ReportStatus.OPEN))
        )
    blogs = list((await session.execute(stmt)).scalars().all())
    posts, reports = await _blog_counters(session, [b.id for b in blogs])
    if state == "reported":
        blogs.sort(key=lambda b: -int(reports.get(b.id, 0)))
    return [_to_admin_blog_out(b, posts_count=int(posts.get(b.id, 0)), reports_open=int(reports.get(b.id, 0))) for b in blogs]


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

    was_changed = payload.is_suspended != blog.is_suspended
    if was_changed:
        note = _require_note(payload.note)
        await audit.record(
            session,
            action="blog.suspended" if payload.is_suspended else "blog.unsuspended",
            actor=current_user,
            target_type="blog",
            target_id=blog.id,
            blog_id=blog.id,
            request=request,
            payload={"slug": blog.slug, "blog_alias": blog.default_author_display_name, "note": note},
        )
    blog.is_suspended = payload.is_suspended
    await session.commit()
    await session.refresh(blog, attribute_names=["owner"])
    if was_changed:
        # la sospensione blocca lettura/scrittura pubbliche del blog: tutte le
        # sue pagine e la homepage vanno rigenerate.
        await revalidate_frontend([blog_tag(blog.slug), feed_tag()])
    posts, reports = await _blog_counters(session, [blog.id])
    return _to_admin_blog_out(blog, posts_count=int(posts.get(blog.id, 0)), reports_open=int(reports.get(blog.id, 0)))


class ReportDetailOut(BaseModel):
    id: uuid.UUID
    target_type: ReportTargetType
    target_id: uuid.UUID
    post_slug: str | None
    post_title: str | None
    reason: str
    note: str | None
    reporter_username: str
    created_at: datetime


class BlogReportsOut(BaseModel):
    blog: AdminBlogOut
    owner_mfa_enabled: bool
    owner_email_domain: str
    reports: list[ReportDetailOut]


@router.get("/blogs/{blog_id}/reports", response_model=BlogReportsOut)
async def blog_reports(
    blog_id: uuid.UUID,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> BlogReportsOut:
    """Pannello segnalazioni di un blog (mockup 5e): segnalazioni aperte sul
    blog e sui suoi post, con motivo, nota e chi ha segnalato."""
    blog = (await session.execute(select(Blog).options(selectinload(Blog.owner)).where(Blog.id == blog_id))).scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    rows = (
        await session.execute(
            select(ContentReport, User.username, Post.slug, Post.title)
            .join(User, User.id == ContentReport.reporter_id)
            .outerjoin(Post, (ContentReport.target_type == ReportTargetType.POST) & (Post.id == ContentReport.target_id))
            .where(ContentReport.blog_id == blog.id, ContentReport.status == ReportStatus.OPEN)
            .order_by(ContentReport.created_at.desc())
        )
    ).all()
    posts, reports = await _blog_counters(session, [blog.id])
    return BlogReportsOut(
        blog=_to_admin_blog_out(blog, posts_count=int(posts.get(blog.id, 0)), reports_open=int(reports.get(blog.id, 0))),
        owner_mfa_enabled=blog.owner.mfa_enabled,
        owner_email_domain=blog.owner.email.rsplit("@", 1)[-1],
        reports=[
            ReportDetailOut(
                id=r.id,
                target_type=r.target_type,
                target_id=r.target_id,
                post_slug=post_slug,
                post_title=post_title,
                reason=r.reason.value,
                note=r.note,
                reporter_username=username,
                created_at=r.created_at,
            )
            for r, username, post_slug, post_title in rows
        ],
    )


class BlogActionRequest(BaseModel):
    action: Literal["suspend", "restore", "hide_reported_posts", "deactivate_owner", "dismiss"]
    note: str


@router.post("/blogs/{blog_id}/action", response_model=AdminBlogOut)
async def blog_action(
    blog_id: uuid.UUID,
    payload: BlogActionRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminBlogOut:
    """Azione dal pannello segnalazioni (mockup 5e) con nota obbligatoria:
    `suspend`/`restore` il blog, `hide_reported_posts` (solo i post con
    segnalazioni aperte), `deactivate_owner` (l'account del proprietario,
    solo super admin se il proprietario è admin), `dismiss` (archivia). In
    tutti i casi le segnalazioni aperte del blog vengono chiuse
    (`actioned`/`dismissed`) e l'azione finisce nel registro con la nota."""
    note = _require_note(payload.note)
    blog = (await session.execute(select(Blog).options(selectinload(Blog.owner)).where(Blog.id == blog_id))).scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    tags = [blog_tag(blog.slug), feed_tag()]
    audit_payload: dict = {"slug": blog.slug, "note": note, "action": payload.action}

    if payload.action == "suspend":
        blog.is_suspended = True
        action = "blog.suspended"
    elif payload.action == "restore":
        blog.is_suspended = False
        action = "blog.unsuspended"
    elif payload.action == "hide_reported_posts":
        reported_post_ids = select(ContentReport.target_id).where(
            ContentReport.blog_id == blog.id,
            ContentReport.target_type == ReportTargetType.POST,
            ContentReport.status == ReportStatus.OPEN,
        )
        posts = (await session.execute(select(Post).where(Post.id.in_(reported_post_ids)))).scalars().all()
        for post in posts:
            post.is_hidden = True
            tags.append(post_tag(blog.slug, post.slug))
        audit_payload["hidden_posts"] = [p.slug for p in posts]
        action = "blog.reported_posts_hidden"
    elif payload.action == "deactivate_owner":
        owner = blog.owner
        if owner.platform_role in PRIVILEGED_ROLES and current_user.platform_role != PlatformRole.SUPER_ADMIN:
            raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo un Super Admin può disattivare un amministratore.")
        if owner.id == current_user.id:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi disattivare il tuo stesso account.")
        owner.is_active = False
        blog.is_suspended = True
        audit_payload["owner"] = owner.username
        action = "user.deactivated"
    else:
        action = "blog.reports_dismissed"

    new_status = ReportStatus.DISMISSED if payload.action == "dismiss" else ReportStatus.ACTIONED
    await session.execute(
        sa_update(ContentReport)
        .where(ContentReport.blog_id == blog.id, ContentReport.status == ReportStatus.OPEN)
        .values(status=new_status, resolved_at=datetime.now(timezone.utc), resolved_by_id=current_user.id)
    )
    await audit.record(
        session,
        action=action,
        actor=current_user,
        target_type="blog",
        target_id=blog.id,
        blog_id=blog.id,
        request=request,
        payload=audit_payload,
    )
    await session.commit()
    await session.refresh(blog, attribute_names=["owner"])
    if payload.action != "dismiss":
        await revalidate_frontend(tags)
    posts_c, reports_c = await _blog_counters(session, [blog.id])
    return _to_admin_blog_out(blog, posts_count=int(posts_c.get(blog.id, 0)), reports_open=int(reports_c.get(blog.id, 0)))


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
    reports_open: int = 0

    model_config = {"from_attributes": True}


def _to_admin_post_out(post: Post, blog: Blog, author: User, reports_open: int = 0) -> AdminPostOut:
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
        reports_open=reports_open,
    )


class AdminPostUpdateRequest(BaseModel):
    is_hidden: bool
    # obbligatoria (B5): finisce nel registro di audit
    note: str | None = None


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
    rows = (await session.execute(stmt)).all()
    reports: dict = {}
    if rows:
        reports = dict(
            (
                await session.execute(
                    select(ContentReport.target_id, func.count())
                    .where(
                        ContentReport.target_type == ReportTargetType.POST,
                        ContentReport.status == ReportStatus.OPEN,
                        ContentReport.target_id.in_([p.id for p, _b, _a in rows]),
                    )
                    .group_by(ContentReport.target_id)
                )
            ).all()
        )
    return [_to_admin_post_out(post, blog, author, int(reports.get(post.id, 0))) for post, blog, author in rows]


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

    was_changed = payload.is_hidden != post.is_hidden
    if was_changed:
        note = _require_note(payload.note)
        await audit.record(
            session,
            action="post.hidden" if payload.is_hidden else "post.unhidden",
            actor=current_user,
            target_type="post",
            target_id=post.id,
            blog_id=post.blog_id,
            request=request,
            payload={"slug": post.slug, "blog_slug": blog.slug, "blog_alias": blog.default_author_display_name, "note": note},
        )
    post.is_hidden = payload.is_hidden
    await session.commit()
    await session.refresh(post)
    if was_changed:
        await revalidate_frontend(
            [post_tag(blog.slug, post.slug), blog_tag(blog.slug), feed_tag()]
        )
    return _to_admin_post_out(post, blog, author)


AuditChannel = Literal["web", "api", "system"]

# Canale da cui è partita l'azione, derivato da actor_type (nessuna colonna
# dedicata: la mappa è deterministica e stabile, vedi app/domain/audit.py —
# `user`/`anonymous` passano sempre da una richiesta HTTP autenticata via
# sessione o non ancora autenticata (login), mai da un ApiToken; `core_token`/
# `user_token` sono sempre un accesso diretto via token opaco; `system` un
# processo interno senza richiesta HTTP (bootstrap, job schedulati).
_CHANNEL_BY_ACTOR_TYPE: dict[AuditActorType, AuditChannel] = {
    AuditActorType.USER: "web",
    AuditActorType.ANONYMOUS: "web",
    AuditActorType.CORE_TOKEN: "api",
    AuditActorType.USER_TOKEN: "api",
    AuditActorType.SYSTEM: "system",
}
_ACTOR_TYPES_BY_CHANNEL: dict[AuditChannel, list[AuditActorType]] = {
    "web": [AuditActorType.USER, AuditActorType.ANONYMOUS],
    "api": [AuditActorType.CORE_TOKEN, AuditActorType.USER_TOKEN],
    "system": [AuditActorType.SYSTEM],
}


class AuditLogOut(BaseModel):
    id: uuid.UUID
    occurred_at: datetime
    actor_type: AuditActorType
    # calcolato da actor_type dopo la validazione, non una colonna a sé:
    # vedi _CHANNEL_BY_ACTOR_TYPE sopra. Il default è solo un placeholder,
    # sempre sovrascritto dal model_validator qui sotto.
    channel: AuditChannel = "system"
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

    @model_validator(mode="after")
    def _compute_channel(self) -> "AuditLogOut":
        self.channel = _CHANNEL_BY_ACTOR_TYPE[self.actor_type]
        return self


@router.get("/audit-log", response_model=list[AuditLogOut])
async def list_audit_log(
    action: str | None = None,
    actor_id: uuid.UUID | None = None,
    target_id: uuid.UUID | None = None,
    blog_id: uuid.UUID | None = None,
    channel: AuditChannel | None = None,
    since: datetime | None = None,
    until: datetime | None = None,
    limit: int = 100,
    offset: int = 0,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[AuditLog]:
    """Consultazione del registro di audit (append-only). Filtri opzionali per
    azione, attore, oggetto, blog, canale (web/api/system, vedi
    _CHANNEL_BY_ACTOR_TYPE) e intervallo temporale; ordine dal più recente.
    Gli eventi oltre la retention non sono qui ma negli archivi su storage
    (vedi `app/workers/audit_maintenance.py`)."""
    stmt = select(AuditLog).order_by(AuditLog.occurred_at.desc())
    if action is not None:
        stmt = stmt.where(AuditLog.action == action)
    if actor_id is not None:
        stmt = stmt.where(AuditLog.actor_id == actor_id)
    if target_id is not None:
        stmt = stmt.where(AuditLog.target_id == target_id)
    if blog_id is not None:
        stmt = stmt.where(AuditLog.blog_id == blog_id)
    if channel is not None:
        stmt = stmt.where(AuditLog.actor_type.in_(_ACTOR_TYPES_BY_CHANNEL[channel]))
    if since is not None:
        stmt = stmt.where(AuditLog.occurred_at >= since)
    if until is not None:
        stmt = stmt.where(AuditLog.occurred_at < until)
    stmt = stmt.limit(min(max(limit, 1), 500)).offset(max(offset, 0))
    result = await session.execute(stmt)
    return list(result.scalars().all())


class AdminCommentOut(BaseModel):
    """Come `BlogCommentOut` (`app/api/v1/comments.py`, moderazione per-blog),
    con anche blog di appartenenza: qui i commenti attraversano *tutti* i
    blog della piattaforma (ROADMAP.md §1 — pannello di moderazione
    trasversale, finora mancante)."""

    id: uuid.UUID
    post_id: uuid.UUID
    post_title: str
    post_slug: str
    blog_id: uuid.UUID
    blog_slug: str
    blog_title: str
    author_id: uuid.UUID | None
    author_display_name: str
    status: CommentStatus
    content: str
    created_at: datetime
    reported_to_platform: bool = False
    report_note: str | None = None
    reported_at: datetime | None = None

    model_config = {"from_attributes": True}


@router.get("/comments", response_model=list[AdminCommentOut])
async def list_all_comments(
    status_filter: CommentStatus = Query(CommentStatus.PENDING, alias="status"),
    reported: bool = False,
    q: str | None = None,
    current_user: User = Depends(require_platform_moderator),
    session: AsyncSession = Depends(get_session),
) -> list[AdminCommentOut]:
    """Commenti di *tutti* i blog della piattaforma nello stato indicato
    (default `pending`), dal più recente. Riservato ad Amministratore/Super
    Admin/Moderatore (`require_platform_moderator`) — a differenza di
    `GET /api/v1/blogs/{slug}/comments`, che resta per-blog ad opera del
    proprietario/mediatore di quel singolo blog."""
    stmt = (
        select(Comment, Post.title, Post.slug, Blog.id, Blog.slug, Blog.title)
        .join(Post, Post.id == Comment.post_id)
        .join(Blog, Blog.id == Post.blog_id)
        .where(Comment.reported_to_platform.is_(True) if reported else (Comment.status == status_filter))
        .order_by(Comment.reported_at.desc().nulls_last() if reported else Comment.created_at.desc())
    )
    if q:
        needle = f"%{q}%"
        stmt = stmt.where(
            or_(
                Comment.content.ilike(needle),
                Comment.author_display_name.ilike(needle),
                Post.title.ilike(needle),
                Blog.slug.ilike(needle),
            )
        )
    rows = (await session.execute(stmt)).all()

    # display name degli autori registrati ricalcolato in blocco, stesso
    # principio di list_blog_comments (un solo SELECT invece di uno per commento).
    author_ids = {c.author_id for c, *_ in rows if c.author_id is not None}
    authors: dict[uuid.UUID, User] = {}
    if author_ids:
        res = await session.execute(select(User).where(User.id.in_(author_ids)))
        authors = {u.id: u for u in res.scalars()}

    out: list[AdminCommentOut] = []
    for comment, post_title, post_slug, blog_id, blog_slug, blog_title in rows:
        display_name = comment.author_display_name
        author = authors.get(comment.author_id) if comment.author_id is not None else None
        if author is not None:
            display_name = resolve_personal_display_name(author)
        out.append(
            AdminCommentOut(
                id=comment.id,
                post_id=comment.post_id,
                post_title=post_title,
                post_slug=post_slug,
                blog_id=blog_id,
                blog_slug=blog_slug,
                blog_title=blog_title,
                author_id=comment.author_id,
                author_display_name=display_name,
                status=comment.status,
                content=comment.content,
                created_at=comment.created_at,
                reported_to_platform=comment.reported_to_platform,
                report_note=comment.report_note,
                reported_at=comment.reported_at,
            )
        )
    return out


# ---------------------------------------------------------------------------
# Impostazioni di piattaforma (todo/UX_REDESIGN.md B6, mockup 5f)
# ---------------------------------------------------------------------------


class PlatformConfigOut(BaseModel):
    default_locale: str
    registration_mode: str
    sso_providers: list[str]
    sso_configured: list[str]
    mfa_required_for_admins: bool
    reserved_blog_names: list[str]
    reserved_builtin: list[str]
    moderation_threshold: float
    max_blogs_per_user: int
    anonymous_comments_allowed: bool
    audit_retention_days: int
    # Footer di piattaforma (mostrato anche su ogni blog, con override
    # possibile solo per le colonne 1/2 — vedi Blog): null = colonna/barra
    # vuota, non mostrata.
    footer_column1_markdown: str | None
    footer_column2_markdown: str | None
    footer_column3_markdown: str | None
    footer_bottom_bar_markdown: str | None
    interests: list[dict]
    # Sigilli di verifica manuali/dominio (app/domain/verification.py):
    # max_blog_storage_mb null = nessun limite.
    max_blog_storage_mb: int | None
    verification_gold_identifiers: list[str]
    verification_silver_identifiers: list[str]
    verification_blue_domains: list[str]
    updated_at: datetime | None
    infrastructure: dict[str, str | bool | None]


class PlatformConfigUpdateRequest(BaseModel):
    default_locale: str | None = None
    registration_mode: str | None = None
    sso_providers: list[str] | None = None
    mfa_required_for_admins: bool | None = None
    reserved_blog_names: list[str] | None = None
    moderation_threshold: float | None = None
    max_blogs_per_user: int | None = None
    anonymous_comments_allowed: bool | None = None
    audit_retention_days: int | None = None
    # "" azzera (colonna/barra vuota), assente non tocca — stesso schema di
    # BlogUpdateRequest.subtitle.
    footer_column1_markdown: str | None = None
    footer_column2_markdown: str | None = None
    footer_column3_markdown: str | None = None
    footer_bottom_bar_markdown: str | None = None
    # Elenco completo (sostituisce, non aggiunge — a differenza di
    # reserved_blog_names non c'è un builtin da preservare separatamente):
    # ogni voce `{"key": "...", "translations": {"it": "...", ...}}`,
    # validata da app/domain/interests.py::validate_interest_list.
    interests: list[dict] | None = None
    # 0 = nessun limite (azzera), assente = non tocca; un valore positivo
    # imposta il limite in MB. Stesso schema "0 = azzera" per non introdurre
    # un tri-state solo per questo campo (nessun limite negativo o a 0 ha
    # senso come valore reale).
    max_blog_storage_mb: int | None = None
    # Elenchi completi (sostituiscono, non aggiungono): email/username per
    # GOLD e SILVER, domini per BLUE — vedi app/domain/verification.py.
    verification_gold_identifiers: list[str] | None = None
    verification_silver_identifiers: list[str] | None = None
    verification_blue_domains: list[str] | None = None


def _config_out(config: PlatformConfig) -> PlatformConfigOut:
    return PlatformConfigOut(
        default_locale=config.default_locale,
        registration_mode=config.registration_mode,
        sso_providers=list(config.sso_providers),
        sso_configured=sorted(configured_providers()),
        mfa_required_for_admins=config.mfa_required_for_admins,
        reserved_blog_names=list(config.reserved_blog_names),
        reserved_builtin=sorted(RESERVED_BLOG_SLUGS),
        moderation_threshold=config.moderation_threshold,
        max_blogs_per_user=config.max_blogs_per_user,
        anonymous_comments_allowed=config.anonymous_comments_allowed,
        audit_retention_days=config.audit_retention_days,
        footer_column1_markdown=config.footer_column1_markdown,
        footer_column2_markdown=config.footer_column2_markdown,
        footer_column3_markdown=config.footer_column3_markdown,
        footer_bottom_bar_markdown=config.footer_bottom_bar_markdown,
        interests=list(config.interests),
        max_blog_storage_mb=config.max_blog_storage_mb,
        verification_gold_identifiers=list(config.verification_gold_identifiers),
        verification_silver_identifiers=list(config.verification_silver_identifiers),
        verification_blue_domains=list(config.verification_blue_domains),
        updated_at=config.updated_at,
        # sola lettura: riepilogo dell'ambiente NOCT_* (mai segreti). La
        # retention dell'audit log non ci sta più: da B6+ è configurabile a
        # runtime (vedi audit_retention_days sopra), l'env resta solo il
        # seme iniziale (app/domain/platform_config.py::get_platform_config).
        infrastructure={
            "deployment_mode": settings.deployment_mode,
            "instance_fqdn": settings.instance_fqdn,
            "storage_backend": settings.storage_backend,
            "s3_endpoint_url": settings.s3_endpoint_url if settings.storage_backend == "s3" else None,
            "moderation_service": bool(settings.moderation_service_url),
            "turnstile": bool(settings.turnstile_site_key),
            "smtp": bool(settings.smtp_host),
        },
    )


@router.get("/config", response_model=PlatformConfigOut)
async def get_admin_config(
    current_user: User = Depends(require_super_admin),
    session: AsyncSession = Depends(get_session),
) -> PlatformConfigOut:
    return _config_out(await get_platform_config(session))


@router.patch("/config", response_model=PlatformConfigOut)
async def update_admin_config(
    payload: PlatformConfigUpdateRequest,
    request: Request,
    current_user: User = Depends(require_super_admin),
    session: AsyncSession = Depends(get_session),
) -> PlatformConfigOut:
    """Solo Super Admin; ogni modifica finisce nel registro di audit con i
    valori precedenti e nuovi (`platform.config_updated`)."""
    config = await get_platform_config(session)
    changes: dict[str, dict] = {}

    def apply(field: str, value) -> None:
        old = getattr(config, field)
        if old != value:
            changes[field] = {"from": old, "to": value}
            setattr(config, field, value)

    if payload.default_locale is not None:
        if payload.default_locale not in SUPPORTED_LOCALES:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Lingua non supportata.")
        apply("default_locale", payload.default_locale)
    if payload.registration_mode is not None:
        if payload.registration_mode not in REGISTRATION_MODES:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Modalità di registrazione non valida.")
        apply("registration_mode", payload.registration_mode)
    if payload.sso_providers is not None:
        unknown = set(payload.sso_providers) - set(SSO_PROVIDER_NAMES)
        if unknown:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Provider sconosciuti: {', '.join(sorted(unknown))}.")
        apply("sso_providers", sorted(set(payload.sso_providers)))
    if payload.mfa_required_for_admins is not None:
        apply("mfa_required_for_admins", payload.mfa_required_for_admins)
    if payload.reserved_blog_names is not None:
        cleaned = sorted({n.strip().lower() for n in payload.reserved_blog_names if n.strip()})
        bad = [n for n in cleaned if not SLUG_PATTERN.fullmatch(n)]
        if bad:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Nomi non validi: {', '.join(bad)}.")
        apply("reserved_blog_names", cleaned)
    if payload.moderation_threshold is not None:
        if not 0 < payload.moderation_threshold <= 1:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "La soglia deve essere tra 0 e 1.")
        apply("moderation_threshold", payload.moderation_threshold)
    if payload.max_blogs_per_user is not None:
        if not 1 <= payload.max_blogs_per_user <= 100:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il massimo di blog per utente deve essere tra 1 e 100.")
        apply("max_blogs_per_user", payload.max_blogs_per_user)
    if payload.anonymous_comments_allowed is not None:
        apply("anonymous_comments_allowed", payload.anonymous_comments_allowed)
    if payload.audit_retention_days is not None:
        if not MIN_AUDIT_RETENTION_DAYS <= payload.audit_retention_days <= MAX_AUDIT_RETENTION_DAYS:
            raise HTTPException(
                status.HTTP_400_BAD_REQUEST,
                f"La conservazione del registro di audit deve essere tra {MIN_AUDIT_RETENTION_DAYS} e {MAX_AUDIT_RETENTION_DAYS} giorni.",
            )
        apply("audit_retention_days", payload.audit_retention_days)
    for field in ("footer_column1_markdown", "footer_column2_markdown", "footer_column3_markdown", "footer_bottom_bar_markdown"):
        value = getattr(payload, field)
        if value is not None:
            if len(value) > MAX_FOOTER_MARKDOWN_LENGTH:
                raise HTTPException(
                    status.HTTP_400_BAD_REQUEST,
                    f"Il testo del footer supera i {MAX_FOOTER_MARKDOWN_LENGTH} caratteri.",
                )
            apply(field, value or None)
    removed_interest_keys: set[str] = set()
    if payload.interests is not None:
        try:
            cleaned_interests = validate_interest_list(payload.interests)
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        removed_interest_keys = {i["key"] for i in config.interests} - {i["key"] for i in cleaned_interests}
        apply("interests", cleaned_interests)

    if payload.max_blog_storage_mb is not None:
        if payload.max_blog_storage_mb == 0:
            apply("max_blog_storage_mb", None)
        elif not 1 <= payload.max_blog_storage_mb <= 1_048_576:
            raise HTTPException(
                status.HTTP_400_BAD_REQUEST,
                "Lo spazio massimo per blog deve essere tra 1 MB e 1.048.576 MB (1 TB), oppure 0 per nessun limite.",
            )
        else:
            apply("max_blog_storage_mb", payload.max_blog_storage_mb)
    if payload.verification_gold_identifiers is not None:
        apply("verification_gold_identifiers", sorted({v.strip().lower() for v in payload.verification_gold_identifiers if v.strip()}))
    if payload.verification_silver_identifiers is not None:
        apply(
            "verification_silver_identifiers",
            sorted({v.strip().lower() for v in payload.verification_silver_identifiers if v.strip()}),
        )
    if payload.verification_blue_domains is not None:
        cleaned_domains = sorted({v.strip().lower() for v in payload.verification_blue_domains if v.strip()})
        bad_domains = [d for d in cleaned_domains if not DOMAIN_PATTERN.match(d)]
        if bad_domains:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Domini non validi: {', '.join(bad_domains)}.")
        apply("verification_blue_domains", cleaned_domains)

    if removed_interest_keys:
        # Le chiavi rimosse dall'elenco di piattaforma non sono più valide per
        # PATCH /users/me (validate_user_interest_keys), ma le selezioni già
        # salvate su User.interests non vengono toccate da sole: andrebbero
        # orfane (una chiave che non esiste più in nessuna traduzione).
        # User.interests è un ARRAY "core" (sqlalchemy.ARRAY, non il tipo
        # dialect-specific postgresql.ARRAY): niente comparator .overlap(),
        # serve l'operatore "&&" esplicito.
        affected = await session.execute(
            select(User).where(User.interests.op("&&")(list(removed_interest_keys)))
        )
        for user in affected.scalars():
            user.interests = [k for k in user.interests if k not in removed_interest_keys]

    if {"verification_gold_identifiers", "verification_silver_identifiers", "verification_blue_domains"} & changes.keys():
        # Un elenco può aggiungere o togliere chi ha diritto a GOLD/SILVER/
        # BLU: a differenza degli interessi sopra (dove basta ripulire chi
        # perde una chiave), qui serve ricalcolare da zero il tier di ogni
        # utente attivo, perché una voce rimossa da GOLD potrebbe comunque
        # avere diritto a SILVER/BLU/BRONZE (app/domain/verification.py).
        all_active_users = (await session.execute(select(User).where(User.is_active.is_(True)))).scalars().all()
        for user in all_active_users:
            sync_verification_tier(user, config)

    if changes:
        touch(config, by_id=current_user.id)
        await audit.record(
            session, action="platform.config_updated", actor=current_user, target_type="platform", request=request, payload={"changes": changes}
        )
        await session.commit()
        await session.refresh(config)
        if any(f.startswith("footer_") for f in changes):
            # mostrato su ogni pagina pubblica di piattaforma e di ogni blog:
            # un solo tag condiviso, invalida tutto in un colpo.
            await revalidate_frontend([platform_footer_tag()])
    return _config_out(config)


# ---------------------------------------------------------------------------
# Coda richieste GDPR (B6, mockup 5f)
# ---------------------------------------------------------------------------


class GdprRequestOut(BaseModel):
    id: uuid.UUID
    username: str
    type: GdprRequestType
    status: GdprRequestStatus
    deadline_at: datetime
    note: str | None
    created_by_username: str | None
    approved_by_username: str | None
    approved_at: datetime | None
    completed_at: datetime | None
    created_at: datetime


class GdprRequestCreate(BaseModel):
    username: str
    type: GdprRequestType
    note: str


async def _gdpr_out(session: AsyncSession, rows: list[GdprRequest]) -> list[GdprRequestOut]:
    ids = {r.created_by_id for r in rows if r.created_by_id} | {r.approved_by_id for r in rows if r.approved_by_id}
    names: dict[uuid.UUID, str] = {}
    if ids:
        names = dict((await session.execute(select(User.id, User.username).where(User.id.in_(ids)))).all())
    return [
        GdprRequestOut(
            id=r.id,
            username=r.username,
            type=r.type,
            status=r.status,
            deadline_at=r.deadline_at,
            note=r.note,
            created_by_username=names.get(r.created_by_id) if r.created_by_id else None,
            approved_by_username=names.get(r.approved_by_id) if r.approved_by_id else None,
            approved_at=r.approved_at,
            completed_at=r.completed_at,
            created_at=r.created_at,
        )
        for r in rows
    ]


@router.get("/gdpr", response_model=list[GdprRequestOut])
async def list_gdpr_requests(
    status_filter: GdprRequestStatus | None = Query(None, alias="status"),
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[GdprRequestOut]:
    """Registro delle richieste GDPR: quelle self-service (già completate)
    e quelle inserite dagli admin per richieste arrivate fuori banda. Aperte
    prima, per scadenza."""
    stmt = select(GdprRequest).order_by(GdprRequest.deadline_at)
    if status_filter is not None:
        stmt = stmt.where(GdprRequest.status == status_filter)
    rows = list((await session.execute(stmt)).scalars().all())
    rows.sort(key=lambda r: (r.status != GdprRequestStatus.OPEN, r.status != GdprRequestStatus.APPROVED, r.deadline_at))
    return await _gdpr_out(session, rows)


@router.post("/gdpr", response_model=GdprRequestOut, status_code=status.HTTP_201_CREATED)
async def create_gdpr_request(
    payload: GdprRequestCreate,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> GdprRequestOut:
    """Inserisce una richiesta arrivata fuori banda (email, PEC) a nome di un
    utente; la nota (chi/come ha chiesto) è obbligatoria e va nel registro."""
    note = _require_note(payload.note)
    user = (await session.execute(select(User).where(User.username == payload.username))).scalar_one_or_none()
    if user is None or not user.is_active:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    req = new_request(user=user, type_=payload.type, note=note, created_by_id=current_user.id)
    session.add(req)
    await audit.record(
        session, action="gdpr.request_created", actor=current_user, target_type="user", target_id=user.id, request=request, payload={"type": payload.type.value, "note": note}
    )
    await session.commit()
    await session.refresh(req)
    return (await _gdpr_out(session, [req]))[0]


async def _get_request(session: AsyncSession, request_id: uuid.UUID) -> GdprRequest:
    req = await session.get(GdprRequest, request_id)
    if req is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Richiesta non trovata.")
    return req


@router.post("/gdpr/{request_id}/approve", response_model=GdprRequestOut)
async def approve_gdpr_request(
    request_id: uuid.UUID,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> GdprRequestOut:
    """Seconda approvazione (mockup 5f "deletions need a second admin"): chi
    approva deve essere un admin diverso da chi ha inserito la richiesta.
    Un export non ha bisogno di approvazione: passa direttamente a `execute`."""
    req = await _get_request(session, request_id)
    if req.status != GdprRequestStatus.OPEN:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "La richiesta non è aperta.")
    if req.type == GdprRequestType.DELETION and req.created_by_id == current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Una cancellazione va approvata da un secondo amministratore.")
    req.status = GdprRequestStatus.APPROVED
    req.approved_by_id = current_user.id
    req.approved_at = datetime.now(timezone.utc)
    await audit.record(
        session, action="gdpr.request_approved", actor=current_user, target_type="user", target_id=req.user_id, request=request, payload={"type": req.type.value}
    )
    await session.commit()
    await session.refresh(req)
    return (await _gdpr_out(session, [req]))[0]


class GdprRejectRequest(BaseModel):
    note: str


@router.post("/gdpr/{request_id}/reject", response_model=GdprRequestOut)
async def reject_gdpr_request(
    request_id: uuid.UUID,
    payload: GdprRejectRequest,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> GdprRequestOut:
    """Rifiuto motivato (nota obbligatoria, nel registro)."""
    req = await _get_request(session, request_id)
    if req.status in (GdprRequestStatus.COMPLETED, GdprRequestStatus.REJECTED):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "La richiesta è già chiusa.")
    note = _require_note(payload.note)
    req.status = GdprRequestStatus.REJECTED
    req.completed_at = datetime.now(timezone.utc)
    req.note = f"{req.note or ''}\n[rifiutata] {note}".strip()
    await audit.record(
        session, action="gdpr.request_rejected", actor=current_user, target_type="user", target_id=req.user_id, request=request, payload={"type": req.type.value, "note": note}
    )
    await session.commit()
    await session.refresh(req)
    return (await _gdpr_out(session, [req]))[0]


@router.post("/gdpr/{request_id}/execute")
async def execute_gdpr_request(
    request_id: uuid.UUID,
    request: Request,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
):
    """Esegue la richiesta: un **export** (aperto o approvato) restituisce il
    JSON dei dati dell'utente (stesso formato di `GET /users/me/export-data`)
    e chiude la richiesta; una **cancellazione** deve essere `approved` da un
    secondo admin e anonimizza l'account (stessa procedura di
    `DELETE /users/me`)."""
    req = await _get_request(session, request_id)
    user = await session.get(User, req.user_id)
    if user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    if req.type == GdprRequestType.EXPORT:
        if req.status not in (GdprRequestStatus.OPEN, GdprRequestStatus.APPROVED):
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "La richiesta è già chiusa.")
        data = await export_user_data(session, user)
        req.status = GdprRequestStatus.COMPLETED
        req.completed_at = datetime.now(timezone.utc)
        await audit.record(
            session, action="gdpr.export_executed", actor=current_user, target_type="user", target_id=user.id, request=request
        )
        await session.commit()
        return data
    if req.status != GdprRequestStatus.APPROVED:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "La cancellazione richiede prima l'approvazione di un secondo amministratore.")
    if user.platform_role == PlatformRole.SUPER_ADMIN:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non è possibile cancellare un Super Admin da qui.")
    await audit.record(
        session, action="gdpr.deletion_executed", actor=current_user, target_type="user", target_id=user.id, request=request, payload={"username": user.username}
    )
    req.status = GdprRequestStatus.COMPLETED
    req.completed_at = datetime.now(timezone.utc)
    await anonymize_and_deactivate_user(session, user)
    await session.commit()
    return {"status": "completed"}
