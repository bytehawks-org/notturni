from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_default_config_when_not_customized(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-1", "title": "x"}, headers=owner.headers)

    res = await client.get("/api/v1/blogs/blog-cfg-1/config")
    assert res.status_code == 200
    assert res.json()["typography"] == {"heading_font": "Lora", "body_font": "Inter"}


async def test_update_config_owner_only(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-cfg")
    stranger: AuthedUser = await make_user("stranger-cfg")
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-2", "title": "x"}, headers=owner.headers)

    forbidden_res = await client.put(
        "/api/v1/blogs/blog-cfg-2/config", json={"layout": "rubato"}, headers=stranger.headers
    )
    assert forbidden_res.status_code == 403

    ok_res = await client.put(
        "/api/v1/blogs/blog-cfg-2/config",
        json={"palette": {"primary": "#ff0000"}, "layout": "magazine"},
        headers=owner.headers,
    )
    assert ok_res.status_code == 200

    get_res = await client.get("/api/v1/blogs/blog-cfg-2/config")
    assert get_res.json()["layout"] == "magazine"


async def test_config_rejects_too_many_colors(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-3", "title": "x"}, headers=owner.headers)

    res = await client.put(
        "/api/v1/blogs/blog-cfg-3/config",
        json={"palette": {f"c{i}": "#000000" for i in range(6)}},
        headers=owner.headers,
    )
    assert res.status_code == 400


async def test_config_rejects_too_many_fonts(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-4", "title": "x"}, headers=owner.headers)

    res = await client.put(
        "/api/v1/blogs/blog-cfg-4/config",
        json={"typography": {"a": "Font1", "b": "Font2", "c": "Font3", "d": "Font4"}},
        headers=owner.headers,
    )
    assert res.status_code == 400
