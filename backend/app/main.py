from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from starlette.middleware.sessions import SessionMiddleware

from app.api.v1.admin import router as admin_router
from app.api.v1.auth import router as auth_router
from app.api.v1.blogs import router as blogs_router
from app.api.v1.comments import router as comments_router
from app.api.v1.feed import router as feed_router
from app.api.v1.fragments import router as fragments_router
from app.api.v1.health import router as health_router
from app.api.v1.link_preview import router as link_preview_router
from app.api.v1.pages import router as pages_router
from app.api.v1.posts import router as posts_router
from app.api.v1.tokens import router as tokens_router
from app.api.v1.users import router as users_router
from app.core.config import settings
from app.core.database import SessionLocal
from app.domain.auth import bootstrap_super_admin


@asynccontextmanager
async def lifespan(app: FastAPI):
    async with SessionLocal() as session:
        await bootstrap_super_admin(session)
    yield


app = FastAPI(title="Notturni API", lifespan=lifespan)

# richiesto da Authlib per il flow OAuth2/OIDC (state/nonce firmati in sessione)
app.add_middleware(SessionMiddleware, secret_key=settings.session_secret)

# il frontend (origine diversa: altra porta in dev, altro sottodominio in
# produzione) chiama l'API direttamente dal browser
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.cors_allowed_origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(health_router, prefix="/api/v1", tags=["health"])
app.include_router(tokens_router, prefix="/api/v1/tokens", tags=["tokens"])
app.include_router(auth_router, prefix="/api/v1/auth", tags=["auth"])
app.include_router(blogs_router, prefix="/api/v1/blogs", tags=["blogs"])
app.include_router(posts_router, prefix="/api/v1", tags=["posts"])
app.include_router(feed_router, prefix="/api/v1/feed", tags=["feed"])
app.include_router(comments_router, prefix="/api/v1", tags=["comments"])
app.include_router(fragments_router, prefix="/api/v1", tags=["fragments"])
app.include_router(pages_router, prefix="/api/v1/pages", tags=["pages"])
app.include_router(users_router, prefix="/api/v1/users", tags=["users"])
app.include_router(admin_router, prefix="/api/v1/admin", tags=["admin"])
app.include_router(link_preview_router, prefix="/api/v1/link-preview", tags=["link-preview"])
