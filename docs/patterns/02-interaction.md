# Tier 2 — Interaction (P101–P180)

**Before generating from this file, read [`00-generation-guide.md`](00-generation-guide.md).**

`tier: "interaction"` for every item generated from this file.

Requests, clarification, confirmation, route-checking, booking,
availability, and repair language — essential for using Spanish
before you can narrate fluently. Heavy on `UST` (tú/usted) and
`CL` (clitic placement) constraints, both good MCQ material.

Suggested first batch: P101–P120 (polite requests: ¿puedes/podrías/
me puede...?).

| ID | Canonical template | Example | Pre | Xf | Generation constraints | Score | Gate |
|---|---|---|---|---|---|---:|---|
| P101 | `¿puedes [Inf]?` | ¿Puedes ayudarme? | PR,MOD | B | UST choice | 95 | A1 |
| P102 | `¿me puedes [Inf]?` | ¿Me puedes esperar? | PR,MOD,IO | B | CL,UST | 95 | A1 |
| P103 | `¿podrías [Inf]?` | ¿Podrías repetirlo? | MOD,COND | B | CL,UST | 94 | A2 |
| P104 | `¿me podría [Inf]?` | ¿Me podría indicar la salida? | MOD,COND,IO | B | CL,UST | 94 | A2 |
| P105 | `quiero/querría [N]` | Quiero un café. | PR,COND | A | AGR | 94 | A0 |
| P106 | `me gustaría [Inf/N]` | Me gustaría reservar. | COND | A | infinitive/noun | 94 | A2 |
| P107 | `¿me da [N], por favor?` | ¿Me da dos boletos? | PR,IO | D | UST,AGR | 94 | A0 |
| P108 | `¿me trae [N], por favor?` | ¿Me trae la cuenta? | PR,IO | D | UST,AGR | 93 | A1 |
| P109 | `¿me ayuda con [N]?` | ¿Me ayuda con la maleta? | PR,IO | D | PREP(con),UST | 93 | A1 |
| P110 | `¿me explica [X]?` | ¿Me explica esto? | PR,IO | D | UST | 93 | A1 |
| P111 | `¿me repite [X]?` | ¿Me repite la dirección? | PR,IO | D | UST | 93 | A1 |
| P112 | `¿me habla más despacio?` | ¿Me habla más despacio, por favor? | PR,IO | D | UST | 93 | A1 |
| P113 | `¿me lo puede escribir?` | ¿Me lo puede escribir aquí? | MOD,OBJ,IO | B | CL | 93 | A2 |
| P114 | `¿me lo muestras?` | ¿Me lo muestras en el mapa? | PR,OBJ,IO | B | CL,UST | 92 | A1 |
| P115 | `¿dónde está el baño?` | ¿Dónde está el baño? | EST,PR | A | fixed travel utility | 92 | A0 |
| P116 | `¿dónde queda [place]?` | ¿Dónde queda Bellas Artes? | PR | A | PREP omitted | 92 | A1 |
| P117 | `¿cómo llego a [Loc]?` | ¿Cómo llego al Zócalo? | PR | A | PREP(a) | 92 | A1 |
| P118 | `¿qué línea tomo?` | ¿Qué línea tomo para Chapultepec? | PR | A | PREP(para) | 92 | A1 |
| P119 | `¿dónde tomo [transport]?` | ¿Dónde tomo el Metrobús? | PR | A | OBJ,MX lex | 92 | A1 |
| P120 | `¿aquí es para [N]?` | ¿Aquí es para recargar la tarjeta? | SER,PR | A | PREP(para) | 92 | A1 |
| P121 | `¿este va a [Loc]?` | ¿Este va a la UNAM? | PR | A | PREP(a) | 92 | A1 |
| P122 | `¿este pasa por [Loc]?` | ¿Este pasa por Reforma? | PR | A | PREP(por) | 91 | A1 |
| P123 | `¿cuánto tarda?` | ¿Cuánto tarda el trayecto? | PR | A | intransitive use | 91 | A1 |
| P124 | `¿cuánto falta para [Loc]?` | ¿Cuánto falta para Hidalgo? | PR | A | PREP(para) | 91 | A1 |
| P125 | `¿cuántas paradas faltan?` | ¿Cuántas paradas faltan? | PR | A | AGR | 91 | A1 |
| P126 | `¿me avisa cuando [Cl]?` | ¿Me avisa cuando lleguemos? | PR,IO | D | UST,time clause | 91 | A2 |
| P127 | `¿puedo pagar con tarjeta?` | ¿Puedo pagar con tarjeta? | PR,MOD | A | PREP(con) | 91 | A0 |
| P128 | `¿aceptan efectivo?` | ¿Aceptan efectivo? | PR | A | plural impersonal | 91 | A0 |
| P129 | `¿tiene cambio?` | ¿Tiene cambio de quinientos? | PR | A | UST | 90 | A0 |
| P130 | `¿hay mesa para [num]?` | ¿Hay mesa para dos? | HAY,PR | A | PREP(para) | 90 | A0 |
| P131 | `¿tiene habitaciones?` | ¿Tiene habitaciones disponibles? | PR | A | UST,AGR | 90 | A1 |
| P132 | `¿hay boletos para [event]?` | ¿Hay boletos para hoy? | HAY,PR | A | MX lex | 90 | A1 |
| P133 | `[N] está disponible` | La habitación está disponible. | EST,PR | A | AGR | 90 | A1 |
| P134 | `quisiera reservar [N]` | Quisiera reservar una mesa. | COND | A | noun slot | 90 | A2 |
| P135 | `quiero cancelar [N]` | Quiero cancelar la reserva. | PR | A | noun slot | 90 | A1 |
| P136 | `quiero cambiar [N]` | Quiero cambiar la fecha. | PR | A | noun slot | 90 | A1 |
| P137 | `¿se puede [Inf]?` | ¿Se puede entrar? | SE,MOD | A | impersonal se | 89 | A1 |
| P138 | `¿se permite [Inf]?` | ¿Se permite fumar aquí? | SE,PR | A | impersonal/passive | 89 | A2 |
| P139 | `¿está abierto/cerrado?` | ¿Está abierto hoy? | EST,PR | A | AGR | 89 | A0 |
| P140 | `¿a qué hora abre/cierra?` | ¿A qué hora cierra el museo? | PR | A | interrogative | 89 | A0 |
| P141 | `¿desde cuándo [Cl]?` | ¿Desde cuándo vive aquí? | PR | A | PREP(desde) | 89 | A2 |
| P142 | `¿hasta cuándo [Cl]?` | ¿Hasta cuándo es válido? | PR | A | PREP(hasta) | 89 | A2 |
| P143 | `¿de qué tamaño/color es [N]?` | ¿De qué tamaño es? | SER,PR | A | PREP(de) | 88 | A1 |
| P144 | `¿qué incluye [N]?` | ¿Qué incluye el tour? | PR | A | OBJ | 88 | A1 |
| P145 | `¿cuál recomienda?` | ¿Cuál recomienda usted? | PR | A | UST | 88 | A1 |
| P146 | `¿qué me recomienda para [N]?` | ¿Qué me recomienda para desayunar? | PR,IO | D | PREP(para),UST | 88 | A1 |
| P147 | `¿hay algo más barato?` | ¿Hay algo más barato? | HAY,PR | A | comparison | 88 | A1 |
| P148 | `¿tiene otra talla?` | ¿Tiene otra talla? | PR | A | AGR | 88 | A1 |
| P149 | `¿hay sin [N]?` | ¿Hay sin gluten? | HAY,PR | A | ellipsis | 88 | A1 |
| P150 | `¿qué lleva [dish]?` | ¿Qué lleva el mole? | PR | A | MX food relevance | 88 | A1 |
| P151 | `¿es picante?` | ¿Es picante? | SER,PR | A | food semantics | 88 | A0 |
| P152 | `¿puedo ver [N]?` | ¿Puedo ver otra opción? | PR,MOD | A | OBJ | 87 | A0 |
| P153 | `¿lo puede envolver?` | ¿Lo puede envolver para regalo? | MOD,OBJ | B | CL | 87 | A2 |
| P154 | `¿qué pasó?` | ¿Qué pasó aquí? | PR | A | idiomatic past form | 87 | A1 |
| P155 | `¿qué necesitas?` | ¿Qué necesitas? | PR | A | UST | 87 | A0 |
| P156 | `¿qué prefieres?` | ¿Qué prefieres tomar? | PR | A | infinitive/noun | 87 | A1 |
| P157 | `¿qué opinas de [N]?` | ¿Qué opinas del lugar? | PR | A | PREP(de) | 87 | A1 |
| P158 | `¿te/le parece bien si [Cl]?` | ¿Le parece bien si pago ahora? | PR,IO | D | UST,clause | 87 | A2 |
| P159 | `¿quieres que [Cl]?` | ¿Quieres que te espere? | PR,SUBJ | B | SUBJ,CL | 87 | B1 |
| P160 | `¿quieres ir a [Loc]?` | ¿Quieres ir al mercado? | PR | A | PREP(a) | 86 | A1 |
| P161 | `vamos a [Inf]` | Vamos a comer primero. | PR,MOD | A | 1pl | 86 | A1 |
| P162 | `¿qué tal si [Cl]?` | ¿Qué tal si nos vemos mañana? | PR | A | colloquial suggestion | 86 | A2 |
| P163 | `mejor [Cl]` | Mejor tomamos un Uber. | PR | A | ellipsis/imperative nuance | 86 | A1 |
| P164 | `perdón, ¿cómo?` | Perdón, ¿cómo? | none | D | fixed repair | 86 | A0 |
| P165 | `perdón, no entendí` | Perdón, no entendí. | PR,PT | E | fixed repair | 86 | A0 |
| P166 | `no oigo bien` | No oigo bien. | PR | A | negation | 85 | A1 |
| P167 | `no recuerdo la palabra` | No recuerdo la palabra. | PR | A | OBJ | 85 | A1 |
| P168 | `¿cómo se llama esto?` | ¿Cómo se llama esto? | PR,SE | A | SE | 85 | A0 |
| P169 | `¿qué dice aquí?` | ¿Qué dice aquí? | PR | A | deictic | 85 | A1 |
| P170 | `¿es correcto así?` | ¿Es correcto así? | SER,PR | A | adverbial así | 85 | A1 |
| P171 | `¿puede confirmar si [Cl]?` | ¿Puede confirmar si salió el vuelo? | MOD,UST | D | embedded clause | 85 | B1 |
| P172 | `necesito saber si [Cl]` | Necesito saber si aceptan tarjetas. | PR | A | embedded clause | 85 | B1 |
| P173 | `¿es verdad que [Cl]?` | ¿Es verdad que cierran temprano? | SER,PR | A | confirmation | 84 | B1 |
| P174 | `entonces, ¿[rephrase]?` | Entonces, ¿voy por aquí? | PR | A | discourse marker | 84 | A1 |
| P175 | `¿quiere decir que [Cl]?` | ¿Quiere decir que no hay lugar? | PR | A | UST,embedded clause | 84 | B1 |
| P176 | `¿me puede dar un momento?` | ¿Me puede dar un minuto? | MOD,IO | D | fixed request | 84 | A2 |
| P177 | `un momento, por favor` | Un momento, por favor. | none | D | fixed phrase | 84 | A0 |
| P178 | `enseguida vuelvo` | Enseguida vuelvo. | PR | A | adverb placement | 83 | A1 |
| P179 | `ahora no puedo` | Ahora no puedo. | PR,MOD | A | negation | 83 | A0 |
| P180 | `más tarde, por favor` | Más tarde, por favor. | none | D | fixed phrase | 83 | A0 |
