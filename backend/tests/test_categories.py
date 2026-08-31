from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> str:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers)
    assert res.status_code == 201
    return slug


async def test_category_crud(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-cat")
    stranger: AuthedUser = await make_user("stranger-cat")
    slug = await _create_blog(client, owner, "blog-cat-test")

    forbidden = await client.post(
        f"/api/v1/blogs/{slug}/categories", json={"name": "Viaggi", "slug": "viaggi"}, headers=stranger.headers
    )
    assert forbidden.status_code == 403

    create_res = await client.post(
        f"/api/v1/blogs/{slug}/categories", json={"name": "Viaggi", "slug": "viaggi"}, headers=owner.headers
    )
    assert create_res.status_code == 201
    category_id = create_res.json()["id"]

    duplicate = await client.post(
        f"/api/v1/blogs/{slug}/categories", json={"name": "Altro", "slug": "viaggi"}, headers=owner.headers
    )
    assert duplicate.status_code == 409

    list_res = await client.get(f"/api/v1/blogs/{slug}/categories")
    assert [c["slug"] for c in list_res.json()] == ["viaggi"]

    update_res = await client.patch(
        f"/api/v1/blogs/{slug}/categories/{category_id}", json={"name": "Viaggi e trasferte"}, headers=owner.headers
    )
    assert update_res.status_code == 200
    assert update_res.json()["name"] == "Viaggi e trasferte"

    delete_res = await client.delete(f"/api/v1/blogs/{slug}/categories/{category_id}", headers=owner.headers)
    assert delete_res.status_code == 204
    list_after = await client.get(f"/api/v1/blogs/{slug}/categories")
    assert list_after.json() == []


async def test_post_category_assignment_and_validation(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-post-cat")
    slug = await _create_blog(client, owner, "blog-post-cat-test")
    other_slug = await _create_blog(client, owner, "blog-post-cat-other")

    cat_res = await client.post(
        f"/api/v1/blogs/{slug}/categories", json={"name": "Cucina", "slug": "cucina"}, headers=owner.headers
    )
    category_id = cat_res.json()["id"]

    other_cat_res = await client.post(
        f"/api/v1/blogs/{other_slug}/categories", json={"name": "Sport", "slug": "sport"}, headers=owner.headers
    )
    other_category_id = other_cat_res.json()["id"]

    # una categoria di un altro blog non è valida qui
    rejected = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-cat-sbagliata", "title": "x", "content": "y", "category_id": other_category_id},
        headers=owner.headers,
    )
    assert rejected.status_code == 400

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-con-categoria", "title": "x", "content": "y", "category_id": category_id},
        headers=owner.headers,
    )
    assert post_res.status_code == 201
    assert post_res.json()["category"] == {"id": category_id, "name": "Cucina", "slug": "cucina"}
    post_id = post_res.json()["id"]

    # rimuovere la categoria esplicitamente con null
    clear_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"category_id": None}, headers=owner.headers
    )
    assert clear_res.json()["category"] is None

    # riassegnarla, poi cancellare la categoria: il post resta senza categoria
    await client.patch(f"/api/v1/posts/{post_id}", json={"category_id": category_id}, headers=owner.headers)
    await client.delete(f"/api/v1/blogs/{slug}/categories/{category_id}", headers=owner.headers)
    final_res = await client.get(f"/api/v1/posts/{post_id}", headers=owner.headers)
    assert final_res.json()["category"] is None


async def test_translation_inherits_category_by_default(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-tr-cat")
    slug = await _create_blog(client, owner, "blog-tr-cat-test")

    cat_res = await client.post(
        f"/api/v1/blogs/{slug}/categories", json={"name": "Poesia", "slug": "poesia"}, headers=owner.headers
    )
    category_id = cat_res.json()["id"]

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-poesia", "title": "x", "content": "y", "category_id": category_id},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]

    translation_res = await client.post(
        f"/api/v1/posts/{post_id}/translations",
        json={"slug": "post-poesia-en", "locale": "en", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    assert translation_res.status_code == 201
    assert translation_res.json()["category"]["slug"] == "poesia"
