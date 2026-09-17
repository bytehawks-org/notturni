PASSWORD_MIN_LENGTH = 10


def validate_password_policy(password: str) -> None:
    if len(password) < PASSWORD_MIN_LENGTH:
        raise ValueError(f"La password deve avere almeno {PASSWORD_MIN_LENGTH} caratteri.")
