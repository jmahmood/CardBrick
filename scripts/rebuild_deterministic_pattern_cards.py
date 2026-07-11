#!/usr/bin/env python3
"""Rebuild production cards with closed, deterministic English cues.

The inventory example is the canonical target. A local language model is used
only for two narrow operations: render that exact target as an English task cue
and write a different sibling using the same construction. A second pass audits
both fields before any generated pack is replaced.
"""

from __future__ import annotations

import argparse
import http.client
import json
import re
from pathlib import Path

from generate_pattern_cards_with_ollama import (
    BATCH_SIZE,
    MODEL,
    OUTPUT_DIR,
    PATTERN_DIR,
    TIER_FILES,
    Pattern,
    derive_skeleton,
    parse_patterns,
)


ROOT = Path(__file__).resolve().parents[1]
CONTENT_PATH = OUTPUT_DIR / "deterministic_content.json"
AUDIT_PATH = OUTPUT_DIR / "semantic_audit.json"
MODEL_BATCH_SIZE = 10

CONTENT_OVERRIDES: dict[str, dict[str, str]] = {
    "P005": {"sibling_es": "El museo es interesante."},
    "P014": {"sibling_es": "Quiero un boleto de tren."},
    "P018": {"sibling_es": "A Maysa le gusta la lucha libre."},
    "P024": {"prompt_en": "Say that you think you and your companions arrived late."},
    "P038": {
        "prompt_en": "Say that you arrive and call the person you are speaking to.",
        "sibling_es": "Llego y te escribo un mensaje.",
    },
    "P039": {"prompt_en": "Say that you and your companions are leaving today or tomorrow."},
    "P041": {"sibling_es": "¿Tienes cambio de cien pesos?"},
    "P047": {"prompt_en": "Ask a familiar person when they are arriving."},
    "P051": {"prompt_en": "Ask a familiar person why they are running."},
    "P052": {"sibling_es": "Esta tarjeta sirve para entrar al Metro."},
    "P055": {"sibling_es": "Mi celular funciona."},
    "P066": {"sibling_es": "Me llamo Jawaad."},
    "P069": {"sibling_es": "Está lloviendo en Coyoacán."},
    "P075": {
        "prompt_en": "Say that you drink mineral water.",
        "sibling_es": "Bebo jugo de naranja.",
    },
    "P076": {"prompt_en": "Say that you buy tickets here."},
    "P077": {"prompt_en": "Say that you are looking for a pharmacy."},
    "P078": {"prompt_en": "Say that you cannot find your card."},
    "P079": {"prompt_en": "Say that you are carrying a small backpack."},
    "P080": {"prompt_en": "Say that you are bringing cash."},
    "P090": {"sibling_es": "Voy con Yumiko."},
    "P081": {"sibling_es": "Quito el celular de la mesa."},
    "P084": {"prompt_en": "Say that you leave the hotel at eight o'clock."},
    "P095": {"prompt_en": "Say that the Spanish word ‘salida’ means ‘exit’."},
    "P096": {
        "prompt_en": "Ask how to say ‘charger’ in Spanish.",
        "sibling_es": "¿Cómo se dice ‘laptop’?",
    },
    "P098": {"sibling_es": "¿Qué significa ‘oportuno’?"},
    "P103": {"prompt_en": "Politely ask a familiar person to repeat it."},
    "P109": {
        "prompt_en": "Politely ask a service worker to help you with the suitcase.",
        "sibling_es": "¿Me ayuda con estas bolsas, por favor?",
    },
    "P110": {"prompt_en": "Ask a service worker to explain this to you."},
    "P113": {"prompt_en": "Politely ask a service worker to write it here for you."},
    "P114": {"prompt_en": "Ask a familiar person to show it to you on the map."},
    "P118": {"prompt_en": "Ask which line you take to Chapultepec."},
    "P126": {"prompt_en": "Ask a service worker to notify you when you and your companions arrive."},
    "P127": {"prompt_en": "Ask whether you can pay by card."},
    "P131": {"sibling_es": "¿Tiene cuartos libres para esta noche?"},
    "P142": {"prompt_en": "Pointing to a ticket, ask until when it is valid."},
    "P143": {"prompt_en": "Pointing to a souvenir, ask what size it is."},
    "P141": {"prompt_en": "Ask someone how long they have lived here."},
    "P147": {"prompt_en": "Ask whether there is anything cheaper."},
    "P151": {"prompt_en": "Pointing to a dish, ask whether it is spicy."},
    "P154": {"prompt_en": "Ask what happened here."},
    "P155": {"prompt_en": "Ask a familiar person what they need."},
    "P156": {"prompt_en": "Ask a familiar person what they prefer to drink."},
    "P157": {"prompt_en": "Ask a familiar person what they think of the place."},
    "P159": {"prompt_en": "Ask a familiar person whether they want you to wait for them."},
    "P160": {"prompt_en": "Ask a familiar person whether they want to go to the market."},
    "P161": {"prompt_en": "Say that you and your companions are going to eat first."},
    "P162": {"prompt_en": "Ask a familiar person, as a suggestion, what they think about meeting tomorrow."},
    "P163": {"prompt_en": "Say that you and your companions had better take an Uber."},
    "P164": {"sibling_es": "Disculpe, ¿cómo?"},
    "P171": {"prompt_en": "Ask a service worker to confirm whether the flight departed."},
    "P174": {"prompt_en": "After hearing directions, confirm that you should go this way."},
    "P195": {"prompt_en": "Say that she kept walking."},
    "P201": {"prompt_en": "Say that after paying, you and your companions left."},
    "P203": {"prompt_en": "Say that later you and your companions took the Metro."},
    "P204": {"prompt_en": "Say that first you and your companions ate, then left."},
    "P216": {"prompt_en": "Say that your companions have not arrived yet."},
    "P218": {"sibling_es": "Nadia acaba de salir."},
    "P219": {"prompt_en": "Say that your wallet got lost on you."},
    "P220": {"sibling_es": "Me encontré con Yumiko."},
    "P228": {"prompt_en": "Say that you were told they closed early."},
    "P239": {"prompt_en": "Say that you had a problem with the reservation."},
    "P247": {
        "prompt_en": "Ask what the total is.",
        "sibling_es": "¿Cuánto es todo junto?",
    },
    "P248": {
        "prompt_en": "Politely ask the cashier to charge you.",
        "sibling_es": "¿Me cobra estos boletos, por favor?",
    },
    "P249": {
        "prompt_en": "Ask a service worker to charge you here.",
        "sibling_es": "¿Me puede cobrar en la caja?",
    },
    "P245": {"sibling_es": "La camisa cuesta trescientos pesos."},
    "P250": {"sibling_es": "Pago en efectivo siempre."},
    "P252": {"prompt_en": "Politely ask the server to bring you the bill."},
    "P263": {"prompt_en": "Politely ask the vendor to give you two tacos de pastor."},
    "P266": {"prompt_en": "Politely ask for a table for three."},
    "P276": {
        "prompt_en": "Say that this is not what you ordered.",
        "sibling_es": "Esto no es lo que compré.",
    },
    "P277": {"prompt_en": "Report that a drink is missing from your order."},
    "P278": {
        "prompt_en": "Say that you want to exchange this.",
        "sibling_es": "Quiero cambiar la talla.",
    },
    "P279": {
        "prompt_en": "Say that you want to return this.",
        "sibling_es": "Quiero devolver esta playera.",
    },
    "P280": {
        "prompt_en": "Say that you want another size.",
        "sibling_es": "Quiero otra playera.",
    },
    "P270": {"prompt_en": "Politely ask for no spice."},
    "P285": {"prompt_en": "Ask whether there is a discount when paying cash."},
    "P303": {"prompt_en": "Ask whether you are going the right way to Reforma."},
    "P304": {"prompt_en": "Ask whether this is the correct exit."},
    "P307": {"prompt_en": "Politely tell the taxi driver to take you to the hotel."},
    "P311": {"sibling_es": "Use el GPS, por favor."},
    "P314": {"prompt_en": "State that the license plate is A123CD."},
    "P316": {"prompt_en": "State that the app says one hundred ninety pesos."},
    "P319": {"sibling_es": "¿Dónde está la oficina de boletos?"},
    "P320": {"sibling_es": "¿Dónde recargo el saldo?"},
    "P322": {"prompt_en": "Ask where you take Line 3."},
    "P323": {"prompt_en": "Ask whether you have to transfer."},
    "P328": {"prompt_en": "Ask where you pick up your luggage."},
    "P333": {
        "prompt_en": "Ask how much earlier you have to arrive.",
        "sibling_es": "¿Cuánto tiempo antes tenemos que estar ahí?",
    },
    "P336": {"prompt_en": "Ask which documents you need."},
    "P340": {"sibling_es": "Necesito una copia del itinerario."},
    "P342": {"prompt_en": "Say that your cell phone was stolen from you."},
    "P357": {"prompt_en": "Politely tell a service worker to send you the location."},
    "P358": {"prompt_en": "Tell a familiar person to share their location with you."},
    "P366": {"sibling_es": "El boleto no pasa."},
    "P367": {"prompt_en": "Ask the driver whether they have change."},
    "P369": {"prompt_en": "Politely tell the driver to avoid deserted streets."},
    "P371": {"prompt_en": "Politely tell the driver to lock the doors."},
    "P379": {"prompt_en": "Ask a service worker to lend you their phone for a call."},
    "P380": {"prompt_en": "Tell a service worker to call this number if anything happens to you."},
    "P382": {"sibling_es": "Soy Jawaad."},
    "P383": {"sibling_es": "Me llamo Yumiko."},
    "P389": {"sibling_es": "¿A qué se dedica tu hermana?"},
    "P393": {"sibling_es": "¿Qué te gusta hacer, Nadia?"},
    "P411": {"sibling_es": "Cuídese en el camino."},
    "P408": {"prompt_en": "Say that you hope so."},
    "P410": {"prompt_en": "Wish a formal addressee well."},
    "P416": {"prompt_en": "Say that you will write to the other person later."},
    "P417": {"prompt_en": "Politely tell a service worker to notify you if anything changes."},
    "P418": {"prompt_en": "Say that if a familiar person wants, you will accompany them."},
    "P431": {"sibling_es": "Necesito una enfermera."},
    "P437": {"sibling_es": "Me caí en la calle."},
    "P438": {"sibling_es": "Me lastimé el tobillo."},
    "P439": {"sibling_es": "Me desmayé en la calle."},
    "P443": {
        "prompt_en": "Ask how often you should take it.",
        "sibling_es": "¿Cada cuánto lo debo tomar?",
    },
    "P448": {"sibling_es": "Necesito un abogado penalista."},
    "P452": {"prompt_en": "State that here is the copy of your passport."},
    "P471": {"prompt_en": "Say that even if it rains, you will go."},
    "P472": {"prompt_en": "Say that you speak slowly so the other person understands."},
    "P473": {"prompt_en": "Tell a familiar person to notify you before they leave."},
    "P474": {"prompt_en": "Say that when the other person arrives, you will call them."},
    "P466": {"prompt_en": "Tell a service worker that it is better for them to wait here."},
    "P475": {"prompt_en": "Say that as soon as you finish, you leave."},
    "P476": {"prompt_en": "Tell a familiar person to notify you if they arrive late."},
    "P477": {"prompt_en": "Say that if you had time, you would go."},
    "P479": {"prompt_en": "Politely say that you would like the other person to confirm it for you."},
    "P481": {"prompt_en": "Politely say that you would like to change seats."},
    "P482": {"prompt_en": "Politely ask a service worker to speak more slowly."},
    "P492": {"prompt_en": "Say that you will not go out unless the other person calls you."},
    "P498": {"prompt_en": "Say that according to the app, ten minutes remain."},
}

