"""Modo effettivo dei commenti di un post (override del post, altrimenti del
blog, più la chiusura automatica B4) — in `domain` perché serve sia a
posts.py (serializzazione) sia a comments.py (creazione)."""

from datetime import datetime, timedelta, timezone

from app.models.blog import Blog
from app.models.comment import CommentsMode
from app.models.post import Post


def effective_comments_mode(post: Post, blog: Blog) -> CommentsMode:
    mode = post.comments_mode or blog.comments_mode
    days = blog.comments_auto_close_days
    if mode != CommentsMode.CLOSED and days and post.published_at is not None:
        if post.published_at + timedelta(days=days) <= datetime.now(timezone.utc):
            return CommentsMode.CLOSED
    return mode
