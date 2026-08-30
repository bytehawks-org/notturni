"""Bootstrap: crea il primo token API (motore core o, opzionalmente, un utente).

Non esiste ancora un login/sessione da cui ottenere un token iniziale: questo
script inserisce il token direttamente nel database. Una volta ottenuto un
token valido, se ne possono creare altri via API (POST /api/v1/tokens).

Uso (dalla directory backend/, con il venv attivo):
    python -m scripts.create_api_token --name "core-engine"
    python -m scripts.create_api_token --name "utente-mario" --user-id <uuid>
"""

import argparse
import asyncio
import uuid

from app.core.database import SessionLocal
from app.domain.api_tokens import generate_api_token
from app.models.api_token import ApiToken, ApiTokenOwnerType


async def create_token(name: str, user_id: uuid.UUID | None) -> None:
    owner_type = ApiTokenOwnerType.USER if user_id else ApiTokenOwnerType.CORE
    plaintext, prefix, token_hash = generate_api_token()

    async with SessionLocal() as session:
        session.add(
            ApiToken(
                name=name,
                owner_type=owner_type,
                user_id=user_id,
                token_prefix=prefix,
                token_hash=token_hash,
            )
        )
        await session.commit()

    print(f"Token ({owner_type.value}) creato: {plaintext}")
    print("Salvalo ora: non sarà più recuperabile in chiaro.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True, help="Etichetta descrittiva del token")
    parser.add_argument("--user-id", default=None, help="UUID utente (omesso = token core)")
    args = parser.parse_args()

    user_id = uuid.UUID(args.user_id) if args.user_id else None
    asyncio.run(create_token(args.name, user_id))


if __name__ == "__main__":
    main()
