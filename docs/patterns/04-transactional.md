# Tier 4 — Transactional (P241–P300)

**Before generating from this file, read [`00-generation-guide.md`](00-generation-guide.md).**

`tier: "transactional"` for every item generated from this file.

Food, shopping, lodging, tickets, payment, changes, receipts, and
booking — disproportionately useful for a traveler using a handheld
drill app. Mostly A0–A2. Good MCQ material where `UST` or `CL`
constraints appear (ordering/payment exchanges are usually with
strangers → usted).

Suggested first batch: P241–P260 (money/payment core).

| ID | Canonical template | Example | Pre | Xf | Generation constraints | Score | Gate |
|---|---|---|---|---|---|---:|---|
| P241 | `quiero comprar [N]` | Quiero comprar agua. | PR | A | OBJ | 88 | A0 |
| P242 | `busco [N]` | Busco una batería externa. | PR | A | OBJ | 88 | A0 |
| P243 | `necesito [N]` | Necesito bloqueador solar. | PR | A | OBJ | 88 | A0 |
| P244 | `¿cuánto cuesta [N]?` | ¿Cuánto cuesta esta playera? | PR | A | AGR,MX-neutral | 88 | A0 |
| P245 | `[N] cuesta [Amt]` | Cuesta ciento veinte pesos. | PR | A | amount format | 87 | A0 |
| P246 | `[N] vale [Amt]` | Vale doscientos pesos. | PR | A | amount format | 86 | A1 |
| P247 | `¿cuánto es en total?` | ¿Cuánto es en total? | PR | A | fixed phrase | 87 | A0 |
| P248 | `¿me cobra, por favor?` | ¿Me cobra, por favor? | PR,IO | D | UST | 87 | A1 |
| P249 | `¿me puede cobrar aquí?` | ¿Me puede cobrar aquí? | MOD,IO | D | UST | 86 | A2 |
| P250 | `pago en efectivo` | Pago en efectivo. | PR | A | PREP(en) | 87 | A0 |
| P251 | `pago con tarjeta` | Pago con tarjeta. | PR | A | PREP(con) | 87 | A0 |
| P252 | `¿me trae la cuenta?` | ¿Me trae la cuenta? | PR,IO | D | UST | 87 | A1 |
| P253 | `la cuenta, por favor` | La cuenta, por favor. | none | D | fixed phrase | 87 | A0 |
| P254 | `¿puede dividir la cuenta?` | ¿Puede dividir la cuenta? | MOD | D | UST | 86 | A2 |
| P255 | `¿incluye propina?` | ¿Incluye propina? | PR | A | service setting | 86 | A1 |
| P256 | `sin bolsa, por favor` | Sin bolsa, por favor. | none | D | fixed phrase | 86 | A0 |
| P257 | `con/sin [ingredient]` | Sin cebolla, por favor. | none | D | fixed frame | 86 | A0 |
| P258 | `para llevar` | Es para llevar. | none | D | fixed MX-neutral | 86 | A0 |
| P259 | `para aquí` | Es para aquí. | none | D | fixed phrase | 86 | A0 |
| P260 | `¿qué me recomienda?` | ¿Qué me recomienda? | PR,IO | D | UST | 86 | A1 |
| P261 | `quiero [dish/drink]` | Quiero un chocolate caliente. | PR | A | noun slot | 86 | A0 |
| P262 | `me da [qty] de [N]` | ¿Me da medio kilo de tortillas? | PR,IO | D | qty syntax | 85 | A1 |
| P263 | `me pone [qty] de [N]` | ¿Me pone dos tacos de pastor? | PR,IO | D | regional food use | 85 | A1 |
| P264 | `quisiera pedir [N]` | Quisiera pedir enchiladas. | COND | A | restaurant register | 85 | A2 |
| P265 | `quisiera reservar una mesa` | Quisiera reservar una mesa para cuatro. | COND | A | reservation slot | 85 | A2 |
| P266 | `mesa para [num]` | Mesa para tres, por favor. | none | D | fixed phrase | 85 | A0 |
| P267 | `¿tienen menú?` | ¿Tienen menú en inglés? | PR | A | plural impersonal | 85 | A0 |
| P268 | `¿qué lleva [dish]?` | ¿Qué lleva la salsa verde? | PR | A | food composition | 85 | A1 |
| P269 | `soy alérgico/a a [N]` | Soy alérgico al cacahuate. | SER,PR | A | AGR,PREP(a) | 85 | A1 |
| P270 | `sin picante, por favor` | Sin picante, por favor. | none | D | fixed phrase | 85 | A0 |
| P271 | `poco picante, por favor` | Poco picante, por favor. | none | D | fixed phrase | 84 | A0 |
| P272 | `[food] bien cocido/a` | La carne bien cocida, por favor. | PR | D | AGR | 84 | A1 |
| P273 | `un vaso de [drink]` | Un vaso de agua, por favor. | none | D | qty frame | 84 | A0 |
| P274 | `otra/otro [N]` | Otro café, por favor. | none | D | AGR | 84 | A0 |
| P275 | `¿me puede traer más [N]?` | ¿Me puede traer más salsa? | MOD,IO | D | quantity | 84 | A2 |
| P276 | `esto no es lo que pedí` | Esto no es lo que pedí. | PT | E | contrastive relative | 84 | B1 |
| P277 | `falta [N] en mi pedido` | Falta una bebida en mi pedido. | PR | A | PREP(en) | 84 | A1 |
| P278 | `quiero cambiar esto` | Quiero cambiar esto. | PR | A | deictic object | 83 | A1 |
| P279 | `quiero devolver esto` | Quiero devolver esto. | PR | A | deictic object | 83 | A1 |
| P280 | `quiero otra talla` | Quiero otra talla. | PR | A | clothing lexicon | 83 | A1 |
| P281 | `¿lo tiene en [size/color]?` | ¿Lo tiene en negro? | PR,OBJ | B | CL | 83 | A1 |
| P282 | `¿puedo probármelo?` | ¿Puedo probármelo? | MOD,REFL,OBJ | B | CL,REFL | 83 | A2 |
| P283 | `me queda bien/mal` | Me queda bien esta chamarra. | PR | A | clothing fit | 83 | A2 |
| P284 | `me queda grande/pequeño` | Me queda grande. | PR | A | AGR | 83 | A2 |
| P285 | `¿hay descuento?` | ¿Hay descuento en efectivo? | HAY,PR | A | retail context | 82 | A1 |
| P286 | `¿me puede hacer precio?` | ¿Me puede hacer precio? | MOD,IO | D | bargaining register | 82 | B1 |
| P287 | `necesito factura/recibo` | Necesito un recibo. | PR | A | noun slot | 82 | A1 |
| P288 | `¿me da un recibo?` | ¿Me da un recibo, por favor? | PR,IO | D | UST | 82 | A1 |
| P289 | `¿cuál es la clave del wifi?` | ¿Cuál es la clave del wifi? | SER,PR | A | possessive/de phrase | 82 | A1 |
| P290 | `el wifi no funciona` | El wifi no funciona en mi cuarto. | PR | A | negation | 82 | A0 |
| P291 | `tengo una reserva a nombre de [Name]` | Tengo una reserva a nombre de Hellwig. | PR | A | formulaic naming | 82 | A1 |
| P292 | `quiero hacer check-in` | Quiero hacer check-in. | PR | A | borrowing; optional "registrarme" | 82 | A1 |
| P293 | `quiero hacer check-out` | Quiero hacer check-out ahora. | PR | A | borrowing; optional "salida" | 82 | A1 |
| P294 | `¿incluye desayuno?` | ¿La tarifa incluye desayuno? | PR | A | lodging frame | 81 | A1 |
| P295 | `necesito una toalla más` | Necesito una toalla más. | PR | A | quantity | 81 | A1 |
| P296 | `¿pueden guardar mi equipaje?` | ¿Pueden guardar mi equipaje? | PR | D | plural service staff | 81 | A2 |
| P297 | `¿a qué hora sale/llega [transport]?` | ¿A qué hora sale el autobús? | PR | A | schedule frame | 81 | A1 |
| P298 | `¿desde qué andén/puerta sale?` | ¿Desde qué puerta sale? | PR | A | transit lexicon | 81 | A2 |
| P299 | `quiero un boleto de ida/vuelta` | Quiero un boleto de ida y vuelta. | PR | A | MX lex | 81 | A1 |
| P300 | `¿hay lugares disponibles?` | ¿Hay lugares disponibles para hoy? | HAY,PR | A | plural agreement | 81 | A1 |
