import deploy
import json
import pytest
from pathlib import Path


def test_headroom_config_defaults_to_disabled():
    config = deploy.parse_headroom_config({})

    assert config.enabled is False
    assert config.host == "127.0.0.1"
    assert config.port == 8787
    assert config.backend == "openrouter"
    assert config.telemetry is False


def test_headroom_config_rejects_invalid_port():
    raw_config = {"headroom": {"enabled": True, "port": 70000}}

    with pytest.raises(deploy.ValidationError, match="headroom.port"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_requires_positive_startup_timeout_when_enabled():
    raw_config = {"headroom": {"enabled": True, "startup_timeout_secs": 0}}

    with pytest.raises(deploy.ValidationError, match="startup_timeout_secs"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_empty_host_when_enabled():
    raw_config = {"headroom": {"enabled": True, "host": ""}}

    with pytest.raises(deploy.ValidationError, match="headroom.host"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_empty_backend_when_enabled():
    raw_config = {"headroom": {"enabled": True, "backend": ""}}

    with pytest.raises(deploy.ValidationError, match="headroom.backend"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_object_section():
    raw_config = {"headroom": "enabled"}

    with pytest.raises(deploy.ValidationError, match="headroom must be an object"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_string_host_when_enabled():
    raw_config = {"headroom": {"enabled": True, "host": 1234}}

    with pytest.raises(deploy.ValidationError, match="headroom.host"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_string_backend_when_enabled():
    raw_config = {"headroom": {"enabled": True, "backend": ["openrouter"]}}

    with pytest.raises(deploy.ValidationError, match="headroom.backend"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_invalid_mode_when_enabled():
    raw_config = {"headroom": {"enabled": True, "mode": ""}}

    with pytest.raises(deploy.ValidationError, match="headroom.mode"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_bool_flags():
    raw_config = {"headroom": {"enabled": "yes", "telemetry": "off"}}

    with pytest.raises(deploy.ValidationError, match="headroom.enabled"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_bool_telemetry():
    raw_config = {"headroom": {"enabled": True, "telemetry": "off"}}

    with pytest.raises(deploy.ValidationError, match="headroom.telemetry"):
        deploy.parse_headroom_config(raw_config)


def test_headroom_config_rejects_non_int_startup_timeout():
    raw_config = {"headroom": {"enabled": True, "startup_timeout_secs": "20"}}

    with pytest.raises(deploy.ValidationError, match="startup_timeout_secs"):
        deploy.parse_headroom_config(raw_config)


def test_config_template_declares_headroom_defaults():
    template_path = Path(deploy.__file__).with_name("config_template.json")
    template = json.loads(template_path.read_text(encoding="utf-8"))

    assert template["headroom"]["enabled"] is False
    assert template["headroom"]["host"] == "127.0.0.1"
    assert template["headroom"]["port"] == 8787
    assert template["headroom"]["telemetry"] is False