SKELETON_OVERRIDES: dict[str, str] = {
    "P078": "No ___ mi tarjeta.",
    "P089": "___ a la ciudad por trabajo.",
    "P095": "\"Salida\" ___ \"exit\".",
    "P096": "¿Cómo ___ \"charger\"?",
    "P097": "¿Cómo ___ \"Xola\"?",
    "P098": "¿Qué ___ \"vigente\"?",
    "P114": "¿Me lo ___ en el mapa?",
    "P122": "¿Este ___ por Reforma?",
    "P163": "Mejor ___ un Uber.",
    "P165": "Perdón, no ___.",
    "P166": "No ___ bien.",
    "P168": "¿Cómo se ___ esto?",
    "P169": "¿Qué ___ aquí?",
    "P182": "Ayer ___ tarde.",
    "P183": "Anoche ___ en Roma.",
    "P192": "No ___ dormir.",
    "P199": "Cuando ___, sonó el teléfono.",
    "P200": "Mientras ___, leí.",
    "P202": "Antes de entrar, ___.",
    "P203": "Luego ___ el Metro.",
    "P207": "Entonces me ___.",
    "P212": "Todavía no ___ comido.",
    "P214": "Nunca ___ probado esto.",
    "P215": "Ya ___ hablado con recepción.",
    "P216": "Todavía no ___ llegado.",
    "P223": "Me ___ el barrio.",
    "P224": "Me ___ caro.",
    "P228": "Me ___ que cerraban temprano.",
    "P238": "___ en Metro cuando perdí la cartera.",
    "P263": "¿Me ___ dos tacos de pastor?",
    "P266": "Mesa para ___, por favor.",
    "P270": "Sin ___, por favor.",
    "P271": "Poco ___, por favor.",
    "P272": "La carne bien ___, por favor.",
    "P273": "Un ___ de agua, por favor.",
    "P274": "___ café, por favor.",
    "P316": "La app ___ ciento noventa pesos.",
    "P317": "¿Me ___ a pedir un Uber?",
    "P320": "¿Dónde ___ la tarjeta?",
    "P328": "¿Dónde ___ mi equipaje?",
    "P344": "No ___ esta zona.",
    "P356": "¿Me ___ el wifi un momento?",
    "P360": "Ya ___.",
    "P365": "¿Dónde ___ transbordo?",
    "P366": "La tarjeta no ___.",
    "P370": "No se ___ aquí.",
    "P380": "Si me pasa algo, ___ a este número.",
    "P381": "Mucho ___.",
    "P389": "¿A qué te ___?",
    "P395": "Me ___ la arquitectura.",
    "P399": "¿Te ___ ir al mercado?",
    "P402": "Con ___.",
    "P405": "Qué ___ lugar.",
    "P406": "Qué bueno que ___.",
    "P407": "Qué mal que se ___.",
    "P410": "Que le ___ bien.",
    "P416": "Te ___ luego.",
    "P418": "Si ___, te acompaño.",
    "P439": "Casi me ___.",
    "P442": "¿Cómo se ___ este medicamento?",
    "P474": "Cuando ___, te llamo.",
    "P477": "Si ___ tiempo, iría.",
    "P491": "En caso de que ___, llevo paraguas.",
    "P494": "Dondequiera que ___, llevo copia del pasaporte.",
}


