from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_create_page_requires_admin(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.post(
        "/api/v1/pages",
        json={"slug": "chi-siamo", "locale": "it", "title": "Chi siamo", "content": "..."},
        headers=user.headers,
    )
    assert res.status_code == 403


async def test_page_crud_and_translations(client: AsyncClient, make_admin: Callable) -> None:
    admin: AuthedUser = await make_admin()

    create_res = await client.post(
        "/api/v1/pages",
        json={
            "slug": "chi-siamo",
            "locale": "it",
            "title": "Chi siamo",
            "content": "Siamo Notturni.",
            "is_published": True,
        },
        headers=admin.headers,
    )
    assert create_res.status_code == 201
    page = create_res.json()

    translation_res = await client.post(
        f"/api/v1/pages/{page['id']}/translations",
        json={
            "slug": "about-us",
            "locale": "en",
            "title": "About us",
            "content": "We are Notturni.",
            "is_published": True,
        },
        headers=admin.headers,
    )
    assert translation_res.status_code == 201
    assert translation_res.json()["translation_group_id"] == page["translation_group_id"]

    it_res = await client.get("/api/v1/pages/chi-siamo", params={"locale": "it"})
    assert it_res.status_code == 200
    assert it_res.json()["title"] == "Chi siamo"

    en_res = await client.get("/api/v1/pages/about-us", params={"locale": "en"})
    assert en_res.status_code == 200

    missing_res = await client.get("/api/v1/pages/chi-siamo", params={"locale": "de"})
    assert missing_res.status_code == 404

    list_res = await client.get("/api/v1/pages", params={"locale": "it"})
    assert len(list_res.json()) == 1

    update_res = await client.patch(
        f"/api/v1/pages/{page['id']}", json={"title": "Chi siamo (aggiornato)"}, headers=admin.headers
    )
    assert update_res.status_code == 200
    assert update_res.json()["title"] == "Chi siamo (aggiornato)"


async def test_unpublished_page_not_public(client: AsyncClient, make_admin: Callable) -> None:
    admin: AuthedUser = await make_admin()
    await client.post(
        "/api/v1/pages",
        json={"slug": "bozza", "locale": "it", "title": "x", "content": "y", "is_published": False},
        headers=admin.headers,
    )

    res = await client.get("/api/v1/pages/bozza", params={"locale": "it"})
    assert res.status_code == 404


async def test_admin_sees_unpublished_pages(client: AsyncClient, make_admin: Callable) -> None:
    admin: AuthedUser = await make_admin()
    await client.post(
        "/api/v1/pages",
        json={"slug": "bozza2", "locale": "it", "title": "x", "content": "y", "is_published": False},
        headers=admin.headers,
    )

    detail_res = await client.get(
        "/api/v1/pages/bozza2", params={"locale": "it"}, headers=admin.headers
    )
    assert detail_res.status_code == 200

    list_res = await client.get("/api/v1/pages", params={"locale": "it"}, headers=admin.headers)
    slugs = {p["slug"] for p in list_res.json()}
    assert "bozza2" in slugs
