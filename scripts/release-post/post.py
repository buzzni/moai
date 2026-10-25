#!/usr/bin/env python3
"""release-posts/<판>/ 의 글과 그림을 Discord 웹훅으로 올린다.

    scripts/release-post/post.py release-posts/v0.23.0 --dry-run          무엇을 보낼지만 찍는다
    scripts/release-post/post.py release-posts/v0.23.0 --preview p.html   미리보기 한 장을 쓴다
    DISCORD_WEBHOOK_URL=… scripts/release-post/post.py release-posts/v0.23.0

한 판의 디렉터리는 이렇게 생겼다.

    post.md            첫 메시지. compose.py 가 짓고 사람이 다듬어도 된다. 2,000자 안
    media/01-keys.png  그다음 메시지마다 그림이나 영상 하나. 이름 차례로 나간다
    media/01-keys.md   그 그림에 붙는 한 줄 (없어도 된다)

웹훅 주소는 GitHub 저장소 시크릿 `DISCORD_WEBHOOK_URL` 이 환경으로 건넨다. 그 주소를 쥔 사람은 누구나
그 채널에 글을 올릴 수 있으니, 이 스크립트는 주소를 어디에도 찍지 않는다 — 거절문에도 안 싣는다.
글은 아무도 부르지 않는다(`allowed_mentions` 를 비운다) — 릴리스 노트 속 `@` 하나가 채널 전체를 부르면 안 된다.
"""
import argparse
import base64
import html
import json
import mimetypes
import os
import re
import sys
import time
import urllib.error
import urllib.request
import uuid

DISCORD_LIMIT = 2000
MAX_FILE = 10 * 1024 * 1024  # 부스트 없는 서버의 올림 상한
MEDIA = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".webm", ".mp4", ".mov"}
HOOK = re.compile(r"^https://(?:(?:ptb|canary)\.)?discord(?:app)?\.com/api/webhooks/\d+/[\w-]+$")
SUPPRESS_EMBEDS = 4  # 링크 미리보기를 끈다 — 릴리스 페이지 카드가 글보다 크게 서지 않게
AGENT = "DiscordBot (https://github.com/buzzni/moai, 1)"


class PostError(Exception):
    pass


def messages(directory):
    """보낼 메시지 차례. 하나하나는 {"content": 글, "file": 경로 또는 None} 이다."""
    path = os.path.join(directory, "post.md")
    if not os.path.isfile(path):
        raise PostError(f"{path} is missing — write it with compose.py first")
    with open(path, encoding="utf-8") as f:
        text = f.read().strip()
    if not text:
        raise PostError(f"{path} is empty")
    if len(text) > DISCORD_LIMIT:
        raise PostError(f"{path} is {len(text)} characters; Discord takes {DISCORD_LIMIT} in one message")
    out = [{"content": text, "file": None}]
    media = os.path.join(directory, "media")
    names = sorted(os.listdir(media)) if os.path.isdir(media) else []
    for name in names:
        stem, ext = os.path.splitext(name)
        if name.startswith(".") or ext == ".md":
            continue
        full = os.path.join(media, name)
        if ext.lower() not in MEDIA:
            raise PostError(f"{full}: Discord shows {', '.join(sorted(MEDIA))} inline — not {ext or 'a file with no extension'}")
        size = os.path.getsize(full)
        if size > MAX_FILE:
            raise PostError(f"{full} is {size / 1048576:.1f} MiB; a webhook takes {MAX_FILE // 1048576} MiB per file")
        caption = ""
        cap = os.path.join(media, stem + ".md")
        if os.path.isfile(cap):
            with open(cap, encoding="utf-8") as f:
                caption = f.read().strip()
            if len(caption) > DISCORD_LIMIT:
                raise PostError(f"{cap} is longer than one Discord message")
        out.append({"content": caption, "file": full})
    return out


def webhook_url(env):
    url = (env.get("DISCORD_WEBHOOK_URL") or "").strip()
    if not url:
        raise PostError("DISCORD_WEBHOOK_URL is not set")
    if not HOOK.match(url):
        raise PostError("DISCORD_WEBHOOK_URL is not a Discord webhook address (https://discord.com/api/webhooks/…)")
    return url


def encode(message):
    payload = {"content": message["content"], "allowed_mentions": {"parse": []}, "flags": SUPPRESS_EMBEDS}
    if not message["file"]:
        return json.dumps(payload).encode(), "application/json"
    name = os.path.basename(message["file"])
    payload["attachments"] = [{"id": 0, "filename": name}]
    with open(message["file"], "rb") as f:
        data = f.read()
    kind = mimetypes.guess_type(name)[0] or "application/octet-stream"
    if name.lower().endswith(".webm"):
        kind = "video/webm"
    bound = uuid.uuid4().hex
    body = b"".join([
        f'--{bound}\r\nContent-Disposition: form-data; name="payload_json"\r\nContent-Type: application/json\r\n\r\n'.encode(),
        json.dumps(payload).encode(), b"\r\n",
        f'--{bound}\r\nContent-Disposition: form-data; name="files[0]"; filename="{name}"\r\nContent-Type: {kind}\r\n\r\n'.encode(),
        data, f"\r\n--{bound}--\r\n".encode(),
    ])
    return body, f"multipart/form-data; boundary={bound}"