def call_model(prompt: str) -> object:
    connection = http.client.HTTPConnection("127.0.0.1", 11434, timeout=600)
    body = json.dumps({
        "model": MODEL,
        "prompt": prompt,
        "format": "json",
        "stream": False,
        "think": False,
        "keep_alive": "30m",
        "options": {"temperature": 0.15, "num_predict": 8000},
    }).encode("utf-8")
    connection.request("POST", "/api/generate", body=body,
                       headers={"Content-Type": "application/json"})
    response = connection.getresponse()
    payload = response.read().decode("utf-8")
    connection.close()
    if response.status != 200:
        raise RuntimeError(f"Ollama API failed ({response.status}): {payload[:500]}")
    envelope = json.loads(payload)
    text = str(envelope.get("response", "")).strip()
    value = json.loads(text)
    if isinstance(value, dict) and isinstance(value.get("items"), list):
        return value["items"]
    return value


def validate_content(value: object, batch: list[Pattern], *, strict: bool = True) -> list[str]:
    if not isinstance(value, list):
        return ["top level must be an array"]
    errors: list[str] = []
    expected = [pattern.pattern_id for pattern in batch]
    actual = [item.get("pattern_id") for item in value if isinstance(item, dict)]
    if actual != expected:
        errors.append(f"IDs must be {expected}, got {actual}")
    source = {pattern.pattern_id: pattern for pattern in batch}
    for item in value:
        if not isinstance(item, dict) or item.get("pattern_id") not in source:
            continue
        pattern = source[item["pattern_id"]]
        if set(item) != {"pattern_id", "prompt_en", "sibling_es"}:
            errors.append(f"{pattern.pattern_id}: wrong keys {sorted(item)}")
        prompt = item.get("prompt_en")
        sibling = item.get("sibling_es")
        if not isinstance(prompt, str) or len(prompt.strip()) < 8:
            errors.append(f"{pattern.pattern_id}: missing prompt_en")
        elif strict and not re.match(
                r"^(Ask|Politely ask|Say|Tell|State|Explain|Report|Let)", prompt):
            errors.append(f"{pattern.pattern_id}: cue is not a task cue: {prompt!r}")
        if not isinstance(sibling, str) or len(sibling.strip()) < 2:
            errors.append(f"{pattern.pattern_id}: missing sibling_es")
    return errors


