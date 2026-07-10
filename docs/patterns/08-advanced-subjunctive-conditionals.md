# Tier 8 — Advanced, Subjunctive, and Conditionals (P461–P500)

**Before generating from this file, read [`00-generation-guide.md`](00-generation-guide.md).**

`tier: "advanced_subjunctive_conditionals"` for every item generated
from this file.

A compact high-payoff set for hedging, polite requests,
recommendations, uncertainty, contingency, and hypotheticals — B1–C1
gate. This is mood/register territory (`SUBJ`, `COND`), not
discrete-rule territory, so **MCQ is weaker here than in the earlier
tiers** — use it sparingly, only where a row's constraint names a
genuinely binary contrast (e.g. indicative-vs-subjunctive after a
specific trigger verb). Prioritize `production` items, and keep
`prompt_en` cues concrete/situational — these patterns land best
when tied to an actual travel problem, not an abstract grammar
prompt.

Suggested first batch: P461–P470 (subjunctive triggers: espero que,
quiero que, necesito que...).

| ID | Canonical template | Example | Pre | Xf | Generation constraints | Score | Gate |
|---|---|---|---|---|---|---:|---|
| P461 | `espero que [SUBJ Cl]` | Espero que llegue a tiempo. | SUBJ | A | mood trigger | 76 | B1 |
| P462 | `quiero que [SUBJ Cl]` | Quiero que me ayude. | SUBJ,OBJ | D | SUBJ,CL,UST | 76 | B1 |
| P463 | `necesito que [SUBJ Cl]` | Necesito que revise esto. | SUBJ | D | SUBJ,UST | 76 | B1 |
| P464 | `prefiero que [SUBJ Cl]` | Prefiero que sea en efectivo. | SUBJ | D | SUBJ | 75 | B1 |
| P465 | `es importante que [SUBJ Cl]` | Es importante que lo sepa. | SUBJ,OBJ | D | impersonal + SUBJ | 75 | B1 |
| P466 | `es mejor que [SUBJ Cl]` | Es mejor que espere aquí. | SUBJ | D | impersonal + SUBJ | 75 | B1 |
| P467 | `es posible que [SUBJ Cl]` | Es posible que llueva. | SUBJ | D | uncertainty | 74 | B1 |
| P468 | `puede ser que [SUBJ Cl]` | Puede ser que cierre temprano. | SUBJ,MOD | D | uncertainty | 74 | B1 |
| P469 | `dudo que [SUBJ Cl]` | Dudo que acepten dólares. | SUBJ | D | negative certainty | 73 | B1 |
| P470 | `no creo que [SUBJ Cl]` | No creo que haya espacio. | SUBJ | D | negative belief trigger | 73 | B1 |
| P471 | `aunque [SUBJ/IND Cl], [Cl]` | Aunque llueva, voy. | SUBJ | C | mood depends on certainty | 73 | B1 |
| P472 | `[Cl] para que [SUBJ Cl]` | Hablo despacio para que entienda. | SUBJ | C | PREP(para),purpose | 73 | B1 |
| P473 | `antes de que [SUBJ Cl]` | Avísame antes de que salgas. | SUBJ | C | time trigger | 72 | B1 |
| P474 | `cuando [SUBJ future-ref], [Cl]` | Cuando llegue, te llamo. | SUBJ | C | future time reference | 72 | B1 |
| P475 | `en cuanto [SUBJ Cl], [Cl]` | En cuanto termine, salgo. | SUBJ | C | time trigger | 72 | B1 |
| P476 | `si [PR Cl], [future/imp]` | Si llegas tarde, avísame. | PR,IMPV | C | no future in si-clause | 72 | B1 |
| P477 | `si [IMP SUBJ], [COND Cl]` | Si tuviera tiempo, iría. | SUBJ,COND | C | classic hypothetical | 71 | B2 |
| P478 | `si [plup SUBJ], [COND perf Cl]` | Si hubiera sabido, habría salido antes. | SUBJ,COND,PF | C | advanced hypothetical | 69 | C1 |
| P479 | `me gustaría que [SUBJ Cl]` | Me gustaría que me confirmara. | COND,SUBJ | D | SUBJ,UST | 74 | B1 |
| P480 | `sería mejor [Inf/que SUBJ]` | Sería mejor tomar un taxi. | COND,SUBJ | A | infinitive vs clause | 74 | B1 |
| P481 | `querría [Inf]` | Querría cambiar de asiento. | COND | A | formal-polite | 73 | B1 |
| P482 | `podría [Inf]` | ¿Podría hablar más despacio? | COND,MOD | B | UST | 74 | B1 |
| P483 | `habría que [Inf]` | Habría que revisar la reserva. | COND,MOD | A | impersonal | 70 | B2 |
| P484 | `debería [Inf]` | Debería descansar hoy. | COND,MOD | A | advice | 72 | B1 |
| P485 | `tendría que [Inf]` | Tendría que confirmarlo primero. | COND,MOD | A | obligation hedge | 72 | B1 |
| P486 | `no quisiera que [SUBJ Cl]` | No quisiera que hubiera un problema. | COND,SUBJ | D | polite negative | 70 | B2 |
| P487 | `lo que necesito es [N/Inf]` | Lo que necesito es descansar. | PR | A | cleft/focus | 70 | B1 |
| P488 | `lo que más me preocupa es [N/que Cl]` | Lo que más me preocupa es perder el pasaporte. | PR | A | focus structure | 69 | B2 |
| P489 | `el problema es que [Cl]` | El problema es que no hay señal. | PR | A | focus structure | 70 | B1 |
| P490 | `por si [Cl]` | Llevo efectivo por si falla la red. | PR | A | contingency frame | 69 | B2 |
| P491 | `en caso de que [SUBJ Cl]` | En caso de que llueva, llevo paraguas. | SUBJ | C | contingency frame | 69 | B2 |
| P492 | `a menos que [SUBJ Cl]` | No salgo a menos que me llames. | SUBJ | C | negative condition | 68 | B2 |
| P493 | `con tal de que [SUBJ Cl]` | Voy con tal de que sea temprano. | SUBJ | C | condition frame | 68 | B2 |
| P494 | `dondequiera que [SUBJ Cl]` | Dondequiera que vaya, llevo copia del pasaporte. | SUBJ | C | free relative | 66 | C1 |
| P495 | `busco a alguien que [SUBJ Cl]` | Busco a alguien que hable inglés. | SUBJ,PA | A | relative with non-specific antecedent | 71 | B1 |
| P496 | `necesito un lugar donde [SUBJ Cl]` | Necesito un lugar donde pueda cargar el celular. | SUBJ,MOD | A | relative place clause | 71 | B1 |
| P497 | `ojalá [SUBJ Cl]` | Ojalá no cierre temprano. | SUBJ | D | fixed trigger | 72 | B1 |
| P498 | `según [N/Cl]` | Según la app, faltan diez minutos. | PR | A | evidential frame | 68 | B2 |
| P499 | `parece que / al parecer [Cl]` | Al parecer, cambió la ruta. | PR | A | evidential hedge | 68 | B1 |
| P500 | `me habría gustado [Inf/que ...]` | Me habría gustado quedarme más tiempo. | COND,PF | A | counterfactual regret | 66 | C1 |