def send(url, msgs, opener=urllib.request.urlopen, sleep=time.sleep):
    for i, m in enumerate(msgs, 1):
        body, kind = encode(m)
        for attempt in range(6):
            req = urllib.request.Request(url + "?wait=true", data=body, method="POST",
                                         headers={"Content-Type": kind, "User-Agent": AGENT})
            try:
                with opener(req, timeout=60) as r:
                    r.read()
                break
            except urllib.error.HTTPError as e:
                said = e.read().decode("utf-8", "replace")
                if e.code == 429 and attempt < 5:
                    try:
                        wait = float(json.loads(said).get("retry_after", 1))
                    except (ValueError, AttributeError):
                        wait = 1.0
                    sleep(wait)
                    continue
                done = f" ({i - 1} of {len(msgs)} already posted)" if i > 1 else ""
                raise PostError(f"message {i} of {len(msgs)}: Discord answered {e.code} {e.reason}: {said[:300]}{done}") from None
            except urllib.error.URLError as e:
                raise PostError(f"message {i} of {len(msgs)}: cannot reach Discord ({e.reason})") from None
        what = os.path.basename(m["file"]) if m["file"] else f"text, {len(m['content'])} characters"
        print(f"posted {i}/{len(msgs)}: {what}")


def describe(msgs):
    for i, m in enumerate(msgs, 1):
        if m["file"]:
            kb = os.path.getsize(m["file"]) / 1024
            cap = f" — {m['content']}" if m["content"] else ""
            print(f"{i}. {os.path.basename(m['file'])} ({kb:.0f} KB){cap}")
        else:
            print(f"{i}. text, {len(m['content'])} characters\n" + "\n".join("   " + l for l in m["content"].splitlines()))


def markdown(text):
    """Discord 가 그리는 만큼만 — 제목·목록·굵게·코드·<링크>. 미리보기에서 글이 실제와 비슷하게 보이게."""
    def inline(line):
        line = html.escape(line, quote=False)
        line = re.sub(r"`([^`]+)`", r"<code>\1</code>", line)
        line = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", line)
        return re.sub(r"&lt;(https?://[^&\s]+)&gt;", r'<a href="\1">\1</a>', line)
    out, items = [], []
    for line in text.splitlines() + [""]:
        if line.startswith("- "):
            items.append(f"<li>{inline(line[2:])}</li>")
            continue
        if items:
            out.append("<ul>" + "".join(items) + "</ul>")
            items = []
        if line.startswith("### "):
            out.append(f"<h3>{inline(line[4:])}</h3>")
        elif line.startswith("## "):
            out.append(f"<h2>{inline(line[3:])}</h2>")
        elif line.strip():
            out.append(f"<p>{inline(line)}</p>")
    return "".join(out)


def preview(msgs, path):
    """보낼 차례를 한 장의 HTML 로. 그림은 data: 로 넣어 그 파일 하나로 열린다."""
    parts = []
    for m in msgs:
        block = f'<div class="text">{markdown(m["content"])}</div>' if m["content"] else ""
        if m["file"]:
            kind = "video/webm" if m["file"].endswith(".webm") else (mimetypes.guess_type(m["file"])[0] or "")
            with open(m["file"], "rb") as f:
                uri = f"data:{kind};base64,{base64.b64encode(f.read()).decode()}"
            tag = f'<video src="{uri}" controls loop muted autoplay></video>' if kind.startswith("video") else f'<img src="{uri}" alt="">'
            block += tag
        parts.append(f'<div class="msg">{block}</div>')
    page = f"""<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Discord post preview</title><style>
body{{margin:0;background:#313338;color:#dbdee1;font:15px/1.4 system-ui,sans-serif;padding:24px 16px}}
.msg{{max-width:760px;margin:0 auto 18px;padding:12px 16px;background:#2b2d31;border-radius:8px}}
.text{{margin:0 0 8px}} .text p,.text ul{{margin:4px 0}} .text ul{{padding-left:22px}} h2{{font-size:20px;margin:4px 0 6px}}
h3{{font-size:16px;margin:10px 0 4px}} code{{background:#1e1f22;border-radius:4px;padding:1px 4px;font-size:.88em}}
a{{color:#00a8fc;text-decoration:none}} img,video{{max-width:100%;border-radius:6px;display:block}}
</style></head><body>{''.join(parts)}</body></html>"""
    with open(path, "w", encoding="utf-8") as f:
        f.write(page)
    print(path)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dir", help="release-posts/<version>")
    ap.add_argument("--dry-run", action="store_true", help="print what would be posted; send nothing")
    ap.add_argument("--preview", metavar="FILE", help="write an HTML preview of the messages; send nothing")
    args = ap.parse_args(argv)
    try:
        msgs = messages(args.dir)
        if args.dry_run or args.preview:
            describe(msgs)
            if args.preview:
                preview(msgs, args.preview)
            return 0
        send(webhook_url(os.environ), msgs)
    except PostError as e:
        print(f"post.py: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