def normalize_reviewed(value: object) -> object:
    """Turn an occasional direct first-person gloss into a learner task cue."""
    if not isinstance(value, list):
        return value
    starters = re.compile(r"^(Ask|Politely ask|Say|Tell|State|Explain|Report|Let)")
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("prompt_en"), str):
            continue
        prompt = item["prompt_en"].strip()
        if starters.match(prompt):
            continue
        prompt = re.sub(r"\bI was\b", "you were", prompt)
        prompt = re.sub(r"\bI am\b", "you are", prompt)
        prompt = re.sub(r"\bI have\b", "you have", prompt)
        prompt = re.sub(r"\bI\b", "you", prompt)
        prompt = re.sub(r"\bmy\b", "your", prompt, flags=re.IGNORECASE)
        prompt = re.sub(r"\bwe\b", "you and your companions", prompt,
                        flags=re.IGNORECASE)
        prompt = re.sub(r"\bour\b", "your group's", prompt, flags=re.IGNORECASE)
        item["prompt_en"] = "Say that " + prompt[:1].lower() + prompt[1:]
    return value


def rows_for_prompt(batch: list[Pattern], draft: list[dict] | None = None) -> str:
    by_id = {item["pattern_id"]: item for item in (draft or [])}
    rows = []
    for pattern in batch:
        row = {
            "pattern_id": pattern.pattern_id,
            "template": pattern.template,
            "fixed_answer_es": pattern.example,
        }
        if pattern.pattern_id in by_id:
            row["draft_prompt_en"] = by_id[pattern.pattern_id]["prompt_en"]
            row["draft_sibling_es"] = by_id[pattern.pattern_id]["sibling_es"]
        rows.append(row)
    return json.dumps(rows, ensure_ascii=False, indent=2)


