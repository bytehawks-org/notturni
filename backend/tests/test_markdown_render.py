from app.domain.markdown_render import render_markdown_to_safe_html


def test_renders_common_markdown_constructs() -> None:
    html = render_markdown_to_safe_html("# Titolo\n\nCiao **mondo**, vedi [qui](https://example.com).")
    assert "<h1>Titolo</h1>" in html
    assert "<strong>mondo</strong>" in html
    assert '<a href="https://example.com" rel="noopener noreferrer">qui</a>' in html


def test_strips_script_tags() -> None:
    html = render_markdown_to_safe_html("Ciao <script>alert(1)</script> mondo")
    assert "<script>" not in html
    assert "alert(1)" not in html


def test_strips_javascript_scheme_from_links_and_images() -> None:
    html = render_markdown_to_safe_html("[click](javascript:alert(1)) ![img](javascript:alert(1))")
    assert "javascript:" not in html


def test_strips_event_handler_attributes() -> None:
    html = render_markdown_to_safe_html('<img src="https://example.com/x.png" onerror="alert(1)">')
    assert "onerror" not in html
    assert 'src="https://example.com/x.png"' in html


def test_lists_and_tables_render() -> None:
    html = render_markdown_to_safe_html("- uno\n- due\n\n| A | B |\n| - | - |\n| 1 | 2 |")
    assert "<ul>" in html and "<li>uno</li>" in html
    assert "<table>" in html and "<td>1</td>" in html
