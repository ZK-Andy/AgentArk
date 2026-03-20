"""
Mem0 Bridge — Lightweight FastAPI wrapper around the mem0 library.

Provides AgentArk with intelligent memory extraction, deduplication,
contradiction resolution, and semantic search. Adds a decay layer on top
of Mem0 so that ephemeral context fades while core facts persist forever.

Memory tiers:
  - Core facts  : "lives in Tokyo", "name is Alex", preferences → never decay
  - Context     : everything else → exponential decay over time

LLM config is pushed dynamically from AgentArk's model pool via /configure.
"""

import copy
import gc
import logging
import math
import os
import re
import time
from typing import Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("mem0-bridge")

app = FastAPI(title="AgentArk Mem0 Bridge", version="2.0.0")

# Global state
memory_instance = None
configured = False
last_valid_config = None
active_qdrant_generation = 0
embedder_instance = None

# Paths for persistent data
QDRANT_PATH = os.environ.get("QDRANT_PATH", "/data/qdrant")
MODEL_CACHE = os.environ.get("MODEL_CACHE", "/data/models")
DEFAULT_COLLECTION_NAME = os.environ.get("MEM0_COLLECTION_NAME", "agentark_memories")
MIGRATIONS_COLLECTION_NAME = "mem0migrations"
DEFAULT_EMBEDDER_MODEL = os.environ.get("MEM0_EMBEDDER_MODEL", "all-MiniLM-L6-v2")

# ── Decay Configuration ─────────────────────────────────────────────────

DECAY_RATE = 0.995          # λ — exponential decay per hour (~50% per 58 hours)
DECAY_FLOOR = 0.05          # Below this score → eligible for pruning
MAX_MEMORIES = 500           # Hard cap — prune lowest-scored beyond this
ACCESS_BOOST = 0.15          # Recency bonus when a memory is accessed
CORE_FACT_BOOST = 1.0        # Core facts get max recency (never decay)

# Patterns that indicate a core/permanent fact (case-insensitive)
CORE_FACT_PATTERNS = [
    r"\b(?:name is|called)\b",
    r"\b(?:live[sd]? in|stay[sd]? in|based in|located in|moved to|from)\b",
    r"\b(?:born in|born on|birthday)\b",
    r"\b(?:work[sd]? at|work[sd]? for|job is|occupation|profession|employed)\b",
    r"\b(?:speak[sd]?|language[sd]?|fluent)\b",
    r"\b(?:prefer[sd]?|like[sd]?|love[sd]?|hate[sd]?|dislike[sd]?|favorite|favourite)\b",
    r"\b(?:allergic|allergy|dietary|vegetarian|vegan)\b",
    r"\b(?:married|wife|husband|partner|spouse|children|kids|son|daughter)\b",
    r"\b(?:email is|phone is|number is|address is)\b",
    r"\b(?:timezone|time zone)\b",
    r"\b(?:always|never|every day|every morning|routine)\b",
    r"\b(?:use[sd]?|using|tool|stack|framework|editor|ide)\b",
]

_core_fact_re = re.compile("|".join(CORE_FACT_PATTERNS), re.IGNORECASE)


def is_core_fact(text: str) -> bool:
    """Heuristic: does this memory look like a durable personal fact?"""
    return bool(_core_fact_re.search(text))


def calculate_decay_score(
    created_at: float,
    last_accessed: float,
    access_count: int,
    is_core: bool,
    now: float | None = None,
) -> float:
    """
    Exponential decay with access boost.
    Core facts always return 1.0 (no decay).
    """
    if is_core:
        return CORE_FACT_BOOST

    if now is None:
        now = time.time()

    hours_since_created = max((now - created_at) / 3600.0, 0.0)
    hours_since_accessed = max((now - last_accessed) / 3600.0, 0.0)

    # Base decay from creation time
    base = math.exp(-((1 - DECAY_RATE) * hours_since_created))

    # Access recency bonus (decays from last access, not creation)
    access_bonus = ACCESS_BOOST * math.exp(-((1 - DECAY_RATE) * hours_since_accessed))

    # Frequency bonus: log scale of access count
    freq_bonus = min(0.1 * math.log1p(access_count), 0.3)

    return min(base + access_bonus + freq_bonus, 1.0)