def draft_prompt(batch: list[Pattern]) -> str:
    return f"""Return only {{"items": [...]}} as valid JSON.

For each row, fixed_answer_es is a closed-production card's exact target.
Create exactly these three fields: pattern_id, prompt_en, sibling_es.

prompt_en must be an English TASK CUE beginning Ask, Politely ask, Say, Tell,
State, Explain, Report, or Let. It must pin the COMPLETE proposition in the
Spanish target: tense/aspect, affirmation or negation, every participant,
person/number/gender when expressed by morphology, action/state, object,
location, time, quantity, comparison, and quoted word. Do not invent context
that is absent from the target. Do not ask an open question such as "what did
you do?". A competent speaker should produce fixed_answer_es or only a trivial
wording variant from prompt_en.

Perspective rules are mandatory:
- Spanish first person singular means the learner/speaker: "Say that you ...".
- Spanish first person plural means the learner plus others: "Say that you and
  your companions ..." (or name the included people when the target does).
- Spanish tú/second-person forms address another person. Never cue these with
  ambiguous bare "you"; write "Ask a familiar person whether they ..." or
  "Tell your daughter to ...".
- Spanish usted/ustedes or service-register forms address a service worker or
  staff: identify that addressee and use English "they" where useful.
- Preserve clitic roles exactly: "te llamo" means the learner calls the other
  person, not "you call you"; "me ayudas" means the other person helps the
  learner.

Examples:
- Ayer nos perdimos en el Mercado de Sonora. -> Say that yesterday you and your
  companions got lost in the Mercado de Sonora.
- ¿Me da dos boletos? -> Politely ask the attendant to give you two tickets.
- Mi amiga acaba de salir. -> Say that your female friend has just left.

sibling_es must be a different, complete, natural Mexican Spanish sentence
using the SAME canonical construction, with different lexical content. It is a
hint, not a paraphrase of the answer. Preserve Spanish punctuation and accents.

ROWS:
{rows_for_prompt(batch)}
"""


