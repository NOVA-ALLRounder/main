from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict

import yaml

PROJECT_ROOT = Path(__file__).resolve().parents[2]


@dataclass
class IngestConfig:
    enabled: bool = True
    host: str = "127.0.0.1"
    port: int = 8080
    token: str = ""


@dataclass
class QueueConfig:
    max_size: int = 1000
    shutdown_drain_seconds: int = 3


@dataclass
class PrivacyConfig:
    hash_salt: str = "dev-salt"
    url_mode: str = "rules"


@dataclass
class StoreConfig:
    busy_timeout_ms: int = 5000
    insert_batch_size: int = 100
    insert_flush_ms: int = 1000
    insert_retry_attempts: int = 3
    insert_retry_backoff_ms: int = 50


@dataclass
class EncryptionConfig:
    enabled: bool = False
    key_env: str = "DATA_COLLECTOR_ENC_KEY"
    key_path: str = ""
    encrypt_raw_json: bool = False


@dataclass
class PriorityConfig:
    debounce_seconds: float = 2.0
    focus_event_types: list[str] = field(default_factory=lambda: ["os.foreground_changed"])
    focus_block_event_type: str = "os.app_focus_block"
    drop_p2_when_queue_over: float = 0.8
    p0_event_types: list[str] = field(default_factory=list)
    p1_event_types: list[str] = field(default_factory=list)
    p2_event_types: list[str] = field(default_factory=list)


@dataclass
class RetentionConfig:
    enabled: bool = True
    interval_minutes: int = 60
    raw_events_days: int = 7
    sessions_days: int = 30
    routine_candidates_days: int = 90
    handoff_queue_days: int = 7
    daily_summaries_days: int = 180
    pattern_summaries_days: int = 60
    llm_inputs_days: int = 30
    max_db_mb: int = 500
    batch_size: int = 5000
    vacuum_hours: int = 24


@dataclass
class ObservabilityConfig:
    log_interval_sec: int = 60
    activity_log: bool = True
    activity_top_n: int = 3
    activity_min_duration_sec: int = 5
    activity_include_title: bool = False
    activity_title_apps: list[str] = field(default_factory=list)
    activity_title_max_len: int = 128


@dataclass
class LoggingConfig:
    dir: Path = PROJECT_ROOT / "logs"
    file_name: str = "collector.log"
    max_mb: int = 20
    backup_count: int = 10
    json: bool = True
    to_console: bool = True
    activity_detail_file: str = ""
    activity_detail_max_mb: int = 20
    activity_detail_backup_count: int = 10
    activity_detail_text_file: str = ""
    activity_detail_text_max_mb: int = 10
    activity_detail_text_backup_count: int = 5
    timezone: str = "local"
    include_run_id: bool = True
    prune_days: int = 0


@dataclass
class ActivityDetailConfig:
    enabled: bool = False
    min_duration_sec: int = 5
    store_hint: bool = True
    full_title_apps: list[str] = field(default_factory=list)
    max_title_len: int = 256


@dataclass
class SensorProcessConfig:
    module: str = ""
    args: list[str] = field(default_factory=list)
    enabled: bool = True


@dataclass
class SensorsConfig:
    auto_start: bool = False
    processes: list[SensorProcessConfig] = field(default_factory=list)


@dataclass
class PostCollectionConfig:
    enabled: bool = False
    run_sessions: bool = False
    run_routines: bool = False
    run_handoff: bool = False
    run_daily_summary: bool = True
    run_pattern_summary: bool = True
    run_llm_input: bool = True
    run_pattern_report: bool = False
    output_dir: str = ""
    llm_max_bytes: int = 8000
    session_gap_minutes: int = 15
    routine_days: int = 7
    routine_min_support: int = 2
    routine_n_min: int = 2
    routine_n_max: int = 3


@dataclass
class LLMConfig:
    enabled: bool = False
    endpoint: str = ""
    api_key_env: str = "LLM_API_KEY"
    model: str = ""
    timeout_sec: int = 20
    max_tokens: int = 500


@dataclass
class AutomationConfig:
    enabled: bool = False
    dry_run: bool = True
    allow_actions: list[str] = field(default_factory=list)
    min_confidence: float = 0.6


