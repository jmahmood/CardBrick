# Tier 5 — Travel and Safety (P301–P380)

**Before generating from this file, read [`00-generation-guide.md`](00-generation-guide.md).**

`tier: "travel_safety"` for every item generated from this file.

Route verification, transit, airports, taxis, phones, safe-place
language, lost objects, and document issues. **This is the
highest-priority tier for the Mexico City trip** — current official
guidance flags Metro pickpocketing, taxi/rideshare caution, and
emergency access, so these patterns should get generated (and
studied) first. Dense with `UST` (formal address to drivers/officers)
and `PREP(x)` constraints — strong MCQ tier.

Suggested first batch: the safety micro-pack — P339, P341, P342,
P347–P352, P362–P364 — before working sequentially through the rest.

| ID | Canonical template | Example | Pre | Xf | Generation constraints | Score | Gate |
|---|---|---|---|---|---|---:|---|
| P301 | `¿dónde está la estación?` | ¿Dónde está la estación del Metro? | EST,PR | A | travel noun | 96 | A0 |
| P302 | `¿cómo llego al centro?` | ¿Cómo llego al centro histórico? | PR | A | PREP(a) | 96 | A1 |
| P303 | `¿voy bien para [Loc]?` | ¿Voy bien para Reforma? | PR | A | PREP(para) | 96 | A1 |
| P304 | `¿esta es la salida correcta?` | ¿Esta es la salida correcta? | SER,PR | A | demonstrative AGR | 95 | A1 |
| P305 | `¿qué ruta recomienda?` | ¿Qué ruta recomienda de noche? | PR | A | UST | 95 | A2 |
| P306 | `quiero ir a [Loc]` | Quiero ir al aeropuerto. | PR | A | PREP(a) | 95 | A0 |
| P307 | `lléveme a [Loc], por favor` | Lléveme al hotel, por favor. | IMPV | D | UST,PREP(a) | 95 | A2 |
| P308 | `aquí me bajo` | Aquí me bajo. | PR,REFL | D | REFL | 95 | A1 |
| P309 | `¿me puede dejar aquí?` | ¿Me puede dejar aquí? | MOD,IO | D | UST | 95 | A2 |
| P310 | `¿cuánto me cobra hasta [Loc]?` | ¿Cuánto me cobra hasta Condesa? | PR,IO | D | PREP(hasta),UST | 95 | A2 |
| P311 | `use el taxímetro, por favor` | Use el taxímetro, por favor. | IMPV | D | UST,MX | 95 | B1 |
| P312 | `prefiero pagar al final` | Prefiero pagar al final. | PR | A | transaction travel | 94 | A1 |
| P313 | `¿es taxi autorizado?` | ¿Es taxi autorizado? | SER,PR | A | UST not needed | 94 | A1 |
| P314 | `la placa es [X]` | La placa es A123CD. | SER,PR | A | alphanumeric slot | 94 | A1 |
| P315 | `¿usted es mi conductor?` | ¿Usted es mi conductor? | SER,PR | A | UST | 94 | A1 |
| P316 | `la app dice [Amt]` | La app dice ciento noventa pesos. | PR | A | amount format | 94 | A1 |
| P317 | `¿me ayudas a pedir un Uber/taxi?` | ¿Me ayudas a pedir un Uber? | PR | D | infinitive complement | 94 | A1 |
| P318 | `¿dónde está la salida/entrada?` | ¿Dónde está la salida? | EST,PR | A | paired nouns | 94 | A0 |
| P319 | `¿dónde está la taquilla?` | ¿Dónde está la taquilla? | EST,PR | A | transit noun | 94 | A1 |
| P320 | `¿dónde recargo la tarjeta?` | ¿Dónde recargo la tarjeta? | PR | A | OBJ | 94 | A1 |
| P321 | `quiero una tarjeta del Metro` | Quiero una tarjeta del Metro. | PR | A | de-phrase | 93 | A1 |
| P322 | `¿dónde tomo la Línea [N]?` | ¿Dónde tomo la Línea 3? | PR | A | MX transit naming | 93 | A1 |
| P323 | `¿tengo que transbordar?` | ¿Tengo que transbordar? | PR,MOD | A | transit verb | 93 | A2 |
| P324 | `¿en qué dirección va?` | ¿En qué dirección va? | PR | A | PREP(en) | 93 | A1 |
| P325 | `¿este tren va a [station]?` | ¿Este tren va a Hidalgo? | PR | A | PREP(a) | 93 | A1 |
| P326 | `¿el último tren sale a qué hora?` | ¿El último tren sale a qué hora? | PR | A | schedule frame | 93 | A2 |
| P327 | `¿dónde está el andén [N]?` | ¿Dónde está el andén dos? | EST,PR | A | station lexicon | 92 | A2 |
| P328 | `¿dónde recojo mi equipaje?` | ¿Dónde recojo mi equipaje? | PR | A | airport context | 92 | A1 |
| P329 | `perdí mi maleta` | Perdí mi maleta. | PT | E | lost-item frame | 92 | A2 |
| P330 | `mi vuelo se retrasó` | Mi vuelo se retrasó dos horas. | PT,REFL | E | REFL | 92 | A2 |
| P331 | `perdí el pase de abordar` | Perdí el pase de abordar. | PT | E | airport lexicon | 92 | A2 |
| P332 | `necesito ir al aeropuerto` | Necesito ir al aeropuerto ya. | PR | A | PREP(a) | 92 | A0 |
| P333 | `¿cuánto tiempo antes tengo que llegar?` | ¿Cuánto tiempo antes tengo que llegar? | PR,MOD | A | schedule frame | 92 | A2 |
| P334 | `¿dónde está migración?` | ¿Dónde está migración? | EST,PR | A | airport frame | 91 | A1 |
| P335 | `¿dónde está aduana?` | ¿Dónde está aduana? | EST,PR | A | airport frame | 91 | A1 |
| P336 | `¿qué documentos necesito?` | ¿Qué documentos necesito? | PR | A | travel admin | 91 | A1 |
| P337 | `aquí está mi pasaporte` | Aquí está mi pasaporte. | EST,PR | A | deictic | 91 | A0 |
| P338 | `mi pasaporte está en [Loc]` | Mi pasaporte está en el hotel. | EST,PR | A | PREP(en) | 91 | A0 |
| P339 | `perdí mi pasaporte` | Perdí mi pasaporte. | PT | E | high-urgency travel | 96 | A2 |
| P340 | `necesito una copia del reporte` | Necesito una copia del reporte. | PR | A | de-phrase | 90 | A2 |
| P341 | `perdí mi cartera` | Perdí mi cartera. | PT | E | lost-item frame | 95 | A2 |
| P342 | `me robaron el celular` | Me robaron el celular. | PT,IO | E | IO affected person | 96 | A2 |
| P343 | `siento que me siguen` | Siento que me siguen. | PR | A | embedded clause | 96 | B1 |
| P344 | `no conozco esta zona` | No conozco esta zona. | PR | A | deictic | 95 | A1 |
| P345 | `mejor aquí no` | Mejor aquí no. | none | D | truncated safety phrase | 95 | A1 |
| P346 | `necesito volver a [safe place]` | Necesito volver al hotel. | PR,MOD | A | PREP(a) | 95 | A1 |
| P347 | `¿puede llamar a la policía?` | ¿Puede llamar a la policía? | MOD | D | UST,PA | 97 | A1 |
| P348 | `¿puede ayudarme a llamar al 911?` | ¿Puede ayudarme a llamar al 911? | MOD | D | UST,MX emergency | 97 | A2 |
| P349 | `¿dónde hay un lugar seguro?` | ¿Dónde hay un lugar seguro? | HAY,PR | A | safety lexicon | 96 | A1 |
| P350 | `¿hay un cajero cerca?` | ¿Hay un cajero cerca? | HAY,PR | A | nearby adverbial | 90 | A0 |
| P351 | `no quiero sacar dinero aquí` | No quiero sacar dinero aquí. | PR | A | safety frame | 95 | A1 |
| P352 | `¿hay una farmacia abierta?` | ¿Hay una farmacia abierta? | HAY,PR | A | AGR | 94 | A1 |
| P353 | `necesito agua embotellada` | Necesito agua embotellada. | PR | A | AGR | 90 | A0 |
| P354 | `¿dónde puedo cargar mi celular?` | ¿Dónde puedo cargar mi celular? | MOD,PR | A | infinitive complement | 90 | A1 |
| P355 | `no tengo señal/batería` | No tengo batería. | PR | A | noun slot | 90 | A0 |
| P356 | `¿me comparte el wifi un momento?` | ¿Me comparte el wifi un momento? | PR,IO | D | colloquial service use | 89 | A2 |
| P357 | `mándeme la ubicación` | Mándeme la ubicación, por favor. | IMPV,OBJ,IO | D | CL,UST | 89 | B1 |
| P358 | `compárteme tu ubicación` | Compárteme tu ubicación. | IMPV,OBJ,IO | D | CL | 89 | B1 |
| P359 | `ya voy en camino` | Ya voy en camino. | PR | A | fixed collocation | 88 | A2 |
| P360 | `ya llegué` | Ya llegué. | PT | E | high-frequency travel update | 88 | A1 |
| P361 | `estoy cerca de [Loc]` | Estoy cerca de la estación. | EST,PR | A | PREP(de) | 88 | A1 |
| P362 | `estoy perdido/a` | Estoy perdida. | EST,PR | A | AGR | 95 | A0 |
| P363 | `me equivoqué de estación` | Me equivoqué de estación. | PT,REFL | E | PREP(de),REFL | 90 | A2 |
| P364 | `me pasé de parada` | Me pasé de parada. | PT,REFL | E | PREP(de),REFL | 92 | B1 |
| P365 | `¿dónde hago transbordo?` | ¿Dónde hago transbordo? | PR | A | transit noun | 89 | A2 |
| P366 | `la tarjeta no pasa` | La tarjeta no pasa. | PR | A | Mexico transit/payment | 89 | A1 |
| P367 | `¿trae cambio?` | ¿Trae cambio? | PR | A | UST,MX-common | 88 | A1 |
| P368 | `quiero ir por la ruta más directa` | Quiero ir por la ruta más directa. | PR | A | PREP(por),comparison | 90 | A2 |
| P369 | `evite calles solas, por favor` | Evite calles solas, por favor. | IMPV | D | UST,safety | 90 | B1 |
| P370 | `no se detenga aquí` | No se detenga aquí. | IMPV,REFL | D | negative imperative | 94 | B1 |
| P371 | `cierre los seguros, por favor` | Cierre los seguros, por favor. | IMPV | D | UST | 92 | B1 |
| P372 | `¿puede esperar aquí un minuto?` | ¿Puede esperar aquí un minuto? | MOD | D | UST,time | 88 | A2 |
| P373 | `¿esta entrada es oficial?` | ¿Esta entrada es oficial? | SER,PR | A | safety screening | 89 | A1 |
| P374 | `¿dónde está el módulo de información?` | ¿Dónde está el módulo de información? | EST,PR | A | admin travel noun | 88 | A2 |
| P375 | `¿hay lockers/guarda equipaje?` | ¿Hay guarda equipaje? | HAY,PR | A | travel jargon | 87 | A2 |
| P376 | `¿puedo dejar esto aquí?` | ¿Puedo dejar esto aquí? | PR,MOD | A | deictic | 87 | A0 |
| P377 | `¿dónde puedo comprar una SIM/eSIM?` | ¿Dónde puedo comprar una SIM? | MOD,PR | A | tech borrowing | 87 | A2 |
| P378 | `necesito un cargador` | Necesito un cargador USB-C. | PR | A | tech noun | 87 | A0 |
| P379 | `¿me presta su teléfono?` | ¿Me presta su teléfono para una llamada? | PR,IO | D | UST | 93 | A2 |
| P380 | `si me pasa algo, llame a este número` | Si me pasa algo, llame a este número. | PR,IMPV | D | SI+pres,UST | 94 | B1 |
