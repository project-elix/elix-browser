# Stable 1 – Windows x64

Stable 1 har versjonsnummer **0.1.4** og bygger på offentlig Alpha 9, **0.1.4-dev.9**. Nye profiler bruker Stable-kanalen. Eksisterende profiler beholder kanalvalget sitt. Den første Stable-utgaven er klar for nedlasting når den signerte installasjonsfilen og kanalen er publisert.

## Godkjenning av kandidaten

På Windows kjøres skalltestene og de native WebView2-testene fra kandidatens kildekode. Releasejobben tester også den signerte installasjonsfilen på en ren installasjon og oppå den publiserte Alpha 9. Oppgraderingstesten kontrollerer profilidentitet, lagrede innstillinger, oppdateringskanal, bokmerkegruppe, historikk og en fil i profilens WebView2-mappe.

Knytt hvert resultat til samme commit og Windows-bygg-ID. Ved godkjenning av publisering skal installerens SHA-256 og bekreftede oppdateringssignatur samsvare med artefakten `signed-stable1-profile-review`. Registrer hvilke tester som ble kjørt i den instrumenterte testutgaven, og hvilke som er prøvd på den signerte installasjonen.

| Kort | Kontroll på Windows |
| --- | --- |
| ELIX-17 | Åpne en ny fane og skriv og søk direkte i adressefeltet. |
| ELIX-29 | Åpne tannhjulet og bruk delt visning, faner og arbeidsområde. |
| ELIX-28 | Opprett bokmerkegrupper, flytt bokmerker og åpne gruppene igjen etter omstart. |
| ELIX-18 | Kontroller at nettstedets ikon vises på fanen etter navigasjon. |
| ELIX-21 | Bokmerk siden og kontroller at ikonet vises på bokmerket. |
| ELIX-25 | Legg én valgt fane i dvale og kontroller at de øvrige fortsetter. |
| ELIX-19 | Dra en fane, følg den synlige forhåndsvisningen og slipp fanen i ny posisjon. |
| ELIX-26 | Gå inn og ut av fullskjerm i en video og kontroller begge vindustilstandene. |
| ELIX-27 | Rull med musehjul til siste side i en lang PDF. Gjenta testen og bekreft siste side hver gang. |
| ELIX-20 | Flytt en fane inn i en gruppe og ut igjen. Kontroller rekkefølge og medlemskap. |
| ELIX-23 | Flytt avspilling til minispilleren og tilbake. Kontroller at bare én lydkilde spiller. |
| ELIX-33 | Søk med kolon i vanlig søketekst og kontroller at nettadresser behandles som adresser. |
| ELIX-34 | Lim inn den offentlige Alpha 9-signaturens GitHub-lenke og deretter en fersk signert CDN-adresse i ELIX. Kontroller to fullførte filer med riktig filnavn og SHA-256. |

Før publiseringsbeslutningen skal Trello-kortet for Stable 1 inneholde godkjenningsresultat eller konkret feil for hver rad, bygglenke, commit, installerens SHA-256 og referanse til signaturen. Resultatene fra Alpha 9 er sammenligningsgrunnlag.
