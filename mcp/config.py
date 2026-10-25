import os
from dataclasses import dataclass


def _keys(raw):
    return tuple(key.strip() for key in raw.split(",") if key.strip())


@dataclass(frozen=True)
class Settings:
    public_base_url: str = "http://localhost:8090"
    db_path: str = "/data/mcp.sqlite3"
    access_keys: tuple = ()
    generator_url: str = "http://127.0.0.1:8081"
    playout_url: str = "http://127.0.0.1:8080"
    tsx_api_url: str = "http://127.0.0.1:8091"
    access_ttl: int = 3600
    refresh_ttl: int = 2592000
    code_ttl: int = 60

    @property
    def issuer(self):
        return self.public_base_url

    @property
    def resource(self):
        return self.public_base_url + "/mcp"

    @classmethod
    def from_env(cls):
        raw_keys = os.getenv("BRONZECAST_MCP_ACCESS_KEYS", "") or os.getenv(
            "BRONZECAST_MCP_ACCESS_KEY", ""
        )
        return cls(
            public_base_url=os.getenv("PUBLIC_BASE_URL", cls.public_base_url).rstrip("/"),
            db_path=os.getenv("MCP_DB", cls.db_path),
            access_keys=_keys(raw_keys),
            generator_url=os.getenv("GENERATOR_URL", cls.generator_url).rstrip("/"),
            playout_url=os.getenv("PLAYOUT_URL", cls.playout_url).rstrip("/"),
            tsx_api_url=os.getenv("BRONZECAST_TSX_API_URL", cls.tsx_api_url).rstrip("/"),
            access_ttl=int(os.getenv("MCP_ACCESS_TTL", cls.access_ttl)),
            refresh_ttl=int(os.getenv("MCP_REFRESH_TTL", cls.refresh_ttl)),
            code_ttl=int(os.getenv("MCP_CODE_TTL", cls.code_ttl)),
        )