# ── Metadata helpers ─────────────────────────────────────────────────────
# We store decay metadata in a separate dict keyed by memory ID.
# This avoids fighting with Mem0's internal metadata format.
# Persisted as a JSON file alongside Qdrant data.

import json

METADATA_PATH = os.path.join(
    os.environ.get("QDRANT_PATH", "/data/qdrant"), "decay_metadata.json"
)

_decay_meta: dict[str, dict] = {}


def _load_metadata():
    global _decay_meta
    try:
        with open(METADATA_PATH, "r") as f:
            _decay_meta = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        _decay_meta = {}


def _save_metadata():
    os.makedirs(os.path.dirname(METADATA_PATH), exist_ok=True)
    with open(METADATA_PATH, "w") as f:
        json.dump(_decay_meta, f)


def _get_meta(memory_id: str) -> dict:
    """Get or create metadata for a memory."""
    if memory_id not in _decay_meta:
        now = time.time()
        _decay_meta[memory_id] = {
            "created_at": now,
            "last_accessed": now,
            "access_count": 0,
            "is_core": False,
        }
    return _decay_meta[memory_id]


def _touch(memory_id: str):
    """Record an access."""
    meta = _get_meta(memory_id)
    meta["last_accessed"] = time.time()
    meta["access_count"] = meta.get("access_count", 0) + 1


# Load on startup
_load_metadata()


# ── Request/Response Models ──────────────────────────────────────────────


class ConfigureRequest(BaseModel):
    provider: str  # "openai", "anthropic", "ollama"
    model: str
    api_key: Optional[str] = None
    base_url: Optional[str] = None


class Message(BaseModel):
    role: str
    content: str


class AddRequest(BaseModel):
    messages: list[Message]
    user_id: str = "default"


class SearchRequest(BaseModel):
    query: str
    user_id: str = "default"
    limit: int = 5


class EmbedRequest(BaseModel):
    texts: list[str]


class CleanupRequest(BaseModel):
    user_id: str = "default"
    decay_floor: float = DECAY_FLOOR
    max_memories: int = MAX_MEMORIES


def _build_mem0_config(req: ConfigureRequest) -> dict:
    llm_config: dict = {"model": req.model}

    if req.provider == "openai":
        if req.api_key:
            llm_config["api_key"] = req.api_key
            os.environ["OPENAI_API_KEY"] = req.api_key
        if req.base_url:
            llm_config["openai_base_url"] = req.base_url
            os.environ["OPENAI_BASE_URL"] = req.base_url
        else:
            os.environ.pop("OPENAI_BASE_URL", None)
    elif req.provider == "anthropic":
        if req.api_key:
            llm_config["api_key"] = req.api_key
            os.environ["ANTHROPIC_API_KEY"] = req.api_key
    elif req.provider == "ollama":
        if req.base_url:
            llm_config["ollama_base_url"] = req.base_url

    config = {
        "llm": {
            "provider": req.provider,
            "config": llm_config,
        },
        "embedder": {
            "provider": "huggingface",
            "config": {
                "model": DEFAULT_EMBEDDER_MODEL,
                "model_kwargs": {"cache_folder": MODEL_CACHE},
            },
        },
        "vector_store": {
            "provider": "qdrant",
            "config": {
                "collection_name": DEFAULT_COLLECTION_NAME,
                "path": QDRANT_PATH,
                "embedding_model_dims": 384,
            },
        },
    }
    config["vector_store"]["config"]["path"] = _qdrant_path_for_config(config)
    return config


