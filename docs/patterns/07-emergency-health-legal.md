# Tier 7 — Emergency, Health, and Legal (P421–P460)

**Before generating from this file, read [`00-generation-guide.md`](00-generation-guide.md).**

`tier: "emergency_health_legal"` for every item generated from this
file.

Symptoms, medication, ambulance, police report, interpretation,
lawyer, consular contact, and consent/refusal. **High priority
alongside tier 5** — official guidance makes emergency access and
incident reporting practically relevant for this trip. Note the
family angle: several of these (`me duele [body part]`, allergy
patterns) are exactly where family personalization pays off most —
but for this pass, write them for a generic "you"/traveler, not a
named family member (personalization is a later phase, see
[PATTERN_DRILLING_PLAN.md §5](../PATTERN_DRILLING_PLAN.md)).

Suggested first batch: P421–P440 (symptoms + pharmacy).

| ID | Canonical template | Example | Pre | Xf | Generation constraints | Score | Gate |
|---|---|---|---|---|---|---:|---|
| P421 | `no me siento bien` | No me siento bien. | PR,REFL | A | REFL | 97 | A1 |
| P422 | `me siento mal` | Me siento muy mal. | PR,REFL | A | REFL | 97 | A1 |
| P423 | `me duele [body part]` | Me duele la cabeza. | PR,GUS | A | singular/plural agreement | 97 | A1 |
| P424 | `tengo fiebre` | Tengo fiebre alta. | PR | A | symptom noun | 96 | A1 |
| P425 | `tengo náusea/vómito` | Tengo mucha náusea. | PR | A | symptom noun | 95 | A2 |
| P426 | `tengo diarrea` | Tengo diarrea desde ayer. | PR | A | symptom noun | 95 | A1 |
| P427 | `me cuesta respirar` | Me cuesta respirar. | PR | A | impersonal cost frame | 97 | A2 |
| P428 | `soy alérgico/a a [N]` | Soy alérgica a la penicilina. | SER,PR | A | AGR,PREP(a) | 96 | A1 |
| P429 | `tomo este medicamento` | Tomo este medicamento diario. | PR | A | deictic | 94 | A1 |
| P430 | `necesito una farmacia` | Necesito una farmacia abierta. | PR | A | noun slot | 94 | A0 |
| P431 | `necesito un médico` | Necesito un médico. | PR | A | noun slot | 95 | A0 |
| P432 | `necesito un hospital` | Necesito ir al hospital. | PR | A | noun slot | 96 | A0 |
| P433 | `¿hay una clínica cerca?` | ¿Hay una clínica cerca? | HAY,PR | A | nearby frame | 95 | A1 |
| P434 | `¿puede llamar a una ambulancia?` | ¿Puede llamar a una ambulancia? | MOD | D | UST | 97 | A1 |
| P435 | `fue una emergencia` | Fue una emergencia médica. | PT,SER | A | noun slot | 90 | A2 |
| P436 | `tuve un accidente` | Tuve un accidente en la calle. | PT | A | high-urgency frame | 96 | A2 |
| P437 | `me caí` | Me caí en las escaleras. | PT,REFL | E | REFL | 94 | A2 |
| P438 | `me lastimé [body part]` | Me lastimé la rodilla. | PT,REFL | E | REFL,OBJ | 94 | A2 |
| P439 | `me desmayé / casi me desmayo` | Casi me desmayo. | PT/PR,REFL | E | REFL | 94 | B1 |
| P440 | `¿necesito receta?` | ¿Necesito receta para esto? | PR | A | pharmacy frame | 92 | A2 |
| P441 | `con/sin receta` | Lo quiero sin receta. | none | D | fixed phrase | 91 | A2 |
| P442 | `¿cómo se toma este medicamento?` | ¿Cómo se toma este medicamento? | PR,SE | A | SE | 92 | A2 |
| P443 | `¿cada cuánto lo tomo?` | ¿Cada cuánto lo tomo? | PR,OBJ | B | CL | 92 | A2 |
| P444 | `no entiendo las instrucciones` | No entiendo las instrucciones. | PR | A | OBJ | 93 | A1 |
| P445 | `quiero reportar un robo` | Quiero reportar un robo. | PR | A | legal noun | 96 | A2 |
| P446 | `quiero levantar una denuncia` | Quiero levantar una denuncia. | PR | A | legal register | 95 | B1 |
| P447 | `necesito un intérprete` | Necesito un intérprete de inglés. | PR | A | legal/medical support | 96 | A2 |
| P448 | `necesito un abogado` | Necesito un abogado. | PR | A | legal noun | 96 | A1 |
| P449 | `quiero llamar al consulado/embajada` | Quiero llamar al consulado de Canadá. | PR | A | PREP(a),de | 96 | A2 |
| P450 | `mi pasaporte fue robado` | Mi pasaporte fue robado ayer. | PT,passive | E | passive/participle | 96 | B1 |
| P451 | `perdí todos mis documentos` | Perdí todos mis documentos. | PT | E | plural noun | 95 | A2 |
| P452 | `aquí está la copia de mi pasaporte` | Aquí está la copia de mi pasaporte. | EST,PR | A | de-phrase | 91 | A1 |
| P453 | `necesito el número del reporte` | Necesito el número del reporte. | PR | A | de-phrase | 92 | A2 |
| P454 | `¿dónde está la fiscalía/Ministerio Público?` | ¿Dónde está la fiscalía? | EST,PR | A | legal MX lex | 95 | B1 |
| P455 | `no firmo nada sin entenderlo` | No firmo nada sin entenderlo. | PR,OBJ | F | CL,negative scope | 96 | B1 |
| P456 | `no doy mi consentimiento` | No doy mi consentimiento. | PR | F | legal register | 95 | B1 |
| P457 | `quiero leerlo primero` | Quiero leerlo primero. | PR,OBJ | A | CL | 94 | A2 |
| P458 | `necesito que lo explique despacio` | Necesito que lo explique despacio. | SUBJ,OBJ | D | SUBJ,CL,UST | 95 | B1 |
| P459 | `vi a [person/description]` | Vi a un hombre con chamarra negra. | PT | A | PA,description | 90 | A2 |
| P460 | `esto pasó a las [time] en [Loc]` | Esto pasó a las diez en la estación. | PT | C | PREP(a,en) | 91 | B1 |
