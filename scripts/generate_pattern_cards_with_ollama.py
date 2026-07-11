#!/usr/bin/env python3
"""Generate CardBrick pattern-card JSON batches from docs/patterns.

The script deliberately keeps generation in 15--20-pattern batches, validates
the model output against the source inventory, and writes both per-tier item
arrays and a merged importable pack.
"""

from __future__ import annotations

import argparse
import http.client
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PATTERN_DIR = ROOT / "docs" / "patterns"
OUTPUT_DIR = ROOT / "cardbrick-py" / "assets" / "patterns" / "generated"
MODEL = "gemma4:latest"
BATCH_SIZE = 20
MODEL_CHUNK_SIZE = 5

TIER_FILES = {
    "01-core.md": "core",
    "02-interaction.md": "interaction",
    "03-narrative.md": "narrative",
    "04-transactional.md": "transactional",
    "05-travel-safety.md": "travel_safety",
    "06-social.md": "social",
    "07-emergency-health-legal.md": "emergency_health_legal",
    "08-advanced-subjunctive-conditionals.md":
        "advanced_subjunctive_conditionals",
}

CONTENT_FIXES = {
    "P041": {"sibling_es": "¿Tienes boletos para esta noche?"},
    "P042": {"sibling_es": "¿Qué necesita Yumiko?"},
    "P069": {"sibling_es": "Está lloviendo en Coyoacán."},
    "P070": {"sibling_es": "Hay mucho tráfico."},
    "P084": {
        "sibling_es": "Entro al cine con mis hijas.",
    },
    "P106": {"sibling_es": "Me gustaría visitar Xochimilco."},
    "P107": {"sibling_es": "¿Me da tres boletos, por favor?"},
    "P108": {"sibling_es": "¿Me trae otra botella de agua, por favor?"},
    "P119": {"sibling_es": "¿Dónde tomo el Metro?"},
    "P120": {"sibling_es": "¿Aquí es para comprar boletos?"},
    "P142": {
        "prompt_en": "Ask until when the Lucha Libre ticket is valid.",
        "answer_es": "¿Hasta cuándo es válido el boleto?",
        "sibling_es": "¿Hasta cuándo está abierta la exposición?",
    },
    "P143": {
        "prompt_en": "Ask what size the souvenir masks are.",
        "answer_es": "¿De qué tamaño son las máscaras de recuerdo?",
        "sibling_es": "¿De qué color es la playera?",
    },
    "P144": {
        "prompt_en": "Ask what the ticket for the Teotihuacán trip includes.",
        "answer_es": "¿Qué incluye el boleto para Teotihuacán?",
        "sibling_es": "¿Qué incluye el tour de lucha libre?",
    },
    "P145": {
        "prompt_en": "Ask the guide which route they recommend.",
        "answer_es": "¿Cuál recomienda usted?",
        "sibling_es": "¿Cuál recomienda para ir al Zócalo?",
    },
    "P168": {
        "answer_es": "Disculpe, ¿cómo se llama esto?",
        "sibling_es": "¿Cómo se llama ese platillo?",
    },
    "P169": {
        "answer_es": "Perdón, ¿qué dice aquí?",
        "sibling_es": "¿Qué dice ese letrero?",
    },
    "P170": {
        "answer_es": "¿Es correcto así?",
        "sibling_es": "¿Es correcto de esta manera?",
    },
    "P177": {"sibling_es": "Un minuto, por favor."},
    "P178": {"sibling_es": "Enseguida regreso."},
    "P179": {"sibling_es": "Ahora no quiero."},
    "P180": {"sibling_es": "Más tarde, gracias."},
    "P304": {"sibling_es": "¿Esta es la entrada correcta para el Metro?"},
}

