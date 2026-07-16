#!/usr/bin/env python3
"""Offline correction pipeline for learner sentence flags.

The handheld can only *raise* a flag (a keyboard-free multiple-choice reason on
a bad example sentence). Judging what to do about it — rewrite the sentence, or
add the missing sense to the word card — is a language task best run on a real
computer, so it lives here rather than on the device.

Flow:

    # on the handheld / from a synced backup DB
    python main.py flags export --out flags.json

    # on your computer, against a local LLM (Ollama)
    python scripts/flag_pipeline.py flags.json --out resolutions.json

    # back on the device (or the synced DB)
    python main.py flags apply resolutions.json

Input is the JSON emitted by ``flags export`` (a list of open flags, each with
the word, its listed meanings, and every translation). Output is a *resolution
pack*: a list of ``{flag_id, resolution, card_id, vocab?, unsuspend?}`` objects
that ``flags apply`` writes back, updating vocab content while leaving FSRS
progress untouched.

The classifier is a pluggable backend:

  * ``ollama`` (default) — POST each flag's context to a local Ollama server
    (http://localhost:11434) and parse a structured JSON verdict. No data
    leaves the machine.
  * ``dry`` — no model; emit ``no_change`` for every flag. Useful for wiring
    up the round trip, or to review flags by hand first.

Only the Python standard library is used (urllib), matching cardbrick/sync.py.
"""

import argparse
import json
import sys
import urllib.error
import urllib.request

DEFAULT_OLLAMA_URL = "http://localhost:11434"
DEFAULT_MODEL = "llama3.1"

# Resolutions understood by `main.py flags apply`.
VALID_RESOLUTIONS = {
    "sentence_updated",  # example_es/en/jp rewritten
    "card_updated",      # a missing sense added to definitions/word_en/word_jp
    "dropped",           # inappropriate; example cleared/replaced
    "no_change",         # false alarm
}

REASON_HINTS = {
    "wrong_translation":
        "The learner says the English/Japanese translation does not carry the "
        "word's actual meaning in this sentence.",
    "meaning_not_listed":
        "The learner says the sentence uses a sense of the word (often "
        "colloquial/idiomatic) that is not in the card's listed meanings.",
    "inappropriate":
        "The learner says the sentence is not appropriate for a young learner.",
    "other": "The learner flagged this sentence without a specific category.",
}

SYSTEM_INSTRUCTIONS = """\
You are a Spanish-for-children curriculum editor. A young learner flagged an \
example sentence on a vocabulary card. Decide the smallest good fix and reply \
with ONLY a JSON object, no prose, with these keys:

  "resolution": one of "sentence_updated", "card_updated", "dropped", \
"no_change".
  "vocab": an object with ONLY the fields you changed, chosen from \
"definitions", "example_es", "example_en", "example_jp", "word_en", \
"word_jp". Omit or leave empty if nothing changes.
  "explanation": one short sentence for the parent.

Guidance:
- If the sentence is fine and the meaning IS listed, use "no_change".
- If the sentence uses a real sense that is simply MISSING from the listed \
meanings, prefer "card_updated" and extend "definitions" to include that sense \
(keep the existing meanings).
- If the translation is wrong or the sentence is confusing, prefer \
"sentence_updated" and provide a clean example_es plus matching example_en \
(and example_jp only if one was present).
- If the sentence is inappropriate for a child, use "dropped" and provide a \
wholesome replacement example_es/example_en instead.
- Keep replacements simple, literal, and beginner-friendly.
"""


def build_prompt(flag):
    """Assemble the per-flag instruction sent to the model."""
    reason = flag.get("reason", "other")
    context = {
        "word": flag.get("word"),
        "listed_meanings": flag.get("definitions"),
        "word_en": flag.get("word_en"),
        "word_jp": flag.get("word_jp"),
        "example_es": flag.get("example_es"),
        "example_en": flag.get("example_en"),
        "example_jp": flag.get("example_jp"),
        "flag_reason": reason,
    }
    return (
        SYSTEM_INSTRUCTIONS
        + "\nWhy it was flagged: " + REASON_HINTS.get(reason, REASON_HINTS["other"])
        + "\n\nCard:\n" + json.dumps(context, ensure_ascii=False, indent=2)
        + "\n\nJSON:"
    )


def call_ollama(prompt, model, base_url, timeout=120):
    """Ask a local Ollama server for a JSON verdict; return the parsed dict."""
    payload = json.dumps({
        "model": model,
        "prompt": prompt,
        "stream": False,
        "format": "json",  # Ollama constrains output to valid JSON
    }).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + "/api/generate",
        data=payload, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        body = json.load(response)
    # /api/generate returns {"response": "<json string>", ...}
    return json.loads(body["response"])


def verdict_to_resolution(flag, verdict):
    """Turn a model verdict into a resolution-pack entry, validated."""
    resolution = verdict.get("resolution", "no_change")
    if resolution not in VALID_RESOLUTIONS:
        resolution = "no_change"
    entry = {
        "flag_id": flag["flag_id"],
        "card_id": flag.get("card_id"),
        "resolution": resolution,
    }
    vocab = verdict.get("vocab") or {}
    # Keep only non-empty string edits.
    vocab = {k: v for k, v in vocab.items() if isinstance(v, str) and v.strip()}
    if vocab and resolution != "no_change":
        entry["vocab"] = vocab
    # A card whose issue was fixed should return to rotation.
    if resolution != "no_change":
        entry["unsuspend"] = True
    if verdict.get("explanation"):
        entry["explanation"] = verdict["explanation"]
    return entry


def process(flags, backend, model, base_url):
    resolutions = []
    for flag in flags:
        if backend == "dry":
            resolutions.append({
                "flag_id": flag["flag_id"],
                "card_id": flag.get("card_id"),
                "resolution": "no_change",
            })
            continue
        try:
            verdict = call_ollama(build_prompt(flag), model, base_url)
        except (urllib.error.URLError, KeyError, ValueError) as exc:
            print(f"flag {flag.get('flag_id')}: model call failed ({exc}); "
                  f"leaving as no_change", file=sys.stderr)
            resolutions.append({
                "flag_id": flag["flag_id"],
                "card_id": flag.get("card_id"),
                "resolution": "no_change",
            })
            continue
        resolutions.append(verdict_to_resolution(flag, verdict))
    return resolutions


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("flags_json",
                        help="flags.json produced by `main.py flags export`")
    parser.add_argument("--out",
                        help="output resolutions file (default: stdout)")
    parser.add_argument("--backend", choices=("ollama", "dry"),
                        default="ollama",
                        help="classifier backend (default: ollama)")
    parser.add_argument("--model", default=DEFAULT_MODEL,
                        help=f"Ollama model (default: {DEFAULT_MODEL})")
    parser.add_argument("--ollama-url", default=DEFAULT_OLLAMA_URL,
                        help=f"Ollama base URL (default: {DEFAULT_OLLAMA_URL})")
    args = parser.parse_args(argv)

    with open(args.flags_json, encoding="utf-8") as handle:
        flags = json.load(handle)

    resolutions = process(flags, args.backend, args.model, args.ollama_url)
    text = json.dumps(resolutions, indent=2, ensure_ascii=False, sort_keys=True)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as handle:
            handle.write(text + "\n")
        print(f"Wrote {len(resolutions)} resolution(s) to {args.out}",
              file=sys.stderr)
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
