from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_default_config_when_not_customized(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-1", "title": "x"}, headers=owner.headers)

    res = await client.get("/api/v1/blogs/blog-cfg-1/config")
    assert res.status_code == 200
    assert res.json()["typography"] == {
        "heading_font": "Lora",
        "body_font": "Source Sans 3",
        "monospace_font": "JetBrains Mono",
    }


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
        json={"palette": {"primary": "#3e6259"}, "layout": "magazine"},
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


async def test_config_rejects_too_saturated_color(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-5", "title": "x"}, headers=owner.headers)

    res = await client.put(
        "/api/v1/blogs/blog-cfg-5/config",
        json={"palette": {"primary": "#ff0000"}},
        headers=owner.headers,
    )
    assert res.status_code == 400


async def test_config_body_size_and_measure_do_not_count_as_fonts(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-7", "title": "x"}, headers=owner.headers)

    res = await client.put(
        "/api/v1/blogs/blog-cfg-7/config",
        json={
            "typography": {
                "heading_font": "Playfair Display",
                "body_font": "Karla",
                "body_size": "19",
                "measure": "narrow",
            }
        },
        headers=owner.headers,
    )
    assert res.status_code == 200


async def test_config_enforces_serif_heading_sans_serif_body(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-6", "title": "x"}, headers=owner.headers)

    wrong_heading = await client.put(
        "/api/v1/blogs/blog-cfg-6/config",
        json={"typography": {"heading_font": "Inter", "body_font": "Inter"}},
        headers=owner.headers,
    )
    assert wrong_heading.status_code == 400

    wrong_body = await client.put(
        "/api/v1/blogs/blog-cfg-6/config",
        json={"typography": {"heading_font": "Lora", "body_font": "Lora"}},
        headers=owner.headers,
    )
    assert wrong_body.status_code == 400

    ok_res = await client.put(
        "/api/v1/blogs/blog-cfg-6/config",
        json={"typography": {"heading_font": "Playfair Display", "body_font": "Karla"}},
        headers=owner.headers,
    )
    assert ok_res.status_code == 200


async def test_config_enforces_monospace_font_from_curated_list(client: AsyncClient, make_user: Callable) -> None:
    """`monospace_font` (blocco "evidenziazione sintassi"): stesso principio
    di heading_font/body_font, elenco curato validato lato backend
    (backend/app/domain/blog_config.py::MONOSPACE_FONTS)."""
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-7", "title": "x"}, headers=owner.headers)

    wrong = await client.put(
        "/api/v1/blogs/blog-cfg-7/config",
        json={"typography": {"heading_font": "Lora", "body_font": "Karla", "monospace_font": "Comic Sans"}},
        headers=owner.headers,
    )
    assert wrong.status_code == 400

    ok = await client.put(
        "/api/v1/blogs/blog-cfg-7/config",
        json={"typography": {"heading_font": "Lora", "body_font": "Karla", "monospace_font": "Fira Code"}},
        headers=owner.headers,
    )
    assert ok.status_code == 200
    assert ok.json()["typography"]["monospace_font"] == "Fira Code"


async def test_config_monospace_font_counts_toward_max_fonts(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "blog-cfg-8", "title": "x"}, headers=owner.headers)

    # 3 font distinti (heading + body + monospace, tutti dagli elenchi
    # curati): esattamente al limite, ammesso.
    at_limit = await client.put(
        "/api/v1/blogs/blog-cfg-8/config",
        json={"typography": {"heading_font": "Lora", "body_font": "Karla", "monospace_font": "Fira Code", "body_size": "18"}},
        headers=owner.headers,
    )
    assert at_limit.status_code == 200