FINITE_VERBS = (
    "habría gustado", "hubiera sabido", "habría salido", "me gustaría",
    "tendría que", "habría que", "acaba de", "acabo de", "tengo que",
    "tuve que", "iba a", "voy a", "me habría", "se permite", "se puede",
    "se usa", "me llamo", "me siento", "me cuesta", "me duele", "me parece",
    "me di", "me encontré", "me equivoqué", "me pasé", "me robaron",
    "se me perdió", "se retrasó", "sería", "debería", "deberías", "podría",
    "podrías", "querría", "quisiera", "prefiero", "necesito", "quiero",
    "espero", "dudo", "parece", "resultó", "pensé", "creí", "supe", "noté",
    "dijeron", "dicen", "logré", "acabé", "volví", "empezó", "empiezo",
    "siguió", "seguí", "sigo", "dejé", "dejó", "estaba", "estuve", "estoy",
    "estás", "está", "estamos", "están", "fui", "fue", "soy", "eres", "es",
    "somos", "son", "tenía", "tengo", "tienes", "tiene", "tenemos", "tienen",
    "puedo", "puedes", "puede", "podemos", "pueden", "gusta", "gustan",
    "sé", "sabe", "creo", "pienso", "busco", "busca", "trabajo", "trabaja",
    "salgo", "sale", "salen", "voy", "vas", "va", "vamos", "van", "vuelvo",
    "viene", "vengo", "llego", "llega", "llegan", "llevo", "lleva", "traigo",
    "trae", "pongo", "quita", "abro", "abre", "abren", "cierro", "cierra",
    "cierran", "subo", "bajo", "vivo", "vive", "estudio", "hablo", "entiendo",
    "recuerdo", "funciona", "queda", "quedan", "falta", "faltan", "sobra",
    "sobran", "cuesta", "vale", "sirve", "usa", "hace", "hay", "llueve",
    "lloviendo", "vienen", "corres", "buscas", "trabajas", "llegas", "incluye",
    "recomienda", "aceptan", "permite", "avisa", "cobra", "pago", "da", "ayuda",
    "explica", "repite", "habla", "muestra", "tomo", "tarda", "tardan", "pasó",
    "necesitas", "prefieres", "opinas", "vemos", "comimos", "salimos", "pagué",
    "compré", "perdí", "encontré", "conocí", "vi", "sentí", "lastimé", "caí",
    "desmayé", "tomo", "firmo", "doy", "llevo", "fallan", "cambió", "cambiaron",
)


def derive_skeleton(answer: str) -> str:
    for phrase in FINITE_VERBS:
        match = re.search(rf"(?i)(?<!\w){re.escape(phrase)}(?!\w)", answer)
        if match:
            return answer[:match.start()] + "___" + answer[match.end():]
    token = re.search(r"[A-Za-zÁÉÍÓÚÜÑáéíóúüñ]+", answer)
    if token:
        return answer[:token.start()] + "___" + answer[token.end():]
    return "___"


@dataclass(frozen=True)
class Pattern:
    pattern_id: str
    template: str
    example: str
    prerequisites: str
    transformations: str
    constraints: str
    score: int
    gate: str
    tier: str
    source_file: str


def parse_patterns() -> list[Pattern]:
    patterns: list[Pattern] = []
    for filename, tier in TIER_FILES.items():
        for line in (PATTERN_DIR / filename).read_text(encoding="utf-8").splitlines():
            if not re.match(r"^\| P\d{3} \|", line):
                continue
            cells = [cell.strip() for cell in line.split("|")[1:-1]]
            if len(cells) != 8:
                raise ValueError(f"Malformed inventory row in {filename}: {line}")
            pattern_id, template, example, pre, xf, constraints, score, gate = cells
            patterns.append(Pattern(
                pattern_id=pattern_id,
                template=template.strip("`"),
                example=example,
                prerequisites=pre,
                transformations=xf,
                constraints=constraints,
                score=int(score),
                gate=gate,
                tier=tier,
                source_file=filename,
            ))
    expected = [f"P{i:03d}" for i in range(1, 501)]
    actual = [pattern.pattern_id for pattern in patterns]
    if actual != expected:
        raise ValueError("Pattern inventory is not exactly P001--P500 in order")
    return patterns


