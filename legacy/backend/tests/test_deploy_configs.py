"""Le copie in k8s/ dei file di configurazione condivisi con compose.yaml
(kustomize non legge fuori dalla propria directory) non devono divergere."""

from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[3]

PAIRS = [
    ("garage/garage.toml", "k8s/garage.toml"),
    ("s3-public/default.conf.template", "k8s/s3-public.conf.template"),
]


@pytest.mark.parametrize(("source", "copy"), PAIRS)
def test_k8s_copy_matches_source(source: str, copy: str) -> None:
    source_path, copy_path = ROOT / source, ROOT / copy
    if not source_path.exists():  # immagine backend / checkout parziale: nulla da confrontare
        pytest.skip("file di deploy non presenti in questo contesto")
    assert copy_path.read_text() == source_path.read_text(), f"{copy} diverge da {source}"