def _get_embedder():
    global embedder_instance
    if embedder_instance is None:
        from sentence_transformers import SentenceTransformer

        embedder_instance = SentenceTransformer(
            DEFAULT_EMBEDDER_MODEL,
            cache_folder=MODEL_CACHE,
        )
    return embedder_instance


def _embedder_dimension(config: Optional[dict]) -> Optional[int]:
    if not config:
        return None
    embedder = config.get("embedder", {})
    provider = str(embedder.get("provider", "")).strip().lower()
    if provider != "huggingface":
        return None

    embedder_cfg = embedder.get("config", {}) or {}
    model_name = str(embedder_cfg.get("model", DEFAULT_EMBEDDER_MODEL)).strip()
    model_kwargs = embedder_cfg.get("model_kwargs", {}) or {}
    try:
        from sentence_transformers import SentenceTransformer

        model = SentenceTransformer(model_name, **model_kwargs)
        dim = model.get_sentence_embedding_dimension()
        return int(dim) if dim else None
    except Exception as e:
        logger.warning("Could not determine embedder dimension for %s: %s", model_name, e)
        return None


def _safe_slug(value: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "-", value.strip().lower())
    return normalized.strip("-") or "default"


def _qdrant_path_for_config(config: Optional[dict], generation: Optional[int] = None) -> str:
    if generation is None:
        generation = active_qdrant_generation

    embedder = config.get("embedder", {}) if config else {}
    provider = str(embedder.get("provider", "default")).strip()
    embedder_cfg = embedder.get("config", {}) or {}
    model_name = str(embedder_cfg.get("model", DEFAULT_EMBEDDER_MODEL)).strip()
    dim = _embedder_dimension(config)

    parts = [_safe_slug(provider), _safe_slug(model_name)]
    if dim is not None:
        parts.append(f"{dim}d")
    if generation:
        parts.append(f"g{generation}")
    return os.path.join(QDRANT_PATH, "-".join(parts))


def _configured_qdrant_path(config: Optional[dict]) -> str:
    if not config:
        return QDRANT_PATH
    return (
        config.get("vector_store", {})
        .get("config", {})
        .get("path", QDRANT_PATH)
    )


def _configured_collection_names(config: Optional[dict]) -> list[str]:
    names = {DEFAULT_COLLECTION_NAME, MIGRATIONS_COLLECTION_NAME}
    if config:
        collection_name = (
            config.get("vector_store", {})
            .get("config", {})
            .get("collection_name")
        )
        if isinstance(collection_name, str) and collection_name.strip():
            names.add(collection_name.strip())
    return sorted(names)


def _check_known_qdrant_dimensions(qdrant_path: str, config: Optional[dict]) -> Optional[int]:
    expected_dim = _embedder_dimension(config)
    if expected_dim is None:
        return None
    for collection_name in _configured_collection_names(config):
        _check_qdrant_dimensions(qdrant_path, collection_name, expected_dim)
    return expected_dim


def _reset_qdrant_collection(qdrant_path: str, expected_dim: Optional[int] = None):
    """Wipe the entire Qdrant storage directory so it gets recreated fresh."""
    import shutil
    if os.path.isdir(qdrant_path):
        shutil.rmtree(qdrant_path)
    os.makedirs(qdrant_path, exist_ok=True)
    # Also clear stale decay metadata since memories are gone
    global _decay_meta
    _decay_meta = {}
    _save_metadata()
    logger.info("Qdrant storage reset — will be recreated with dim=%d", expected_dim)


def _reset_all_qdrant_storage(expected_dim: Optional[int] = None):
    """Wipe the full Qdrant root, including all derived generation paths."""
    import shutil

    if os.path.isdir(QDRANT_PATH):
        shutil.rmtree(QDRANT_PATH)
    os.makedirs(QDRANT_PATH, exist_ok=True)
    global _decay_meta
    _decay_meta = {}
    _save_metadata()
    logger.info(
        "All Qdrant storage reset - all generations cleared, expected dim=%s",
        expected_dim if expected_dim is not None else "unknown",
    )