def review_prompt(batch: list[Pattern], draft: list[dict], feedback: str = "") -> str:
    return f"""Return only {{"items": [...]}} as valid JSON.

You are the final reviewer for CLOSED-production language cards. For every row,
return exactly pattern_id, prompt_en, sibling_es. Correct the draft wherever
needed.

The decisive test: could a competent speaker given prompt_en produce
fixed_answer_es or a trivial variant? The cue must explicitly determine all
content and morphology in the target. Reject open wording, invented context,
missing subjects, ambiguous singular/plural, omitted negation, changed tense,
missing places/times/quantities, or any cue that permits an unrelated valid
answer. Keep the cue as a natural English task beginning with Ask, Politely ask,
Say, Tell, State, Explain, Report, or Let.

Audit grammatical perspective explicitly. First-person Spanish is the learner
(English "you" in the cue); first-person plural is the learner plus named or
generic companions; second-person Spanish addresses a familiar other person;
usted/ustedes addresses a service worker or staff. Do not use ambiguous bare
"you" for a Spanish second-person target. Preserve every direct/indirect object
and reflexive clitic role exactly (for example, "te llamo" means the learner
calls the other person).

Also verify sibling_es is grammatical Mexican Spanish, complete, lexically
different, and uses the same canonical template as fixed_answer_es. Never alter
fixed_answer_es; it is inventory ground truth.

Previous validation feedback, if any:
{feedback or "none"}

ROWS AND DRAFTS:
{rows_for_prompt(batch, draft)}
"""


def generate_content(batch: list[Pattern]) -> list[dict]:
    draft = None
    feedback = ""
    for attempt in range(1, 4):
        print(f"draft {batch[0].pattern_id}-{batch[-1].pattern_id} attempt {attempt}",
              flush=True)
        try:
            draft = call_model(draft_prompt(batch))
            errors = validate_content(draft, batch, strict=False)
        except Exception as exc:  # keep resumable batch generation simple
            errors = [str(exc)]
        if not errors:
            break
        feedback = "\n".join(f"- {error}" for error in errors)
    else:
        raise RuntimeError(f"Draft failed:\n{feedback}")

    for attempt in range(1, 4):
        print(f"review {batch[0].pattern_id}-{batch[-1].pattern_id} attempt {attempt}",
              flush=True)
        try:
            reviewed = normalize_reviewed(
                call_model(review_prompt(batch, draft, feedback)))
            errors = validate_content(reviewed, batch)
        except Exception as exc:
            reviewed = None
            errors = [str(exc)]
        if not errors:
            return reviewed
        feedback = "\n".join(f"- {error}" for error in errors)
    raise RuntimeError(f"Review failed:\n{feedback}")


