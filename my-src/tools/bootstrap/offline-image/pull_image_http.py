"""通过 Registry HTTP API（标准库 + curl）拉取容器镜像并打包成 docker load 兼容的 tar。

适用于**本机无 Docker**、且网络会**重置大文件传输**的受限环境：blob 层用 curl 的
断点续传（`-C -`）循环逐段拉取，被重置就重连续传，从而把大层"啃"下来。产物为 legacy
docker-archive 格式 tar，拷到离线 Linux 用 `docker load -i <tar>` 导入。

用法：
  python pull_image_http.py --image ghcr.io/chopratejas/headroom:latest \
    --platform linux/amd64 --output headroom-offline.tar

manifest/token 用标准库 urllib；blob 下载用系统 curl（断点续传更稳）。
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

_MANIFEST_ACCEPT = ", ".join([
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.oci.image.manifest.v1+json",
])
_INDEX_TYPES = {
    "application/vnd.docker.distribution.manifest.list.v2+json",
    "application/vnd.oci.image.index.v1+json",
}
_CHUNK_MAX_SECONDS = 120
_MAX_NO_PROGRESS = 8


def parse_image_ref(image):
    """拆出 (registry, repository, reference)。默认 registry 为 ghcr.io。"""
    ref = "latest"
    name = image
    if ":" in image.rsplit("/", 1)[-1]:
        name, ref = image.rsplit(":", 1)
    parts = name.split("/", 1)
    if "." in parts[0] or ":" in parts[0]:
        return parts[0], parts[1], ref
    return "ghcr.io", name, ref


def get_token(registry, repository):
    """获取匿名 pull token（公共镜像）。"""
    url = f"https://{registry}/token?service={registry}&scope=repository:{repository}:pull"
    with urllib.request.urlopen(url, timeout=60) as resp:
        return json.load(resp).get("token", "")


def fetch_manifest(registry, repository, reference, token):
    """返回 (manifest_dict, content_type)。"""
    url = f"https://{registry}/v2/{repository}/manifests/{reference}"
    req = urllib.request.Request(url, headers={
        "Authorization": f"Bearer {token}",
        "Accept": _MANIFEST_ACCEPT,
    })
    with urllib.request.urlopen(req, timeout=120) as resp:
        raw = resp.read()
        ctype = resp.headers.get("Content-Type", "").split(";")[0].strip()
    return json.loads(raw), ctype


def select_platform_digest(index, platform):
    """从 manifest list / OCI index 中挑选目标平台的子 manifest digest。"""
    want_os, want_arch = platform.split("/", 1)
    for m in index.get("manifests", []):
        plat = m.get("platform", {})
        if plat.get("os") == want_os and plat.get("architecture") == want_arch:
            return m["digest"]
    raise SystemExit(f"ERROR: 镜像不包含平台 {platform}")


def download_blob_resumable(registry, repository, digest, expected_size, dest_path, label):
    """用 curl 断点续传循环把 blob 拉满，绕过大文件传输被重置的限制。"""
    url = f"https://{registry}/v2/{repository}/blobs/{digest}"
    no_progress = 0
    while True:
        have = os.path.getsize(dest_path) if os.path.exists(dest_path) else 0
        if expected_size and have >= expected_size:
            break
        token = get_token(registry, repository)  # 每轮刷新，避免长下载 token 过期
        args = ["curl.exe", "-sS", "-L", "--max-time", str(_CHUNK_MAX_SECONDS),
                "--connect-timeout", "30", "-H", f"Authorization: Bearer {token}",
                "-o", dest_path, url]
        if have > 0:
            args[1:1] = ["-C", "-"]  # 断点续传
        subprocess.run(args, capture_output=True, text=True)
        now = os.path.getsize(dest_path) if os.path.exists(dest_path) else 0
        pct = (100.0 * now / expected_size) if expected_size else 0
        print(f"  {label}: {now/1024/1024:.1f}/{expected_size/1024/1024:.1f} MB ({pct:.0f}%)",
              flush=True)
        if now <= have:
            no_progress += 1
            if no_progress >= _MAX_NO_PROGRESS:
                raise SystemExit(f"ERROR: {label} 连续 {_MAX_NO_PROGRESS} 次无进展，放弃。")
            time.sleep(3)
        else:
            no_progress = 0
        if not expected_size:  # 无法判断完整性时，单次成功即止
            break


def _write_json(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f)


def assemble_layout(workdir, registry, repository, image_ref, manifest):
    """下载 config + layers 并组装 legacy docker-archive 结构，返回 manifest.json 列表。"""
    cfg = manifest["config"]
    config_name = cfg["digest"].split(":", 1)[1] + ".json"
    download_blob_resumable(registry, repository, cfg["digest"], cfg.get("size", 0),
                            os.path.join(workdir, config_name), "config")

    layer_files = []
    parent_id = ""
    layers = manifest["layers"]
    for i, layer in enumerate(layers):
        digest = layer["digest"]
        fake_id = hashlib.sha256((parent_id + "\n" + digest + "\n").encode()).hexdigest()
        layer_dir = os.path.join(workdir, fake_id)
        os.makedirs(layer_dir, exist_ok=True)
        with open(os.path.join(layer_dir, "VERSION"), "w", encoding="utf-8") as f:
            f.write("1.0")
        download_blob_resumable(registry, repository, digest, layer.get("size", 0),
                                os.path.join(layer_dir, "layer.tar"), f"layer {i+1}/{len(layers)}")
        _write_json(os.path.join(layer_dir, "json"), {"id": fake_id, "parent": parent_id or None})
        layer_files.append(f"{fake_id}/layer.tar")
        parent_id = fake_id

    return [{"Config": config_name, "RepoTags": [image_ref], "Layers": layer_files}]


def pull(image, platform, output, workdir):
    registry, repository, reference = parse_image_ref(image)
    image_ref = f"{registry}/{repository}:{reference}"
    print(f"==> 镜像 {image_ref}  平台 {platform}", flush=True)
    token = get_token(registry, repository)

    manifest, ctype = fetch_manifest(registry, repository, reference, token)
    if ctype in _INDEX_TYPES or "manifests" in manifest:
        digest = select_platform_digest(manifest, platform)
        print(f"==> 选定平台 manifest: {digest[:19]}", flush=True)
        manifest, ctype = fetch_manifest(registry, repository, digest, token)

    os.makedirs(workdir, exist_ok=True)
    manifest_json = assemble_layout(workdir, registry, repository, image_ref, manifest)
    _write_json(os.path.join(workdir, "manifest.json"), manifest_json)

    print(f"==> 打包到 {output}", flush=True)
    with tarfile.open(output, "w") as tar:
        for name in sorted(os.listdir(workdir)):
            tar.add(os.path.join(workdir, name), arcname=name)
    size_mb = os.path.getsize(output) / 1024 / 1024
    print(f"[OK] 完成：{output}  ({size_mb:.1f} MB)", flush=True)
    print(f"  离线 Linux 上：docker load -i {os.path.basename(output)}", flush=True)


def main():
    parser = argparse.ArgumentParser(description="HTTP+curl resumable image puller (no Docker).")
    parser.add_argument("--image", default="ghcr.io/chopratejas/headroom:latest")
    parser.add_argument("--platform", default="linux/amd64")
    parser.add_argument("--output", default="headroom-offline.tar")
    parser.add_argument("--workdir", default="", help="续传缓存目录（默认临时目录；指定后可断点续跑）")
    args = parser.parse_args()

    workdir = args.workdir or tempfile.mkdtemp(prefix="img-pull-")
    keep = bool(args.workdir)
    try:
        pull(args.image, args.platform, args.output, workdir)
    except urllib.error.URLError as e:
        print(f"ERROR: 网络/Registry 访问失败: {e}", file=sys.stderr)
        return 1
    finally:
        if not keep:
            shutil.rmtree(workdir, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
