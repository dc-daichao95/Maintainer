import deploy


class ReadyHealthChecker:
    def check_ready(self, base_url):
        return True


class SequenceHealthChecker:
    def __init__(self, results):
        self.results = list(results)
        self.checked_urls = []

    def check_ready(self, base_url):
        self.checked_urls.append(base_url)
        if not self.results:
            return False
        return self.results.pop(0)


class RecordingRunner:
    def __init__(self):
        self.commands = []

    def start(self, command):
        self.commands.append(command)


def test_ensure_headroom_running_reuses_ready_proxy():
    config = deploy.HeadroomConfig(enabled=True)
    runner = RecordingRunner()

    status = deploy.ensure_headroom_running(
        config,
        health_checker=ReadyHealthChecker(),
        runner=runner,
    )

    assert runner.commands == []
    assert status.running is True
    assert status.ready is True
    assert status.base_url == "http://127.0.0.1:8787/v1"
    assert status.stats_url == "http://127.0.0.1:8787/stats"


def test_ensure_headroom_running_starts_when_not_ready():
    config = deploy.HeadroomConfig(
        enabled=True,
        host="127.0.0.1",
        port=8788,
        mode="token",
        backend="openrouter",
        telemetry=False,
        startup_timeout_secs=1,
    )
    runner = RecordingRunner()
    health_checker = SequenceHealthChecker([False, True])

    status = deploy.ensure_headroom_running(
        config,
        health_checker=health_checker,
        runner=runner,
        sleeper=lambda _seconds: None,
    )

    assert runner.commands == [[
        "headroom",
        "proxy",
        "--host",
        "127.0.0.1",
        "--port",
        "8788",
        "--mode",
        "token",
        "--backend",
        "openrouter",
        "--no-telemetry",
    ]]
    assert status.ready is True
    assert status.base_url == "http://127.0.0.1:8788/v1"


def test_ensure_headroom_running_reports_missing_command():
    class MissingRunner:
        def start(self, command):
            raise FileNotFoundError("headroom")

    config = deploy.HeadroomConfig(enabled=True)

    try:
        deploy.ensure_headroom_running(
            config,
            health_checker=SequenceHealthChecker([False]),
            runner=MissingRunner(),
            sleeper=lambda _seconds: None,
        )
    except deploy.ExecutionError as error:
        assert "Install Headroom" in str(error)
    else:
        raise AssertionError("Expected ExecutionError")


def test_ensure_headroom_running_fails_after_timeout():
    config = deploy.HeadroomConfig(enabled=True, startup_timeout_secs=1)

    try:
        deploy.ensure_headroom_running(
            config,
            health_checker=SequenceHealthChecker([False, False, False]),
            runner=RecordingRunner(),
            sleeper=lambda _seconds: None,
        )
    except deploy.ExecutionError as error:
        assert "http://127.0.0.1:8787/readyz" in str(error)
        assert "Check Headroom logs" in str(error)
    else:
        raise AssertionError("Expected ExecutionError")
