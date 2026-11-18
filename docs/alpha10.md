# ELIX Browser fra Stable 1 til Alpha 10

Arbeidsgrunnlag fra [ELIX Browser-tavla](https://trello.projectelix.no/elix-browser), lest 18. november 2026. Tavla viser gjeldende teststatus. Et versjonsmål er ikke en publiseringsdato.

| Leveranse | Grunnlag og godkjenningskrav |
| --- | --- |
| Stable 1 · 0.1.4 | Bygger på Alpha 9. Kontroller Windows-installasjon, oppgradering med en eksisterende profil og den eksakte pakken som skal publiseres. ELIX-68 samler resultatene. |
| Alpha 10 · 0.1.5-dev.10 | Utviklerkanalen. Prøv funksjonene nedenfor på Windows og registrer resultatene på ELIX-56 før utgivelse. |
| Alpha 11 | Gaming Mode, Twitch-integrasjon, Game Launcher, Discord Rich Presence, ELIX Dev Panel, JSON Formatter, API Tester, Responsive Preview og Screenshot Studio. |
| Alpha 12 | Annonseblokkering, Privacy Dashboard, nettstedstillatelser, passordbehandling og sikkerhetsvarsler. |
| Alpha 13 | ELIX Accounts, ELIX Sync, Theme Studio og planlegging av Google-/Firefox-import. Inkluder samtykke og leverandørstøtte i vurderingen av hvilke data som kan importeres. |
| Beta 3 | ELIX Extensions, ELIX Community og ELIX Updates. Manifest, signatur og gjenoppretting har tidlige tester fra Alpha 10. |

## Stable 1

Gjenta ni scenarioer fra tidligere utgaver på kandidaten. ELIX-17 dekker fokus i ny fane, ELIX-29 tannhjulmenyen, ELIX-28 bokmerkegrupper, ELIX-18 og ELIX-21 favicons, ELIX-25 dvale for én fane, ELIX-19 synlig fanedrag, ELIX-26 fullskjerm og ELIX-27 gjentatt rulling til siste PDF-side.

Etterprøv også ELIX-20 ved å trekke første, midterste og siste fane ut av en gruppe. ELIX-23 gjelder original lyd og video ved pop-out og retur. ELIX-33 skiller søketekst med kolon fra protokoller. ELIX-34 gjelder faktisk filnedlasting via en gyldig omdirigering. Registrer commit, bygg-ID, sjekksum og signatur sammen med installasjons- og oppgraderingsresultatet.

## Alpha 10

| Kort | Hva skal testes? |
| --- | --- |
| ELIX-36 · Dra og slipp | Slipp et bokmerke på en lukket gruppe, flytt en fane inn og ut av en gruppe og til delt visning. Avbryt med Escape uten å flytte noe. |
| ELIX-37 · Mediespiller | Prøv forrige/neste på en faktisk spilleliste. Åpne kildefanen og velg å beholde eller avslutte spilleren. Sjekk avspillingsposisjon og lyd. |
| ELIX-22 · Innlogging | Prøv ny konto, oppgradering fra e-postkode, passordgjenoppretting og Discord-retur til vinduet som startet innloggingen. Kontotjenesten og leverandørkonfigurasjonen inngår i ende-til-ende-testen. |
| ELIX-32 · Passord | Sjekk aktiv profil og bekreftelsesdialog med en testprofil. Visning av enkeltpassord etter ny godkjenning er et åpent produktkrav. |
| ELIX-38 · Automatiske fanegrupper | Bruk to domener, gi gruppene navn/farge, flytt én fane manuelt og kontroller overstyringen etter ny regelkjøring og omstart. |
| ELIX-39 · Fanesøk | Åpne Ctrl+Shift+A med like fanetitler i to arbeidsområder. Prøv tittel, URL og arbeidsområde. Sjekk miniatyr eller reservevisning og at samme fane aktiveres. |
| ELIX-40 · Lagre økter | Åpne en navngitt økt med grupper, to arbeidsområder og delt visning. Gjenta etter avbrutt prosess. Kontroller rekkefølge, aktiv fane, begge sider og at private faner er utelatt. |
| ELIX-41 · Vertikale faner | Prøv venstre og høyre side, juster bredden, flytt og fest faner, fold inn og ut. Kontroller plassering og aktiv fane etter omstart. |
| ELIX-42 · Smart fane-dvale | Sammenlign minnebruk for samme fanesett. Lyd, møte, aktiv nedlasting og festet fane skal fortsette. En parkert fane skal våkne ved valg. |
| ELIX-43 · Arbeidsområdeprofiler | Bruk samme nettsted i Jobb, Privat og Gaming. Kontroller informasjonskapsler, pålogging, nettlagring og tillatelser i hver profil etter omstart. |
| ELIX-69 · Mellom to vinduer | Dra en fane til et eksisterende ELIX-vindu. Kontroller ankomst før originalfanen lukkes. Prøv også at mottakeren lukkes eller avviser underveis. |

ELIX-31 om API til hjelp og søk mangler et konkret søkeeksempel. Spørsmålet om adressefelt eller egen hjelpeside ligger i kortet.