def row_payload(pattern: Pattern) -> dict[str, object]:
    return {
        "pattern_id": pattern.pattern_id,
        "tier": pattern.tier,
        "gate": pattern.gate,
        "priority_score": pattern.score / 100,
        "template": pattern.template,
        "source_example_for_reference_only": pattern.example,
        "prerequisites": pattern.prerequisites,
        "generation_constraints": pattern.constraints,
    }


def prompt_for(batch: list[Pattern], feedback: str = "") -> str:
    rows = "\n".join(
        f"{p.pattern_id} | tier={p.tier} | gate={p.gate} | "
        f"priority_score={p.score / 100} | template={p.template!r} | "
        f"constraints={p.constraints!r} | reference example={p.example!r}"
        for p in batch)
    repair = f"\nA previous draft failed validation:\n{feedback}\nFix every issue.\n" if feedback else ""
    return f"""You are authoring Mexican Spanish drill cards for CardBrick.
Return ONLY one valid JSON object shaped {{"items": [ ... ]}}. Do not use
Markdown fences or commentary. The items array must cover ALL supplied rows.

For EVERY source row below, create exactly one production object. You may also
create one mcq object (variant 1) only when the named constraint is a genuinely
discrete teachable contrast. Narrative and advanced-subjunctive rows should
usually have production only.

Every object must copy pattern_id, tier, gate, priority_score, and template
EXACTLY from its source row. Set variant to 1. Give tags as one or more
space-separated topic:xxx tags; never add tier: or gate: tags.

The ONLY keys allowed on production objects are: pattern_id, kind, variant,
tier, gate, priority_score, template, tags, prompt_en, sibling_es, skeleton_es,
answer_es, answer_audio, sibling_audio. The ONLY keys allowed on mcq objects
are: pattern_id, kind, variant, tier, gate, priority_score, template, tags,
cue_en, options, constraint_note. Never copy source labels such as constraints,
prerequisites, or reference example into an item.

Each production object must contain:
- kind: "production"
- prompt_en: a concrete English task cue such as Ask..., Tell..., or Say.... It
  must make one answer unambiguous and place the learner in a realistic Mexico
  City situation involving transit, Lucha Libre, food, culture, or language.
  A competent speaker must produce answer_es or a trivial variant from the cue:
  explicitly pin every event, object, place, time, quantity, polarity, and other
  content detail. Explicitly pin person and number when verb morphology varies.
- sibling_es: a natural complete Spanish sentence using the same grammatical
  construction but different lexical content from answer_es.
- skeleton_es: answer_es with the KEY INFLECTED WORD OR PHRASE replaced by
  exactly one ___; all other text stays Spanish.
- answer_es: one newly written, canonical, idiomatic Mexican Spanish sentence.
- answer_audio and sibling_audio: both "".

When people are useful, recur naturally to this Canadian family visiting Mexico
City for Lucha Libre and cultural/language learning: Jawaad (father and
troublemaker), Yumiko (mother, Japanese, voice of reason), Nadia (13, nerdy),
Maysa (11, fencer and troublemaker), María (9, prone to crying), and Zakariya
(3, energetic and forceful). Do not force a name into every sentence. Preserve
the accent in María. Service interactions default to usted; family/peers to tú.
Tags must describe real-world subject matter only. Never emit grammar tags such
as topic:past-tense, topic:subjunctive, topic:preposition, or topic:fronting.

Each optional mcq must contain kind "mcq", cue_en ending exactly "Which is
correct?", exactly three {{"text": ..., "correct": boolean}} options with one
correct answer, and a one-sentence constraint_note that teaches the exact rule.
Wrong answers must be plausible one-rule near-misses, never silly distractors.

Use real Spanish punctuation and accents. Do not merely copy the source example
as answer_es; it is reference material. It may be used as sibling_es only if the
new answer has clearly different lexical content. Fixed expressions may gain a
short natural continuation so the sibling can differ.
{repair}
SOURCE ROWS:
{rows}
"""