def _check_qdrant_dimensions(qdrant_path: str, collection_name: str, expected_dim: int):
    """If an existing Qdrant collection has wrong vector dimensions, delete it.
    Checks both meta.json and the internal collection config files."""
    # Method 1: Check meta.json (Qdrant local storage format)
    meta_path = os.path.join(qdrant_path, "meta.json")
    if os.path.exists(meta_path):
        try:
            with open(meta_path, "r") as f:
                meta = json.load(f)
            coll = meta.get("collections", {}).get(collection_name, {})
            stored_dim = coll.get("vectors", {}).get("size")
            if stored_dim is not None and stored_dim != expected_dim:
                logger.warning(
                    "Qdrant meta.json: collection '%s' has dim=%d but embedder needs %d",
                    collection_name, stored_dim, expected_dim,
                )
                _reset_qdrant_collection(qdrant_path, expected_dim)
                return
        except Exception as e:
            logger.warning("Could not parse Qdrant meta.json: %s", e)

    # Method 2: Check collection directory for config with vector size
    # (handles cases where meta.json doesn't have the size but data exists)
    coll_path = os.path.join(qdrant_path, "collection", collection_name)
    if os.path.isdir(coll_path):
        import glob
        for cfg_file in glob.glob(os.path.join(coll_path, "**", "*.json"), recursive=True):
            try:
                with open(cfg_file, "r") as f:
                    cfg = json.load(f)
                # Look for vector size in various config locations
                vec_cfg = cfg.get("params", {}).get("vectors", {})
                if isinstance(vec_cfg, dict):
                    size = vec_cfg.get("size")
                    if size is not None and size != expected_dim:
                        logger.warning(
                            "Qdrant config %s: dim=%d but embedder needs %d",
                            cfg_file, size, expected_dim,
                        )
                        _reset_qdrant_collection(qdrant_path, expected_dim)
                        return
            except (json.JSONDecodeError, OSError):
                continue


def _reinitialize_memory_from_last_config(reset_storage: bool, reason: str) -> bool:
    global memory_instance, configured, last_valid_config, active_qdrant_generation

    if not last_valid_config:
        logger.warning(
            "Mem0 re-initialization skipped (%s): no last valid config available",
            reason,
        )
        memory_instance = None
        configured = False
        return False

    try:
        from mem0 import Memory
        max_attempts = 2 if reset_storage else 1
        for attempt in range(max_attempts):
            config = copy.deepcopy(last_valid_config)
            expected_dim = _embedder_dimension(config)
            doing_clean_reset = reset_storage or attempt > 0
            if doing_clean_reset:
                active_qdrant_generation += 1
            config.setdefault("vector_store", {}).setdefault("config", {})["path"] = (
                _qdrant_path_for_config(config, active_qdrant_generation)
            )
            qdrant_path = _configured_qdrant_path(config)

            old_instance = memory_instance
            memory_instance = None
            del old_instance
            gc.collect()

            if doing_clean_reset:
                _reset_all_qdrant_storage(expected_dim)
                _reset_qdrant_collection(qdrant_path, expected_dim)
            else:
                _check_known_qdrant_dimensions(qdrant_path, config)

            candidate = Memory.from_config(config)
            try:
                candidate.search(
                    "bridge validation",
                    user_id="system:bridge_validate",
                    limit=1,
                )
            except Exception as validation_err:
                validation_msg = str(validation_err).strip()
                del candidate
                gc.collect()
                if (
                    ("not aligned" in validation_msg or "dimension" in validation_msg.lower())
                    and attempt + 1 < max_attempts
                ):
                    logger.warning(
                        "Mem0 re-init validation failed (%s), retrying from clean storage: %s",
                        reason,
                        validation_err,
                    )
                    continue
                raise

            memory_instance = candidate
            configured = True
            last_valid_config = copy.deepcopy(config)
            logger.info(
                "Mem0 re-initialized successfully (%s)%s",
                reason,
                f" with dim={expected_dim}" if expected_dim is not None else "",
            )
            return True
    except Exception as e:
        logger.error("Failed to re-initialize Mem0 (%s): %s", reason, e)
        memory_instance = None
        configured = False
        return False


