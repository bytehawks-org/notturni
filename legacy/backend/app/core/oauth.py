from authlib.integrations.starlette_client import OAuth

from app.core.config import settings

oauth = OAuth()

_PROVIDERS_METADATA: dict[str, dict] = {
    "google": {
        "client_id": settings.oauth_google_client_id,
        "client_secret": settings.oauth_google_client_secret,
        "server_metadata_url": "https://accounts.google.com/.well-known/openid-configuration",
        "client_kwargs": {"scope": "openid email profile"},
    },
    "microsoft": {
        "client_id": settings.oauth_microsoft_client_id,
        "client_secret": settings.oauth_microsoft_client_secret,
        "server_metadata_url": (
            "https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration"
        ),
        "client_kwargs": {"scope": "openid email profile"},
    },
    "github": {
        "client_id": settings.oauth_github_client_id,
        "client_secret": settings.oauth_github_client_secret,
        "access_token_url": "https://github.com/login/oauth/access_token",
        "authorize_url": "https://github.com/login/oauth/authorize",
        "api_base_url": "https://api.github.com/",
        "client_kwargs": {"scope": "read:user user:email"},
    },
    "linkedin": {
        "client_id": settings.oauth_linkedin_client_id,
        "client_secret": settings.oauth_linkedin_client_secret,
        "access_token_url": "https://www.linkedin.com/oauth/v2/accessToken",
        "authorize_url": "https://www.linkedin.com/oauth/v2/authorization",
        "api_base_url": "https://api.linkedin.com/v2/",
        "client_kwargs": {"scope": "openid email profile"},
    },
}


def configured_providers() -> set[str]:
    return {
        name
        for name, meta in _PROVIDERS_METADATA.items()
        if meta["client_id"] and meta["client_secret"]
    }


for _name in configured_providers():
    oauth.register(name=_name, **_PROVIDERS_METADATA[_name])