def topic_tags(pattern: Pattern) -> str:
    text = f"{pattern.template} {pattern.example}".lower()
    groups = (
        ("topic:emergency", ("policía", "911", "robo", "robar", "seguro", "siguen")),
        ("topic:health", ("farmacia", "médic", "hospital", "ambulancia", "fiebre",
                          "náuse", "diarrea", "duele", "respirar", "medicamento",
                          "receta", "accidente", "lastim", "alérg")),
        ("topic:legal", ("denuncia", "abogado", "intérprete", "fiscalía",
                         "consulado", "consentimiento", "firmo", "reporte")),
        ("topic:taxi", ("taxi", "uber", "conductor", "taxímetro", "placa")),
        ("topic:transit", ("metro", "metrobús", "tren", "autobús", "estación",
                           "línea", "andén", "transbord", "parada", "ruta")),
        ("topic:airport", ("aeropuerto", "vuelo", "migración", "aduana",
                           "equipaje", "maleta", "pase de abordar")),
        ("topic:lodging", ("hotel", "habitación", "recepción", "check-in",
                           "check-out", "toalla", "desayuno")),
        ("topic:restaurant", ("restaurante", "mesa", "cuenta", "menú", "taco",
                              "enchilada", "mole", "salsa", "comer", "bebida",
                              "café", "picante", "propina", "pedido")),
        ("topic:shopping", ("compr", "precio", "talla", "playera", "recibo",
                            "descuento", "devolver", "cambiar esto")),
        ("topic:money", ("pagar", "efectivo", "tarjeta", "cuesta", "pesos",
                         "cambio", "cobra", "cajero")),
        ("topic:lucha-libre", ("lucha", "boleto", "taquilla", "función")),
        ("topic:language", ("español", "inglés", "pronuncia", "significa",
                            "se dice", "palabra", "entend", "habla más despacio")),
        ("topic:smalltalk", ("me llamo", "mucho gusto", "primera vez", "visita",
                             "dedicas", "gracias", "felicidades", "contacto")),
    )
    tags = [tag for tag, words in groups if any(word in text for word in words)]
    if not tags:
        if pattern.tier == "social":
            tags = ["topic:smalltalk"]
        elif pattern.tier == "emergency_health_legal":
            tags = ["topic:emergency"]
        elif pattern.tier == "transactional":
            tags = ["topic:transactions"]
        else:
            tags = ["topic:travel"]
    return " ".join(tags[:2])


def make_item(pattern: Pattern, content: dict) -> dict:
    content = {**content, **CONTENT_OVERRIDES.get(pattern.pattern_id, {})}
    prompt = content["prompt_en"].strip()
    if prompt.endswith("?"):
        prompt = prompt[:-1] + "."
    return {
        "pattern_id": pattern.pattern_id,
        "kind": "production",
        "variant": 1,
        "tier": pattern.tier,
        "gate": pattern.gate,
        "priority_score": pattern.score / 100,
        "template": pattern.template,
        "tags": topic_tags(pattern),
        "prompt_en": prompt,
        "sibling_es": content["sibling_es"].strip(),
        "skeleton_es": SKELETON_OVERRIDES.get(
            pattern.pattern_id, derive_skeleton(pattern.example)),
        "answer_es": pattern.example,
        "answer_audio": "",
        "sibling_audio": "",
    }