def _ensure_memory_ready() -> bool:
    if configured and memory_instance is not None:
        return True
    if last_valid_config:
        return _reinitialize_memory_from_last_config(
            reset_storage=False,
            reason="lazy_restore",
        )
    return False


def _format_search_results(results, limit: int) -> dict:
    if isinstance(results, dict) and "results" in results:
        raw = results["results"]
    elif isinstance(results, dict) and "memories" in results:
        raw = results["memories"]
    elif isinstance(results, list):
        raw = results
    else:
        raw = []

    now = time.time()
    scored = []
    for item in raw:
        mem_id = item.get("id", "")
        mem_text = item.get("memory", "")
        semantic_score = item.get("score", 0.0)

        meta = _get_meta(mem_id)
        if not meta.get("is_core") and is_core_fact(mem_text):
            meta["is_core"] = True

        decay = calculate_decay_score(
            created_at=meta["created_at"],
            last_accessed=meta["last_accessed"],
            access_count=meta.get("access_count", 0),
            is_core=meta.get("is_core", False),
            now=now,
        )

        combined = semantic_score * (0.5 + 0.5 * decay)
        scored.append({
            "id": mem_id,
            "memory": mem_text,
            "score": round(combined, 4),
            "is_core": meta.get("is_core", False),
            "decay": round(decay, 4),
        })

    scored.sort(key=lambda x: x["score"], reverse=True)
    top = scored[:limit]

    for item in top:
        _touch(item["id"])
    _save_metadata()

    return {"memories": top}


# ── Endpoints ────────────────────────────────────────────────────────────


@app.get("/health")
def health():
    total = len(_decay_meta)
    core = sum(1 for m in _decay_meta.values() if m.get("is_core", False))
    return {
        "status": "ok",
        "configured": configured,
        "memories": total,
        "core_facts": core,
        "ephemeral": total - core,
    }


@app.post("/configure")
def configure(req: ConfigureRequest):
    global memory_instance, configured, last_valid_config

    from mem0 import Memory
    config = _build_mem0_config(req)

    # Auto-fix dimension mismatch from stale data across known collections.
    _check_known_qdrant_dimensions(_configured_qdrant_path(config), config)

    try:
        memory_instance = Memory.from_config(copy.deepcopy(config))
        configured = True
        last_valid_config = copy.deepcopy(config)
        display_provider = req.provider
        if req.provider == "openai" and req.base_url:
            if "openrouter" in req.base_url:
                display_provider = "openrouter"
            else:
                display_provider = "openai-compatible"
        logger.info("Mem0 configured: provider=%s model=%s", display_provider, req.model)
        return {"status": "ok", "provider": display_provider, "model": req.model}
    except Exception as e:
        logger.error("Failed to configure Mem0: %s", e)
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/memories")
def add_memories(req: AddRequest):
    global memory_instance, configured, last_valid_config
    if not _ensure_memory_ready():
        return {
            "status": "degraded",
            "result": {"results": []},
            "warning": "mem0 unavailable; skipped memory write",
        }

    messages = [{"role": m.role, "content": m.content} for m in req.messages]

    try:
        result = memory_instance.add(messages, user_id=req.user_id)

        # Track metadata for newly created memories
        if isinstance(result, dict) and "results" in result:
            for entry in result["results"]:
                if entry.get("event") in ("ADD", "UPDATE"):
                    mem_id = entry.get("id", "")
                    memory_text = entry.get("memory", "")
                    if mem_id:
                        meta = _get_meta(mem_id)
                        meta["is_core"] = is_core_fact(memory_text)
                        meta["last_accessed"] = time.time()
            _save_metadata()

        logger.info("Added memories for user=%s", req.user_id)
        return {"status": "ok", "result": result}
    except Exception as e:
        err_msg = str(e).strip()
        if "not aligned" in err_msg or "dimension" in err_msg.lower():
            logger.warning(
                "Embedding dimension mismatch during add; resetting Mem0 storage: %s",
                err_msg,
            )
            if _reinitialize_memory_from_last_config(
                reset_storage=True,
                reason="add_dimension_mismatch",
            ):
                try:
                    result = memory_instance.add(messages, user_id=req.user_id)
                    logger.info("Added memories for user=%s (after reset)", req.user_id)
                    return {"status": "ok", "result": result}
                except Exception as retry_err:
                    logger.error(
                        "Mem0 add retry failed after reset for user=%s: %s",
                        req.user_id,
                        retry_err,
                    )
            return {
                "status": "degraded",
                "result": {"results": []},
                "warning": "mem0 reset after embedding mismatch; skipped this memory write",
            }

        logger.error("Failed to add memories: %s", e)
        return {
            "status": "degraded",
            "result": {"results": []},
            "warning": "mem0 add failed; skipped memory write",
        }


