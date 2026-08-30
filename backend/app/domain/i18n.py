import re

DEFAULT_LOCALE = "it"
LOCALE_PATTERN = re.compile(r"^[a-z]{2}$")


def validate_locale(locale: str) -> None:
    if not LOCALE_PATTERN.fullmatch(locale):
        raise ValueError(
            "Il codice lingua deve essere ISO 639-1 di 2 lettere minuscole (es. it, en, de)."
        )
