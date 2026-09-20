import colorsys
import re
from typing import Any

from app.domain.platform_config import MAX_FOOTER_MARKDOWN_LENGTH

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
    "typography": {"heading_font": "Lora", "body_font": "Source Sans 3", "monospace_font": "JetBrains Mono"},
    "layout": "standard",
}

MAX_PALETTE_COLORS = 5
MAX_FONTS = 3

# Chiavi note di `typography` che non sono un font (dimensione/misura del
# corpo, mockup 2e): pur essendo stringhe come i nomi dei font, non devono
# contare verso MAX_FONTS. Chiavi non riconosciute (compresi eventuali nomi
# di font futuri) restano conteggiate, come da comportamento originale.
NON_FONT_TYPOGRAPHY_KEYS = {"body_size", "measure"}

# Estetica CLAUDE.md #4/#5: "titoli in serif, testo e link in sans-serif" —
# elenco curato di Google Fonts coerenti con il tono elegante/moderno
# richiesto, applicato solo alle tre chiavi note dello schema (heading_font/
# body_font/monospace_font): chiavi di typography non riconosciute restano
# libere per non bloccare l'evoluzione futura del layout.
SERIF_FONTS = {"Lora", "Merriweather", "Playfair Display", "Source Serif 4", "Crimson Pro"}
SANS_SERIF_FONTS = {"Inter", "Nunito Sans", "Work Sans", "Source Sans 3", "Karla"}
# Font a spaziatura fissa dei blocchi di codice (blocco "evidenziazione
# sintassi"): JetBrains Mono è il default di piattaforma (DEFAULT_BLOG_CONFIG
# sopra).
MONOSPACE_FONTS = {"JetBrains Mono", "Fira Code", "IBM Plex Mono", "Source Code Pro", "Space Mono"}

_HEX_COLOR_RE = re.compile(r"^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$")

# Soglia di saturazione HLS oltre la quale un colore si considera troppo
# acceso/aggressivo per la palette "calma e amichevole" richiesta (CLAUDE.md
# #5, sezione Estetica): scelta prudente, lascia passare colori pieni ma non
# ai limiti (es. rosso/verde/blu puri).
MAX_SATURATION = 0.9


def _hex_to_saturation(value: str) -> float | None:
    if not _HEX_COLOR_RE.match(value):
        return None
    hex_value = value.lstrip("#")
    if len(hex_value) == 3:
        hex_value = "".join(ch * 2 for ch in hex_value)
    r, g, b = (int(hex_value[i : i + 2], 16) / 255 for i in (0, 2, 4))
    _, _, s = colorsys.rgb_to_hls(r, g, b)
    return s


def validate_blog_config(config: dict[str, Any]) -> None:
    """Applica i vincoli espliciti di CLAUDE.md #1/#5 (max 5 colori, max 3
    font, titoli in serif/corpo in sans-serif, palette non aggressiva); il
    resto della struttura resta libero per non bloccare l'evoluzione di
    grafica/layout/disposizione."""
    # `palette_dark` (todo/UX_REDESIGN.md B1, mockup 2e "Genera variante
    # scura"): stessi vincoli della palette chiara; assente = in tema scuro
    # vale la palette scura di piattaforma.
    for key in ("palette", "palette_dark"):
        palette = config.get(key)
        if palette is None:
            continue
        if not isinstance(palette, dict):
            raise ValueError(f"{key} deve essere un oggetto {{nome: colore}}.")
        if len(palette) > MAX_PALETTE_COLORS:
            raise ValueError(f"La palette può avere al massimo {MAX_PALETTE_COLORS} colori.")
        for name, value in palette.items():
            if not isinstance(value, str):
                continue
            saturation = _hex_to_saturation(value)
            if saturation is not None and saturation > MAX_SATURATION:
                raise ValueError(
                    f"Il colore '{name}' ({value}) è troppo acceso per una palette calma: "
                    f"saturazione massima consentita {int(MAX_SATURATION * 100)}%."
                )

    typography = config.get("typography")
    if typography is not None:
        if not isinstance(typography, dict):
            raise ValueError("typography deve essere un oggetto.")
        fonts = {
            v
            for k, v in typography.items()
            if k not in NON_FONT_TYPOGRAPHY_KEYS and isinstance(v, str)
        }
        if len(fonts) > MAX_FONTS:
            raise ValueError(f"Al massimo {MAX_FONTS} font distinti.")

        heading_font = typography.get("heading_font")
        if isinstance(heading_font, str) and heading_font not in SERIF_FONTS:
            raise ValueError(
                f"heading_font deve essere un font serif tra: {', '.join(sorted(SERIF_FONTS))}."
            )
        body_font = typography.get("body_font")
        if isinstance(body_font, str) and body_font not in SANS_SERIF_FONTS:
            raise ValueError(
                f"body_font deve essere un font sans-serif tra: {', '.join(sorted(SANS_SERIF_FONTS))}."
            )
        monospace_font = typography.get("monospace_font")
        if isinstance(monospace_font, str) and monospace_font not in MONOSPACE_FONTS:
            raise ValueError(
                f"monospace_font deve essere un font a spaziatura fissa tra: {', '.join(sorted(MONOSPACE_FONTS))}."
            )

    # Override per il proprio blog delle sole colonne 1/2 del footer di
    # piattaforma (richiesta esplicita): colonna 3 e barra inferiore restano
    # sempre e solo di piattaforma, non fanno parte di questo schema.
    footer = config.get("footer")
    if footer is not None:
        if not isinstance(footer, dict):
            raise ValueError("footer deve essere un oggetto.")
        unknown = set(footer) - {"column1", "column2"}
        if unknown:
            raise ValueError(f"Chiavi non valide in footer: {', '.join(sorted(unknown))}.")
        for key, value in footer.items():
            if value is None:
                continue
            if not isinstance(value, str):
                raise ValueError(f"footer.{key} deve essere una stringa Markdown.")
            if len(value) > MAX_FOOTER_MARKDOWN_LENGTH:
                raise ValueError(f"footer.{key} supera i {MAX_FOOTER_MARKDOWN_LENGTH} caratteri.")
