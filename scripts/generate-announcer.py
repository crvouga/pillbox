#!/usr/bin/env python3
"""Generate a finite offline voice pack. Key stays in memory; nothing secret is saved.
Run: python3 scripts/generate-announcer.py --config dev [--voice-id ID]
Uses ELEVEN_LABS_API_KEY if injected, otherwise reads that one Vault field only.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.error
import urllib.request

LINES = {
    "match_starting": "Prepare for deployment.",
    "match_started": "Weapons hot. Take the high ground.",
    "match_over": "Skirmish complete.",
    "victory": "Victory. Excellent work.",
    "defeat": "Defeat. Regroup and redeploy.",
    "double_kill": "Double kill.",
    "triple_kill": "Triple kill.",
    "killing_spree": "Killing spree.",
    "one_minute_remaining": "One minute remaining.",
    "thirty_seconds_remaining": "Thirty seconds remaining.",
    "flag_taken": "Flag taken.",
    "flag_dropped": "Flag dropped.",
    "flag_returned": "Flag returned.",
    "flag_captured": "Flag captured.",
    "hill_controlled": "Control point secured.",
    "hill_contested": "Control point contested.",
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", choices=["dev", "prd"], default="dev")
    parser.add_argument("--voice-id")
    args = parser.parse_args()
    key = os.environ.get("ELEVEN_LABS_API_KEY")
    if not key:
        result = subprocess.run(
            ["vault", "kv", "get", "-field=ELEVEN_LABS_API_KEY", "secret/personal/" + args.config],
            env=os.environ | {"VAULT_ADDR": "https://vault.chrisvouga.dev"},
            capture_output=True, text=True, check=False,
        )
        if result.returncode:
            raise SystemExit("Cannot read ELEVEN_LABS_API_KEY from Vault personal/" + args.config)
        key = result.stdout.strip()
    def request(path, payload=None):
        req = urllib.request.Request("https://api.elevenlabs.io" + path,
            data=json.dumps(payload).encode() if payload else None,
            headers={"xi-api-key": key, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=60) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            # API error bodies can contain request details. Report only status.
            raise SystemExit(f"ElevenLabs HTTP {error.code}; verify ELEVEN_LABS_API_KEY in Vault personal/{args.config}") from None
    voice = args.voice_id
    if not voice:
        voices = json.loads(request("/v2/voices"))["voices"]
        voice = next((v["voice_id"] for v in voices if v["name"] == "Adam"), None)
    if not voice:
        raise SystemExit("Choose --voice-id from your ElevenLabs voice library; no voice was guessed")
    target = Path(__file__).resolve().parents[1] / "web/assets/audio/voice"
    target.mkdir(parents=True, exist_ok=True)
    record = target / "generation.json"
    previous = json.loads(record.read_text()) if record.exists() else {}
    reuse = previous.get("voice_id") == voice and previous.get("lines") == LINES
    manifest = {}
    for kind, text in LINES.items():
        name = "eleven-" + kind + ".mp3"
        path = target / name
        if not (path.exists() and reuse):
            data = request(f"/v1/text-to-speech/{voice}?output_format=mp3_44100_128", {
                "text": text, "model_id": "eleven_multilingual_v2",
                "voice_settings": {"stability": 0.6, "similarity_boost": 0.8, "style": 0.2, "speed": 1.05},
            })
            if len(data) < 1000:
                raise SystemExit("Empty audio response: " + kind)
            path.write_bytes(data)
        manifest[kind] = "/assets/audio/voice/" + name
        print("Generated", kind)
    temp = target / "manifest.json.tmp"
    temp.write_text(json.dumps(manifest, indent=2) + "\n")
    temp.replace(target / "manifest.json")
    record.write_text(json.dumps({"provider":"ElevenLabs","model":"eleven_multilingual_v2","voice_id":voice,"lines":LINES},indent=2)+"\n")

if __name__ == "__main__":
    main()