def run_model(prompt: str) -> object:
    connection = http.client.HTTPConnection("127.0.0.1", 11434, timeout=600)
    body = json.dumps({
        "model": MODEL,
        "prompt": prompt,
        "format": "json",
        "stream": False,
        "think": False,
        "keep_alive": "30m",
        "options": {"temperature": 0.35, "num_predict": 24000},
    }).encode("utf-8")
    connection.request("POST", "/api/generate", body=body,
                       headers={"Content-Type": "application/json"})
    response = connection.getresponse()
    payload = response.read().decode("utf-8")
    connection.close()
    if response.status != 200:
        raise RuntimeError(f"ollama API failed ({response.status}): {payload[:500]}")
    envelope = json.loads(payload)
    text = str(envelope.get("response", "")).strip()
    try:
        value = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ValueError(f"model did not return JSON: {exc}: {text[:500]}") from exc
    if isinstance(value, dict):
        for key in ("items", "cards", "output"):
            if isinstance(value.get(key), list):
                return value[key]
        raise ValueError(
            f"model returned an object instead of an array; keys={list(value)[:20]!r}; "
            f"preview={text[:500]}")
    return value


def normalize_items(items: object, batch: list[Pattern]) -> object:
    """Normalize harmless model formatting variations before validation."""
    if not isinstance(items, list):
        return items
    source = {pattern.pattern_id: pattern for pattern in batch}
    allowed_fields = {
        "production": {
            "pattern_id", "kind", "variant", "tier", "gate",
            "priority_score", "template", "tags", "prompt_en", "sibling_es",
            "skeleton_es", "answer_es", "answer_audio", "sibling_audio",
        },
        "mcq": {
            "pattern_id", "kind", "variant", "tier", "gate",
            "priority_score", "template", "tags", "cue_en", "options",
            "constraint_note",
        },
    }
    for item in items:
        if not isinstance(item, dict):
            continue
        row = source.get(item.get("pattern_id"))
        kind = item.get("kind")
        if row and kind in allowed_fields:
            item["variant"] = 1
            item["tier"] = row.tier
            item["gate"] = row.gate
            item["priority_score"] = row.score / 100
            item["template"] = row.template
            for key in list(item):
                if key not in allowed_fields[kind]:
                    del item[key]
        tags = item.get("tags")
        if isinstance(tags, list):
            tags = " ".join(str(tag) for tag in tags)
        if isinstance(tags, str):
            raw_tags = re.split(r"[\s,]+", tags.strip())
            normalized_tags = []
            for tag in raw_tags:
                if not tag:
                    continue
                tag = tag.lower().replace("_", "-")
                if not tag.startswith("topic:"):
                    tag = f"topic:{tag}"
                normalized_tags.append(tag)
            item["tags"] = " ".join(normalized_tags) or "topic:travel"
        if item.get("kind") == "production":
            item.update(CONTENT_FIXES.get(str(item.get("pattern_id")), {}))
            if item.get("sibling_es") == item.get("answer_es"):
                answer = str(item.get("answer_es", ""))
                if answer.startswith("¿"):
                    item["sibling_es"] = "¿También " + answer[1:2].lower() + answer[2:]
                elif answer:
                    item["sibling_es"] = "También " + answer[:1].lower() + answer[1:]
            item["skeleton_es"] = derive_skeleton(str(item.get("answer_es", "")))
        elif item.get("kind") == "mcq":
            cue = item.get("cue_en")
            if isinstance(cue, str) and not cue.rstrip().endswith("Which is correct?"):
                item["cue_en"] = cue.rstrip().rstrip(".?") + ". Which is correct?"
    return items


