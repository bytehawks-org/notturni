from typing import Any

# Stessa palette/tipografia di default della piattaforma (frontend/src/app/globals.css),
# così un blog non personalizzato appare identico allo shell di base.
DEFAULT_BLOG_CONFIG: dict[str, Any] = {
    "palette": {
        "background": "#fbf9f6",
        "foreground": "#2b2a28",
        "primary": "#3e6259",
        "muted": "#a8a29a",
        "border": "#e7e2da",
    },
    "typography": {"heading_font": "Lora", "body_font": "Inter"},
    "layout": "standard",
}

MAX_PALETTE_COLORS = 5
MAX_FONTS = 3


def validate_blog_config(config: dict[str, Any]) -> None:
    """Applica solo i vincoli espliciti di CLAUDE.md #1 (max 5 colori, max 3
    font); il resto della struttura resta libero per non bloccare
    l'evoluzione di grafica/layout/disposizione."""
    palette = config.get("palette")
    if palette is not None:
        if not isinstance(palette, dict):
            raise ValueError("palette deve essere un oggetto {nome: colore}.")
        if len(palette) > MAX_PALETTE_COLORS:
            raise ValueError(f"La palette può avere al massimo {MAX_PALETTE_COLORS} colori.")

    typography = config.get("typography")
    if typography is not None:
        if not isinstance(typography, dict):
            raise ValueError("typography deve essere un oggetto.")
        fonts = {v for v in typography.values() if isinstance(v, str)}
        if len(fonts) > MAX_FONTS:
            raise ValueError(f"Al massimo {MAX_FONTS} font distinti.")