@dataclass
class Config:
    config_path: Path
    db_path: Path
    summary_db_path: Path | None
    migrations_path: Path
    validation_level: str = "lenient"
    wal_mode: bool = True
    log_level: str = "INFO"
    privacy_rules_path: Path = PROJECT_ROOT / "configs" / "privacy_rules.yaml"
    ingest: IngestConfig = field(default_factory=IngestConfig)
    queue: QueueConfig = field(default_factory=QueueConfig)
    privacy: PrivacyConfig = field(default_factory=PrivacyConfig)
    store: StoreConfig = field(default_factory=StoreConfig)
    encryption: EncryptionConfig = field(default_factory=EncryptionConfig)
    priority: PriorityConfig = field(default_factory=PriorityConfig)
    retention: RetentionConfig = field(default_factory=RetentionConfig)
    observability: ObservabilityConfig = field(default_factory=ObservabilityConfig)
    logging: LoggingConfig = field(default_factory=LoggingConfig)
    activity_detail: ActivityDetailConfig = field(default_factory=ActivityDetailConfig)
    sensors: SensorsConfig = field(default_factory=SensorsConfig)
    post_collection: PostCollectionConfig = field(default_factory=PostCollectionConfig)
    llm: LLMConfig = field(default_factory=LLMConfig)
    automation: AutomationConfig = field(default_factory=AutomationConfig)