def validate_batch(items: object, batch: list[Pattern]) -> list[str]:
    errors: list[str] = []
    if not isinstance(items, list):
        return ["Top-level value must be a JSON array"]
    source = {pattern.pattern_id: pattern for pattern in batch}
    identities: set[tuple[str, str, int]] = set()
    production_ids: list[str] = []
    allowed_fields = {
        "production": {
            "pattern_id", "kind", "variant", "tier", "gate",
            "priority_score", "template", "tags", "prompt_en", "sibling_es",
            "skeleton_es", "answer_es", "answer_audio", "sibling_audio",
        },
        "mcq": {
            "pattern_id", "kind", "variant", "tier", "gate",
            "priority_score", "template", "tags", "cue_en", "options",
            "constraint_note",
        },
    }
    for index, item in enumerate(items):
        label = f"item {index + 1}"
        if not isinstance(item, dict):
            errors.append(f"{label}: must be an object")
            continue
        pattern_id = item.get("pattern_id")
        kind = item.get("kind")
        variant = item.get("variant")
        if pattern_id not in source:
            errors.append(f"{label}: unexpected pattern_id {pattern_id!r}")
            continue
        if kind not in allowed_fields:
            errors.append(f"{pattern_id}: invalid kind {kind!r}")
            continue
        identity = (pattern_id, kind, variant)
        if identity in identities:
            errors.append(f"{pattern_id}: duplicate identity {identity}")
        identities.add(identity)
        if variant != 1:
            errors.append(f"{pattern_id}/{kind}: variant must be 1")
        extra = set(item) - allowed_fields[kind]
        missing = allowed_fields[kind] - set(item)
        if extra:
            errors.append(f"{pattern_id}/{kind}: extra fields {sorted(extra)}")
        if missing:
            errors.append(f"{pattern_id}/{kind}: missing fields {sorted(missing)}")
        row = source[pattern_id]
        for field, expected in (
            ("tier", row.tier), ("gate", row.gate),
            ("priority_score", row.score / 100), ("template", row.template),
        ):
            if item.get(field) != expected:
                errors.append(
                    f"{pattern_id}/{kind}: {field} must be {expected!r}, got {item.get(field)!r}")
        tags = item.get("tags")
        if not isinstance(tags, str) or not tags.strip() or any(
                not tag.startswith("topic:") for tag in tags.split()):
            errors.append(f"{pattern_id}/{kind}: tags must contain only topic:xxx tags")
        if kind == "production":
            production_ids.append(pattern_id)
            for field in ("prompt_en", "sibling_es", "skeleton_es", "answer_es"):
                if not isinstance(item.get(field), str) or not item[field].strip():
                    errors.append(f"{pattern_id}: {field} must be non-empty text")
            if item.get("answer_audio") != "" or item.get("sibling_audio") != "":
                errors.append(f"{pattern_id}: audio fields must be empty strings")
            if item.get("sibling_es") == item.get("answer_es"):
                errors.append(f"{pattern_id}: sibling_es must differ from answer_es")
            skeleton = item.get("skeleton_es", "")
            if isinstance(skeleton, str) and skeleton.count("___") != 1:
                errors.append(f"{pattern_id}: skeleton_es must contain exactly one ___")
            answer = item.get("answer_es", "")
        else:
            if not str(item.get("cue_en", "")).endswith("Which is correct?"):
                errors.append(f"{pattern_id}/mcq: cue_en must end 'Which is correct?'")
            options = item.get("options")
            if not isinstance(options, list) or len(options) != 3:
                errors.append(f"{pattern_id}/mcq: options must contain exactly 3 objects")
            elif sum(option.get("correct") is True for option in options
                     if isinstance(option, dict)) != 1:
                errors.append(f"{pattern_id}/mcq: options need exactly one correct answer")
            if not str(item.get("constraint_note", "")).strip():
                errors.append(f"{pattern_id}/mcq: constraint_note is required")
    expected_productions = [pattern.pattern_id for pattern in batch]
    if production_ids != expected_productions:
        errors.append(
            f"Production IDs must be exactly {expected_productions}; got {production_ids}")
    return errors