def write_outputs(patterns: list[Pattern], content: dict[str, dict]) -> None:
    items = [make_item(pattern, content[pattern.pattern_id]) for pattern in patterns]
    for start in range(0, 500, BATCH_SIZE):
        batch = items[start:start + BATCH_SIZE]
        path = OUTPUT_DIR / f"{batch[0]['pattern_id']}-{batch[-1]['pattern_id']}.items.json"
        path.write_text(json.dumps(batch, ensure_ascii=False, indent=2) + "\n",
                        encoding="utf-8")
    for filename, tier in TIER_FILES.items():
        tier_items = [item for item in items if item["tier"] == tier]
        path = OUTPUT_DIR / f"{filename.removesuffix('.md')}.items.json"
        path.write_text(json.dumps(tier_items, ensure_ascii=False, indent=2) + "\n",
                        encoding="utf-8")
    pack = {
        "format": "cardbrick-pattern-pack",
        "version": 1,
        "deck": "Mexico City — Family Drills",
        "items": items,
    }
    (OUTPUT_DIR / "mexico_city_family_drills.pack.json").write_text(
        json.dumps(pack, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def semantic_audit(patterns: list[Pattern]) -> list[dict]:
    pack = json.loads(
        (OUTPUT_DIR / "mexico_city_family_drills.pack.json").read_text(encoding="utf-8"))
    cards = {item["pattern_id"]: item for item in pack["items"]}
    results: list[dict] = []
    for start in range(0, 500, MODEL_BATCH_SIZE):
        batch = patterns[start:start + MODEL_BATCH_SIZE]
        rows = [{
            "pattern_id": pattern.pattern_id,
            "canonical_template": pattern.template,
            "prompt_en": cards[pattern.pattern_id]["prompt_en"],
            "answer_es": cards[pattern.pattern_id]["answer_es"],
            "sibling_es": cards[pattern.pattern_id]["sibling_es"],
        } for pattern in batch]
        prompt = f"""Return only {{"items": [...]}} as JSON, one item per row.

Act as an adversarial auditor for CLOSED-production language cards. For each
row return pattern_id, status (exactly "pass" or "fail"), and reason. Return a
replacement_prompt_en and replacement_sibling_es only when status is fail.

Fail unless prompt_en uniquely determines answer_es or a trivial variant. Check
every participant and speaker/addressee role, person/number/gender, clitic role,
tense/aspect/mood, polarity, action/state, object, place, time, quantity,
comparison, and quoted word. A learner giving a different valid proposition
must not be possible. Wh-question targets are valid when the cue tells the
learner to ask that exact wh-question. Deictic targets may use a visible object
named in the cue.

Also fail if sibling_es is unnatural, duplicates the answer, changes the
canonical construction, or does not form a useful same-pattern hint. Ignore
stylistic preferences that do not affect determinacy or grammatical validity.

ROWS:
{json.dumps(rows, ensure_ascii=False, indent=2)}
"""
        print(f"audit {batch[0].pattern_id}-{batch[-1].pattern_id}", flush=True)
        value = call_model(prompt)
        if not isinstance(value, list) or len(value) != len(batch):
            raise RuntimeError(f"Malformed audit result for {batch[0].pattern_id}")
        results.extend(value)
    AUDIT_PATH.write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n",
                          encoding="utf-8")
    return results


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--assemble-only", action="store_true")
    parser.add_argument("--audit-only", action="store_true")
    args = parser.parse_args()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    patterns = parse_patterns()
    if args.audit_only:
        results = semantic_audit(patterns)
        failures = [result for result in results if result.get("status") != "pass"]
        print(f"semantic audit: {len(results) - len(failures)} pass, "
              f"{len(failures)} fail", flush=True)
        return 1 if failures else 0
    content: dict[str, dict] = {}
    if CONTENT_PATH.exists() and not args.force:
        content = json.loads(CONTENT_PATH.read_text(encoding="utf-8"))
    if not args.assemble_only:
        for start in range(0, 500, MODEL_BATCH_SIZE):
            batch = patterns[start:start + MODEL_BATCH_SIZE]
            if all(pattern.pattern_id in content for pattern in batch):
                print(f"reuse {batch[0].pattern_id}-{batch[-1].pattern_id}", flush=True)
                continue
            reviewed = generate_content(batch)
            for item in reviewed:
                content[item["pattern_id"]] = item
            CONTENT_PATH.write_text(
                json.dumps(content, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8")
    missing = [pattern.pattern_id for pattern in patterns
               if pattern.pattern_id not in content]
    if missing:
        raise RuntimeError(f"Missing reviewed content: {missing}")
    write_outputs(patterns, content)
    print("wrote deterministic production-only pack with 500 cards", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
