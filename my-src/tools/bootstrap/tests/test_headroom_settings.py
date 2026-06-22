import deploy


def test_write_settings_points_openai_compat_to_headroom(tmp_path):
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text(
        """# Existing settings
[server]
port = 3000
""",
        encoding="utf-8",
    )
    status = deploy.HeadroomStatus(
        running=True,
        ready=True,
        base_url="http://127.0.0.1:8787/v1",
        stats_url="http://127.0.0.1:8787/stats",
    )
    app_config = {
        "server": {"port": 8080},
        "ai": {
            "model": "gpt-4o",
            "streaming": True,
            "stream_idle_timeout_secs": 240,
            "api_timeout_secs": 300,
        },
    }

    deploy.update_settings_toml(str(tmp_path), app_config, headroom_status=status)

    updated_content = settings_file.read_text(encoding="utf-8")
    assert "# Existing settings" in updated_content
    assert "port = 8080" in updated_content
    assert 'provider = "openai-compatible"' in updated_content
    assert 'model = "gpt-4o"' in updated_content
    assert "api_timeout_secs = 300" in updated_content
    assert 'base_url = "http://127.0.0.1:8787/v1"' in updated_content
    assert "streaming = true" in updated_content
    assert "stream_idle_timeout_secs = 240" in updated_content


def test_headroom_disabled_keeps_existing_provider_settings(tmp_path):
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text(
        """[ai]
provider = "gemini"
model = "gemini-3.1-pro-preview"
""",
        encoding="utf-8",
    )

    deploy.update_settings_toml(str(tmp_path), {"ai": {"model": "gpt-4o"}})

    updated_content = settings_file.read_text(encoding="utf-8")
    assert 'provider = "gemini"' in updated_content
    assert 'model = "gemini-3.1-pro-preview"' in updated_content
    assert "openai_compat" not in updated_content


def test_headroom_logs_do_not_expose_api_key(tmp_path, capsys):
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text("[server]\nport = 3000\n", encoding="utf-8")
    status = deploy.HeadroomStatus(
        running=True,
        ready=True,
        base_url="http://127.0.0.1:8787/v1",
        stats_url="http://127.0.0.1:8787/stats",
    )

    deploy.update_settings_toml(
        str(tmp_path),
        {"ai": {"openai_key": "sk-secret-value", "model": "gpt-4o"}},
        headroom_status=status,
    )

    captured = capsys.readouterr()
    assert "sk-secret-value" not in captured.out
    assert "sk-secret-value" not in captured.err
    assert "OPENAI_API_KEY" not in captured.out
    assert "LLM_API_KEY" not in captured.out