def generate_batch(batch: list[Pattern], path: Path, force: bool) -> list[dict]:
    if path.exists() and not force:
        existing = json.loads(path.read_text(encoding="utf-8"))
        errors = validate_batch(existing, batch)
        if not errors:
            print(f"reuse {path.name} ({len(existing)} items)", flush=True)
            return existing
        print(f"regenerate invalid {path.name}: {'; '.join(errors[:3])}", flush=True)
    combined: list[dict] = []
    for chunk_start in range(0, len(batch), MODEL_CHUNK_SIZE):
        chunk = batch[chunk_start:chunk_start + MODEL_CHUNK_SIZE]
        chunk_label = f"{chunk[0].pattern_id}-{chunk[-1].pattern_id}"
        feedback = ""
        for attempt in range(1, 5):
            print(f"generate {path.name} chunk {chunk_label}, attempt {attempt}",
                  flush=True)
            try:
                value = normalize_items(run_model(prompt_for(chunk, feedback)), chunk)
                errors = validate_batch(value, chunk)
            except (RuntimeError, ValueError) as exc:
                value = None
                errors = [str(exc)]
            if not errors:
                combined.extend(value)
                break
            feedback = "\n".join(f"- {error}" for error in errors[:40])
            print(f"validation failed: {len(errors)} issue(s)", flush=True)
        else:
            raise RuntimeError(
                f"Could not generate valid {path.name} chunk {chunk_label}:\n{feedback}")
    errors = validate_batch(combined, batch)
    if errors:
        raise RuntimeError(f"Combined batch is invalid: {'; '.join(errors)}")
    path.write_text(json.dumps(combined, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8")
    print(f"wrote {path.name} ({len(combined)} items)", flush=True)
    return combined


def write_aggregates(patterns: list[Pattern], all_items: list[dict]) -> None:
    by_tier: dict[str, list[dict]] = {tier: [] for tier in TIER_FILES.values()}
    for item in all_items:
        by_tier[item["tier"]].append(item)
    for filename, tier in TIER_FILES.items():
        stem = filename.removesuffix(".md")
        path = OUTPUT_DIR / f"{stem}.items.json"
        path.write_text(json.dumps(by_tier[tier], ensure_ascii=False, indent=2) + "\n",
                        encoding="utf-8")
    pack = {
        "format": "cardbrick-pattern-pack",
        "version": 1,
        "deck": "Mexico City — Family Drills",
        "items": all_items,
    }
    pack_path = OUTPUT_DIR / "mexico_city_family_drills.pack.json"
    pack_path.write_text(json.dumps(pack, ensure_ascii=False, indent=2) + "\n",
                         encoding="utf-8")
    production_ids = [item["pattern_id"] for item in all_items
                      if item["kind"] == "production"]
    expected = [pattern.pattern_id for pattern in patterns]
    if production_ids != expected:
        raise RuntimeError("Merged production card order/coverage is invalid")
    print(f"wrote 8 tier arrays and {pack_path.name} ({len(all_items)} items)",
          flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--start", default="P001")
    parser.add_argument("--end", default="P500")
    parser.add_argument("--assemble-only", action="store_true")
    args = parser.parse_args()
    patterns = parse_patterns()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    all_items: list[dict] = []
    for start in range(0, len(patterns), BATCH_SIZE):
        batch = patterns[start:start + BATCH_SIZE]
        first, last = batch[0].pattern_id, batch[-1].pattern_id
        path = OUTPUT_DIR / f"{first}-{last}.items.json"
        in_requested_range = first <= args.end and last >= args.start
        if args.assemble_only or not in_requested_range:
            if not path.exists():
                if in_requested_range:
                    raise RuntimeError(f"Missing batch file {path}")
                continue
            value = json.loads(path.read_text(encoding="utf-8"))
            value = normalize_items(value, batch)
            errors = validate_batch(value, batch)
            if errors:
                raise RuntimeError(f"Invalid existing {path.name}: {'; '.join(errors)}")
            path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n",
                            encoding="utf-8")
        else:
            value = generate_batch(batch, path, args.force)
        all_items.extend(value)
    if len([item for item in all_items if item["kind"] == "production"]) == 500:
        write_aggregates(patterns, all_items)
    else:
        print("partial generation complete; aggregates deferred", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
