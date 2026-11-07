# -*- coding: utf-8 -*-
import base64, json, pathlib, sys

orig = pathlib.Path("./tmp/README.orig.md").read_text(encoding="utf-8")
sha_path = pathlib.Path("./tmp/README.sha")
exists_path = pathlib.Path("./tmp/README.exists")
exists = exists_path.exists() and exists_path.read_text(encoding="utf-8").strip() == "yes"
sha = sha_path.read_text(encoding="utf-8").strip() if exists and sha_path.exists() else None
if not sha:
    sha = None

SECTION = """## 参与开发

> **给外部贡献者的重要说明**
>
> `main` 分支受保护，且仅维护者可直接推送。
> **请向 `dev` 分支提交 Pull Request；直接向 `main` 提交的 PR 大概率不会受理。**

推荐流程：

1. Fork 本仓库到你自己账号
2. 从 `dev` 分支切出特性分支（`git checkout -b feature/xxx`）
3. 提交改动并推送到你的 Fork
4. 向本仓库的 **`dev` 分支**开 Pull Request，等待 CI 通过与合并

分支流向：`feature/xxx` → `dev`（日常集成）→ `main`（发布版本）
"""

if "## 参与开发" in orig:
    sys.exit("README 已含「参与开发」章节，跳过。")

pos, hit = None, None
for a in ("## 开发与部署", "## 部署", "## 构建",
          "## Development", "## Build", "## Deployment",
          "## 构建与运行", "## License"):
    i = orig.find(a)
    if i != -1:
        pos, hit = i, a
        break

if pos is None:
    print("WARN: 没命中任何锚点，插入到文末")
    merged = orig.rstrip("\n") + "\n\n" + SECTION
else:
    print(f"锚点命中: {orig[pos:pos + 30]!r}")
    merged = orig[:pos] + SECTION + "\n" + orig[pos:]

payload = {
    "message": "docs: add contributing guide",
    "content": base64.b64encode(merged.encode("utf-8")).decode(),
    "branch":  "docs/contributing-guide",
}
if sha is not None:
    payload["sha"] = sha
pathlib.Path("./tmp/readme.json").write_text(
    json.dumps(payload, ensure_ascii=False), encoding="utf-8")
print("payload 已写入 ./tmp/readme.json")