def load_config(path: str | Path) -> Config:
    config_path = Path(path)
    if not config_path.exists():
        raise FileNotFoundError(f"config file not found: {config_path}")

    raw = yaml.safe_load(config_path.read_text()) or {}
    if not isinstance(raw, dict):
        raise ValueError("config root must be a mapping")

    db_path = _resolve_path(raw.get("db_path", "collector.db"))
    summary_db_path = raw.get("summary_db_path", "")
    summary_db_path = _resolve_path(summary_db_path) if summary_db_path else None
    migrations_path = _resolve_path(raw.get("migrations_path", "migrations"))
    privacy_rules_path = _resolve_path(
        raw.get("privacy_rules_path", "configs/privacy_rules.yaml")
    )

    ingest_raw = _as_dict(raw.get("ingest"))
    token_value = ingest_raw.get("token", "")
    ingest = IngestConfig(
        enabled=bool(ingest_raw.get("enabled", True)),
        host=str(ingest_raw.get("host", "127.0.0.1")),
        port=int(ingest_raw.get("port", 8080)),
        token=str(token_value) if token_value is not None else "",
    )

    queue_raw = _as_dict(raw.get("queue"))
    queue = QueueConfig(
        max_size=int(queue_raw.get("max_size", 1000)),
        shutdown_drain_seconds=int(queue_raw.get("shutdown_drain_seconds", 3)),
    )

    privacy_raw = _as_dict(raw.get("privacy"))
    privacy = PrivacyConfig(
        hash_salt=str(privacy_raw.get("hash_salt", "dev-salt")),
        url_mode=str(privacy_raw.get("url_mode", "rules")),
    )

    store_raw = _as_dict(raw.get("store"))
    store = StoreConfig(
        busy_timeout_ms=int(store_raw.get("busy_timeout_ms", 5000)),
        insert_batch_size=int(store_raw.get("insert_batch_size", 100)),
        insert_flush_ms=int(store_raw.get("insert_flush_ms", 1000)),
        insert_retry_attempts=int(store_raw.get("insert_retry_attempts", 3)),
        insert_retry_backoff_ms=int(store_raw.get("insert_retry_backoff_ms", 50)),
    )

    encryption_raw = _as_dict(raw.get("encryption"))
    key_path = str(encryption_raw.get("key_path", "")).strip()
    if key_path:
        key_path = str(_resolve_path(key_path))
    encryption = EncryptionConfig(
        enabled=bool(encryption_raw.get("enabled", False)),
        key_env=str(encryption_raw.get("key_env", "DATA_COLLECTOR_ENC_KEY")),
        key_path=key_path,
        encrypt_raw_json=bool(encryption_raw.get("encrypt_raw_json", False)),
    )

    priority_raw = _as_dict(raw.get("priority"))
    focus_event_types = priority_raw.get("focus_event_types", ["os.foreground_changed"])
    if not isinstance(focus_event_types, list):
        focus_event_types = [str(focus_event_types)]
    p0_event_types = priority_raw.get("p0_event_types", [])
    if not isinstance(p0_event_types, list):
        p0_event_types = [str(p0_event_types)]
    p1_event_types = priority_raw.get("p1_event_types", [])
    if not isinstance(p1_event_types, list):
        p1_event_types = [str(p1_event_types)]
    p2_event_types = priority_raw.get("p2_event_types", [])
    if not isinstance(p2_event_types, list):
        p2_event_types = [str(p2_event_types)]
    priority = PriorityConfig(
        debounce_seconds=float(priority_raw.get("debounce_seconds", 2.0)),
        focus_event_types=[str(item) for item in focus_event_types],
        focus_block_event_type=str(
            priority_raw.get("focus_block_event_type", "os.app_focus_block")
        ),
        drop_p2_when_queue_over=float(
            priority_raw.get("drop_p2_when_queue_over", 0.8)
        ),
        p0_event_types=[str(item) for item in p0_event_types],
        p1_event_types=[str(item) for item in p1_event_types],
        p2_event_types=[str(item) for item in p2_event_types],
    )

    retention_raw = _as_dict(raw.get("retention"))
    retention = RetentionConfig(
        enabled=bool(retention_raw.get("enabled", True)),
        interval_minutes=int(retention_raw.get("interval_minutes", 60)),
        raw_events_days=int(retention_raw.get("raw_events_days", 7)),
        sessions_days=int(retention_raw.get("sessions_days", 30)),
        routine_candidates_days=int(retention_raw.get("routine_candidates_days", 90)),
        handoff_queue_days=int(retention_raw.get("handoff_queue_days", 7)),
        daily_summaries_days=int(retention_raw.get("daily_summaries_days", 180)),
        pattern_summaries_days=int(retention_raw.get("pattern_summaries_days", 60)),
        llm_inputs_days=int(retention_raw.get("llm_inputs_days", 30)),
        max_db_mb=int(retention_raw.get("max_db_mb", 500)),
        batch_size=int(retention_raw.get("batch_size", 5000)),
        vacuum_hours=int(retention_raw.get("vacuum_hours", 24)),
    )

    observability_raw = _as_dict(raw.get("observability"))
    activity_title_apps = observability_raw.get("activity_title_apps", [])
    if not isinstance(activity_title_apps, list):
        activity_title_apps = [str(activity_title_apps)]
    observability = ObservabilityConfig(
        log_interval_sec=int(observability_raw.get("log_interval_sec", 60)),
        activity_log=bool(observability_raw.get("activity_log", True)),
        activity_top_n=int(observability_raw.get("activity_top_n", 3)),
        activity_min_duration_sec=int(
            observability_raw.get("activity_min_duration_sec", 5)
        ),
        activity_include_title=bool(
            observability_raw.get("activity_include_title", False)
        ),
        activity_title_apps=[str(item) for item in activity_title_apps],
        activity_title_max_len=int(observability_raw.get("activity_title_max_len", 128)),
    )

    logging_raw = _as_dict(raw.get("logging"))
    logging_config = LoggingConfig(
        dir=_resolve_path(logging_raw.get("dir", "logs")),
        file_name=str(logging_raw.get("file_name", "collector.log")),
        max_mb=int(logging_raw.get("max_mb", 20)),
        backup_count=int(logging_raw.get("backup_count", 10)),
        json=bool(logging_raw.get("json", True)),
        to_console=bool(logging_raw.get("to_console", True)),
        activity_detail_file=str(logging_raw.get("activity_detail_file", "")),
        activity_detail_max_mb=int(logging_raw.get("activity_detail_max_mb", 20)),
        activity_detail_backup_count=int(
            logging_raw.get("activity_detail_backup_count", 10)
        ),
        activity_detail_text_file=str(logging_raw.get("activity_detail_text_file", "")),
        activity_detail_text_max_mb=int(
            logging_raw.get("activity_detail_text_max_mb", 10)
        ),
        activity_detail_text_backup_count=int(
            logging_raw.get("activity_detail_text_backup_count", 5)
        ),
        timezone=str(logging_raw.get("timezone", "local")),
        include_run_id=bool(logging_raw.get("include_run_id", True)),
        prune_days=int(logging_raw.get("prune_days", 0)),
    )

    detail_raw = _as_dict(raw.get("activity_detail"))
    full_title_apps = detail_raw.get("full_title_apps", [])
    if not isinstance(full_title_apps, list):
        full_title_apps = [str(full_title_apps)]
    activity_detail = ActivityDetailConfig(
        enabled=bool(detail_raw.get("enabled", False)),
        min_duration_sec=int(detail_raw.get("min_duration_sec", 5)),
        store_hint=bool(detail_raw.get("store_hint", True)),
        full_title_apps=[str(item) for item in full_title_apps],
        max_title_len=int(detail_raw.get("max_title_len", 256)),
    )

    sensors_raw = _as_dict(raw.get("sensors"))
    process_items = sensors_raw.get("processes", []) or []
    processes: list[SensorProcessConfig] = []
    if isinstance(process_items, list):
        for item in process_items:
            if not isinstance(item, dict):
                continue
            module = str(item.get("module", "")).strip()
            if not module:
                continue
            raw_args = item.get("args", []) or []
            args: list[str] = []
            if isinstance(raw_args, list):
                args = [str(arg) for arg in raw_args]
            elif isinstance(raw_args, str):
                args = [raw_args]
            processes.append(
                SensorProcessConfig(
                    module=module,
                    args=args,
                    enabled=bool(item.get("enabled", True)),
                )
            )
    sensors = SensorsConfig(
        auto_start=bool(sensors_raw.get("auto_start", False)),
        processes=processes,
    )

    post_raw = _as_dict(raw.get("post_collection"))
    post_collection = PostCollectionConfig(
        enabled=bool(post_raw.get("enabled", False)),
        run_sessions=bool(post_raw.get("run_sessions", False)),
        run_routines=bool(post_raw.get("run_routines", False)),
        run_handoff=bool(post_raw.get("run_handoff", False)),
        run_daily_summary=bool(post_raw.get("run_daily_summary", True)),
        run_pattern_summary=bool(post_raw.get("run_pattern_summary", True)),
        run_llm_input=bool(post_raw.get("run_llm_input", True)),
        run_pattern_report=bool(post_raw.get("run_pattern_report", False)),
        output_dir=str(post_raw.get("output_dir", "")),
        llm_max_bytes=int(post_raw.get("llm_max_bytes", 8000)),
        session_gap_minutes=int(post_raw.get("session_gap_minutes", 15)),
        routine_days=int(post_raw.get("routine_days", 7)),
        routine_min_support=int(post_raw.get("routine_min_support", 2)),
        routine_n_min=int(post_raw.get("routine_n_min", 2)),
        routine_n_max=int(post_raw.get("routine_n_max", 3)),
    )

    llm_raw = _as_dict(raw.get("llm"))
    llm = LLMConfig(
        enabled=bool(llm_raw.get("enabled", False)),
        endpoint=str(llm_raw.get("endpoint", "")),
        api_key_env=str(llm_raw.get("api_key_env", "LLM_API_KEY")),
        model=str(llm_raw.get("model", "")),
        timeout_sec=int(llm_raw.get("timeout_sec", 20)),
        max_tokens=int(llm_raw.get("max_tokens", 500)),
    )

    automation_raw = _as_dict(raw.get("automation"))
    allow_actions = automation_raw.get("allow_actions", [])
    if not isinstance(allow_actions, list):
        allow_actions = [str(allow_actions)]
    automation = AutomationConfig(
        enabled=bool(automation_raw.get("enabled", False)),
        dry_run=bool(automation_raw.get("dry_run", True)),
        allow_actions=[str(item) for item in allow_actions],
        min_confidence=float(automation_raw.get("min_confidence", 0.6)),
    )

    return Config(
        config_path=config_path,
        db_path=db_path,
        summary_db_path=summary_db_path,
        migrations_path=migrations_path,
        validation_level=str(raw.get("validation_level", "lenient")),
        wal_mode=bool(raw.get("wal_mode", True)),
        log_level=str(raw.get("log_level", "INFO")),
        privacy_rules_path=privacy_rules_path,
        ingest=ingest,
        queue=queue,
        privacy=privacy,
        store=store,
        encryption=encryption,
        priority=priority,
        retention=retention,
        observability=observability,
        logging=logging_config,
        activity_detail=activity_detail,
        sensors=sensors,
        post_collection=post_collection,
        llm=llm,
        automation=automation,
    )


def _resolve_path(value: str) -> Path:
    path = Path(value)
    if path.is_absolute():
        return path
    return PROJECT_ROOT / path


def _as_dict(value: Any) -> Dict[str, Any]:
    if isinstance(value, dict):
        return value
    return {}