@app.post("/memories/search")
def search_memories(req: SearchRequest):
    global memory_instance, configured, last_valid_config
    if not _ensure_memory_ready():
        return {
            "memories": [],
            "warning": "mem0 unavailable; search skipped",
        }

    try:
        # Ask for more than needed so we can re-rank after decay
        fetch_limit = min(req.limit * 3, 30)
        try:
            results = memory_instance.search(
                req.query, user_id=req.user_id, limit=fetch_limit
            )
        except Exception as search_err:
            err_msg = str(search_err).strip()
            if "not aligned" in err_msg or "dimension" in err_msg.lower():
                logger.warning(
                    "Embedding dimension mismatch during search; resetting Mem0 storage: %s",
                    err_msg,
                )
                if _reinitialize_memory_from_last_config(
                    reset_storage=True,
                    reason="search_dimension_mismatch",
                ):
                    try:
                        results = memory_instance.search(
                            req.query, user_id=req.user_id, limit=fetch_limit
                        )
                        return _format_search_results(results, req.limit)
                    except Exception as retry_err:
                        logger.error(
                            "Mem0 search retry failed after reset for user=%s: %s",
                            req.user_id,
                            retry_err,
                        )
                        return {
                            "memories": [],
                            "warning": "mem0 reset after embedding mismatch; search skipped",
                        }
                else:
                    return {
                        "memories": [],
                        "warning": "mem0 reset failed after embedding mismatch; search skipped",
                    }
            raise

        return _format_search_results(results, req.limit)
    except Exception as e:
        logger.error("Failed to search memories: %s", e)
        return {
            "memories": [],
            "warning": "mem0 search failed; returning no memories",
        }


