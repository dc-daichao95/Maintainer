"""离线 wheelhouse 填充工具。

在**能访问内网镜像的目标机**上运行一次，把 ``maturin`` 及 Headroom ``[proxy]``
的全部传递依赖下载为 wheel，放入 ``wheelhouse_dir``。之后 ``deploy.py`` 的
source-vendor 流程（``pip --no-index --find-links``）即可在离线环境直接安装启动。

注意：wheel 与平台/Python 版本绑定，必须在与部署目标一致的机器上执行本脚本。

Rust 侧的离线依赖（``cargo build`` 编译 ``headroom._core`` 所需 crates）不在本脚本
范围内——离线编译需配置内网 cargo 镜像或预先 ``cargo vendor``。
"""

import os
import subprocess

from deploy import (
    ExecutionError,
    HeadroomConfig,
    SubprocessCommandRunner,
    resolve_repo_path,
)


def _index_args(index_url=None, trusted_host=None, extra_index_url=None):
    args = []
    if index_url:
        args += ["--index-url", index_url]
    if extra_index_url:
        args += ["--extra-index-url", extra_index_url]
    if trusted_host:
        args += ["--trusted-host", trusted_host]
    return args


def build_maturin_download_command(
    python_executable, wheelhouse_dir, index_url=None, trusted_host=None, extra_index_url=None
):
    """构建下载 maturin（构建后端）到 wheelhouse 的 pip download 命令。"""
    return [
        python_executable, "-m", "pip", "download", "maturin", "-d", wheelhouse_dir,
    ] + _index_args(index_url, trusted_host, extra_index_url)


def build_source_deps_download_command(
    python_executable, source_dir, wheelhouse_dir,
    index_url=None, trusted_host=None, extra_index_url=None,
):
    """构建下载 Headroom ``[proxy]`` 传递依赖到 wheelhouse 的 pip download 命令。"""
    return [
        python_executable, "-m", "pip", "download", f"{source_dir}[proxy]", "-d", wheelhouse_dir,
    ] + _index_args(index_url, trusted_host, extra_index_url)


def fill_wheelhouse(
    source_dir, wheelhouse_dir, python_executable="python",
    index_url=None, trusted_host=None, extra_index_url=None, runner=None,
):
    """下载 maturin 与 ``[proxy]`` 依赖到 wheelhouse，使离线安装成为可能。"""
    runner = runner or SubprocessCommandRunner()
    os.makedirs(wheelhouse_dir, exist_ok=True)

    # maturin 必须先就位：后续解析源码依赖时构建隔离需要它。
    commands = [
        build_maturin_download_command(
            python_executable, wheelhouse_dir, index_url, trusted_host, extra_index_url
        ),
        build_source_deps_download_command(
            python_executable, source_dir, wheelhouse_dir, index_url, trusted_host, extra_index_url
        ),
    ]
    for command in commands:
        try:
            runner.run(command)
        except FileNotFoundError as e:
            raise ExecutionError(f"pip download command not found: {command[0]}") from e
        except subprocess.CalledProcessError as e:
            raise ExecutionError(
                f"pip download failed: {' '.join(map(str, command))}"
            ) from e


def main():
    import argparse

    defaults = HeadroomConfig()
    parser = argparse.ArgumentParser(
        description="Download maturin and Headroom [proxy] deps into the offline wheelhouse."
    )
    parser.add_argument("--index-url", help="内网 PyPI 镜像 index URL")
    parser.add_argument("--extra-index-url", help="额外的 index URL")
    parser.add_argument("--trusted-host", help="镜像主机名（http 或自签证书时需要）")
    parser.add_argument(
        "--source-dir",
        default=resolve_repo_path(defaults.source_dir),
        help="Headroom 源码目录",
    )
    parser.add_argument(
        "--wheelhouse-dir",
        default=resolve_repo_path(defaults.wheelhouse_dir),
        help="wheelhouse 输出目录",
    )
    parser.add_argument("--python-executable", default="python", help="用于 pip download 的 Python")
    args = parser.parse_args()

    try:
        fill_wheelhouse(
            source_dir=args.source_dir,
            wheelhouse_dir=args.wheelhouse_dir,
            python_executable=args.python_executable,
            index_url=args.index_url,
            trusted_host=args.trusted_host,
            extra_index_url=args.extra_index_url,
        )
    except ExecutionError as e:
        print(f"Error: {e}", file=__import__("sys").stderr)
        return 1
    print(f"Wheelhouse filled at {args.wheelhouse_dir}")
    return 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
