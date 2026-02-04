from __future__ import annotations

import os
from pathlib import Path
from typing import Optional

from cryptography.fernet import Fernet


ENC_ALG = "fernet"
ENC_VERSION = 1


def load_key(key_env: str, key_path: str = "") -> Optional[bytes]:
    value = os.getenv(key_env, "").strip()
    if value:
        return value.encode("utf-8")
    if key_path:
        try:
            file_value = Path(key_path).read_text(encoding="utf-8").strip()
        except OSError:
            return None
        if file_value:
            return file_value.encode("utf-8")
    return None


def generate_key() -> str:
    return Fernet.generate_key().decode("ascii")


def encrypt_text(plain_text: str, key: bytes) -> str:
    f = Fernet(key)
    return f.encrypt(plain_text.encode("utf-8")).decode("ascii")


def wrap_encrypted(token: str) -> str:
    return (
        '{"__enc__":"'
        + token
        + '","__alg__":"'
        + ENC_ALG
        + '","__v__":'
        + str(ENC_VERSION)
        + "}"
    )
