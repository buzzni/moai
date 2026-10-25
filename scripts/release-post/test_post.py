"""post.py 의 시험 — 네트워크는 안 탄다. 보내는 자리는 가짜 opener 로 갈아 끼운다."""
import io
import json
import os
import sys
import tempfile
import unittest
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import post  # noqa: E402

HOOK = "https://discord.com/api/webhooks/123456/secret-token"


def make(tmp, text="## moai v0.23.0\n- **New keys**\n", media=None):
    with open(os.path.join(tmp, "post.md"), "w", encoding="utf-8") as f:
        f.write(text)
    if media:
        os.makedirs(os.path.join(tmp, "media"))
        for name, data in media.items():
            mode = "wb" if isinstance(data, bytes) else "w"
            with open(os.path.join(tmp, "media", name), mode) as f:
                f.write(data)
    return tmp


class Messages(unittest.TestCase):
    def test_the_text_goes_first_then_each_picture_with_its_caption(self):
        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, media={"02-web.png": b"\x89PNG", "01-keys.png": b"\x89PNG", "01-keys.md": "Your own keys\n"})
            got = post.messages(tmp)
        self.assertEqual([m["content"] for m in got], ["## moai v0.23.0\n- **New keys**", "Your own keys", ""])
        self.assertEqual([os.path.basename(m["file"] or "") for m in got], ["", "01-keys.png", "02-web.png"])

    def test_a_post_longer_than_one_message_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, text="x" * (post.DISCORD_LIMIT + 1))
            with self.assertRaises(post.PostError):
                post.messages(tmp)

    def test_a_file_discord_cannot_show_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, media={"notes.txt": "hi"})
            with self.assertRaises(post.PostError):
                post.messages(tmp)

    def test_a_missing_post_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(post.PostError):
                post.messages(tmp)


class Url(unittest.TestCase):
    def test_only_a_discord_webhook_is_taken(self):
        self.assertEqual(post.webhook_url({"DISCORD_WEBHOOK_URL": HOOK + "\n"}), HOOK)
        for bad in ({}, {"DISCORD_WEBHOOK_URL": ""}, {"DISCORD_WEBHOOK_URL": "https://example.com/api/webhooks/1/x"}):
            with self.assertRaises(post.PostError):
                post.webhook_url(bad)

    def test_the_error_never_carries_the_address(self):
        try:
            post.webhook_url({"DISCORD_WEBHOOK_URL": "https://evil.example/" + "secret-token"})
        except post.PostError as e:
            self.assertNotIn("secret-token", str(e))


class FakeResponse(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()


class Send(unittest.TestCase):
    def test_it_sends_in_order_and_waits_out_a_rate_limit(self):
        seen, slept = [], []
        replies = [None, 429, None, None]

        def opener(req, timeout=None):
            seen.append(req)
            code = replies.pop(0)
            if code == 429:
                body = io.BytesIO(json.dumps({"retry_after": 0.25}).encode())
                raise urllib.error.HTTPError(req.full_url, 429, "Too Many Requests", {}, body)
            return FakeResponse(b"{}")

        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, media={"01-keys.png": b"\x89PNG\r\n", "01-keys.md": "Keys"})
            msgs = post.messages(tmp)
            post.send(HOOK, msgs, opener=opener, sleep=slept.append)
        self.assertEqual(slept, [0.25])
        self.assertEqual(len(seen), 3)
        self.assertTrue(all(r.full_url == HOOK + "?wait=true" for r in seen))
        first = json.loads(seen[0].data)
        self.assertEqual(first["allowed_mentions"], {"parse": []}, "the post must never ping anyone")
        self.assertIn(b'filename="01-keys.png"', seen[2].data)
        self.assertIn(b'"content": "Keys"', seen[2].data)

    def test_a_rate_limit_without_a_wait_still_waits(self):
        replies = [429, None]

        def opener(req, timeout=None):
            if replies.pop(0) == 429:
                body = io.BytesIO(json.dumps({"retry_after": None}).encode())
                raise urllib.error.HTTPError(req.full_url, 429, "Too Many Requests", {}, body)
            return FakeResponse(b"{}")

        slept = []
        with tempfile.TemporaryDirectory() as tmp:
            make(tmp)
            post.send(HOOK, post.messages(tmp), opener=opener, sleep=slept.append)
        self.assertEqual(slept, [1.0])

    def test_a_run_can_start_part_way_so_a_retry_does_not_post_twice(self):
        seen = []

        def opener(req, timeout=None):
            seen.append(req)
            return FakeResponse(b"{}")

        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, media={"01-a.png": b"\x89PNG", "02-b.png": b"\x89PNG"})
            post.send(HOOK, post.messages(tmp), start=2, opener=opener, sleep=lambda s: None)
        self.assertEqual(len(seen), 2, "message 1 went out again")
        self.assertIn(b'filename="01-a.png"', seen[0].data)

    def test_a_failure_part_way_names_where_to_start_again(self):
        calls = []

        def opener(req, timeout=None):
            calls.append(req)
            if len(calls) == 2:
                raise urllib.error.HTTPError(req.full_url, 502, "Bad Gateway", {}, io.BytesIO(b"upstream"))
            return FakeResponse(b"{}")

        with tempfile.TemporaryDirectory() as tmp:
            make(tmp, media={"01-a.png": b"\x89PNG", "02-b.png": b"\x89PNG"})
            with self.assertRaises(post.PostError) as ctx:
                post.send(HOOK, post.messages(tmp), opener=opener, sleep=lambda s: None)
        self.assertIn("1 of 3 already posted", str(ctx.exception))
        self.assertIn("start at 2", str(ctx.exception))

    def test_a_refusal_says_the_status_but_not_the_address(self):
        def opener(req, timeout=None):
            raise urllib.error.HTTPError(req.full_url, 404, "Not Found", {}, io.BytesIO(b'{"message": "Unknown Webhook"}'))

        with tempfile.TemporaryDirectory() as tmp:
            make(tmp)
            with self.assertRaises(post.PostError) as ctx:
                post.send(HOOK, post.messages(tmp), opener=opener, sleep=lambda s: None)
        self.assertIn("404", str(ctx.exception))
        self.assertIn("Unknown Webhook", str(ctx.exception))
        self.assertNotIn("secret-token", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