@app.post("/embed")
def embed_texts(req: EmbedRequest):
    if not req.texts:
        return {
            "model": DEFAULT_EMBEDDER_MODEL,
            "dimensions": 0,
            "embeddings": [],
        }

    try:
        model = _get_embedder()
        vectors = model.encode(
            req.texts,
            show_progress_bar=False,
            normalize_embeddings=True,
        )
        embeddings = vectors.tolist() if hasattr(vectors, "tolist") else list(vectors)
        dimensions = len(embeddings[0]) if embeddings else 0
        return {
            "model": DEFAULT_EMBEDDER_MODEL,
            "dimensions": dimensions,
            "embeddings": embeddings,
        }
    except Exception as e:
        logger.error("Failed to embed texts: %s", e)
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/memories")
def list_memories(user_id: str = "default"):
    if not configured or memory_instance is None:
        raise HTTPException(status_code=503, detail="not_configured")

    try:
        results = memory_instance.get_all(user_id=user_id)
        memories = []
        raw = results if isinstance(results, list) else results.get("results", [])
        now = time.time()
        for item in raw:
            mem_id = item.get("id", "")
            mem_text = item.get("memory", "")
            meta = _get_meta(mem_id)

            decay = calculate_decay_score(
                created_at=meta["created_at"],
                last_accessed=meta["last_accessed"],
                access_count=meta.get("access_count", 0),
                is_core=meta.get("is_core", False),
                now=now,
            )
            memories.append({
                "id": mem_id,
                "memory": mem_text,
                "is_core": meta.get("is_core", False),
                "decay": round(decay, 4),
                "access_count": meta.get("access_count", 0),
            })
        return {"memories": memories}
    except Exception as e:
        logger.error("Failed to list memories: %s", e)
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/cleanup")
def cleanup_memories(req: CleanupRequest):
    """
    Prune decayed ephemeral memories. Core facts are never deleted.

    Two-pass pruning:
      1. Delete ephemeral memories with decay score below floor
      2. If still over max_memories, delete lowest-scored ephemeral first
    """
    if not configured or memory_instance is None:
        raise HTTPException(status_code=503, detail="not_configured")

    try:
        results = memory_instance.get_all(user_id=req.user_id)
        raw = results if isinstance(results, list) else results.get("results", [])

        now = time.time()
        ephemeral = []
        core_count = 0

        for item in raw:
            mem_id = item.get("id", "")
            mem_text = item.get("memory", "")
            meta = _get_meta(mem_id)

            # Auto-classify
            if not meta.get("is_core") and is_core_fact(mem_text):
                meta["is_core"] = True

            if meta.get("is_core", False):
                core_count += 1
                continue

            decay = calculate_decay_score(
                created_at=meta["created_at"],
                last_accessed=meta["last_accessed"],
                access_count=meta.get("access_count", 0),
                is_core=False,
                now=now,
            )
            ephemeral.append({"id": mem_id, "decay": decay, "memory": mem_text})

        # Pass 1: delete below floor
        deleted_ids = []
        for mem in ephemeral:
            if mem["decay"] < req.decay_floor:
                try:
                    memory_instance.delete(mem["id"])
                    deleted_ids.append(mem["id"])
                    logger.info(
                        "Pruned decayed memory: '%s' (decay=%.3f)",
                        mem["memory"][:60],
                        mem["decay"],
                    )
                except Exception:
                    pass

        # Remove deleted from ephemeral list
        remaining = [m for m in ephemeral if m["id"] not in deleted_ids]

        # Pass 2: enforce hard cap (core + remaining ephemeral)
        total = core_count + len(remaining)
        if total > req.max_memories:
            excess = total - req.max_memories
            # Sort by decay ascending (lowest first = most decayed)
            remaining.sort(key=lambda x: x["decay"])
            for mem in remaining[:excess]:
                try:
                    memory_instance.delete(mem["id"])
                    deleted_ids.append(mem["id"])
                    logger.info(
                        "Pruned excess memory: '%s' (decay=%.3f)",
                        mem["memory"][:60],
                        mem["decay"],
                    )
                except Exception:
                    pass

        # Clean up metadata for deleted memories
        for mid in deleted_ids:
            _decay_meta.pop(mid, None)
        _save_metadata()

        total_after = core_count + len(remaining) - len([
            m for m in remaining if m["id"] in deleted_ids
        ])

        return {
            "status": "ok",
            "deleted": len(deleted_ids),
            "remaining": total_after,
            "core_facts": core_count,
        }
    except Exception as e:
        logger.error("Cleanup failed: %s", e)
        raise HTTPException(status_code=500, detail=str(e))


@app.delete("/memories/{memory_id}")
def delete_memory(memory_id: str):
    if not configured or memory_instance is None:
        raise HTTPException(status_code=503, detail="not_configured")

    try:
        memory_instance.delete(memory_id)
        _decay_meta.pop(memory_id, None)
        _save_metadata()
        return {"status": "ok"}
    except Exception as e:
        logger.error("Failed to delete memory %s: %s", memory_id, e)
        raise HTTPException(status_code=500, detail=str(e))
