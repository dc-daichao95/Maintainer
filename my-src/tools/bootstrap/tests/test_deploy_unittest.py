import unittest
from unittest.mock import patch, MagicMock
import subprocess
import sys
import os
import time
import signal
import threading
import json
import deploy

class TestDeployRunProject(unittest.TestCase):
    def setUp(self):
        self.mock_script = os.path.abspath("mock_script.py")
        with open(self.mock_script, "w") as f:
            f.write("""import time
import sys
import signal

def handler(signum, frame):
    print("Mock script terminated gracefully")
    sys.exit(0)

signal.signal(signal.SIGINT, handler)
signal.signal(signal.SIGTERM, handler)

print("Mock script started")
sys.stdout.flush()
for i in range(5):
    time.sleep(1)
    print(f"Mock script running {i}")
    sys.stdout.flush()
""")

    def tearDown(self):
        if os.path.exists(self.mock_script):
            os.remove(self.mock_script)

    @patch('deploy.get_cargo_path')
    def test_concurrent_execution(self, mock_cargo_path):
        mock_cargo_path.return_value = sys.executable
        
            # We will mock the arguments passed to Popen so it runs our mock script instead of cargo run
        original_popen = subprocess.Popen
        
        processes = []
        def mock_popen(*args, **kwargs):
            # Change the command to run our mock script
            new_args = [sys.executable, self.mock_script]
            p = original_popen(new_args, **kwargs)
            processes.append(p)
            return p
            
        sleep_called = [False]
        def mock_sleep(seconds):
            if not sleep_called[0]:
                sleep_called[0] = True
                raise KeyboardInterrupt()
            else:
                time.sleep(seconds)

        with patch('subprocess.Popen', side_effect=mock_popen):
            with patch('time.sleep', side_effect=mock_sleep):
                # Run project with a config that triggers 2 processes
                config = {"monitor": {"branches": ["main"]}}
                
                try:
                    deploy.run_project(".", config)
                except KeyboardInterrupt:
                    pass
                
                self.assertEqual(len(processes), 2)
                
                for p in processes:
                    p.poll()
                    self.assertIsNotNone(p.returncode, "Child process was not terminated")

class TestDeployConfigGeneration(unittest.TestCase):
    def setUp(self):
        self.monitor_config_path = os.path.join(os.path.dirname(os.path.abspath(deploy.__file__)), "..", "repo_monitor", "monitor_config.json")
        self.original_content = None
        if os.path.exists(self.monitor_config_path):
            with open(self.monitor_config_path, "r", encoding="utf-8") as f:
                self.original_content = f.read()

    def tearDown(self):
        if self.original_content is not None:
            with open(self.monitor_config_path, "w", encoding="utf-8") as f:
                f.write(self.original_content)
        elif os.path.exists(self.monitor_config_path):
            os.remove(self.monitor_config_path)

    def test_generate_monitor_config_new(self):
        if os.path.exists(self.monitor_config_path):
            os.remove(self.monitor_config_path)
            
        config = {
            "app_config": {"server": {"port": 9999}},
            "monitor": {"branches": ["test-branch"]}
        }
        deploy.generate_monitor_config(".", config)
        
        with open(self.monitor_config_path, "r", encoding="utf-8") as f:
            data = json.load(f)
            
        self.assertEqual(data["server_port"], 9999)
        self.assertEqual(data["branches"], ["test-branch"])
        self.assertEqual(data["server_ip"], "127.0.0.1")
        self.assertEqual(data["pull_interval_sec"], 3600)

    def test_generate_monitor_config_existing(self):
        with open(self.monitor_config_path, "w", encoding="utf-8") as f:
            json.dump({
                "pull_interval_sec": 1234,
                "max_retries": 5,
                "server_port": 8888,
                "branches": ["old-branch"]
            }, f)
            
        config = {
            "app_config": {"server": {"port": 7777}},
            "monitor": {"branches": ["new-branch"]}
        }
        deploy.generate_monitor_config(".", config)
        
        with open(self.monitor_config_path, "r", encoding="utf-8") as f:
            data = json.load(f)
            
        self.assertEqual(data["server_port"], 7777)
        self.assertEqual(data["branches"], ["new-branch"])
        self.assertEqual(data["pull_interval_sec"], 1234)
        self.assertEqual(data["max_retries"], 5)

if __name__ == '__main__':
    unittest.main()
