import {readConfig} from './config.mjs';
import {BrevoError, checkBrevo} from './brevo.mjs';

const hints = {
  'missing-api-key': 'Legg BREVO_API_KEY i serverens miljøfil.',
  'invalid-sender': 'Sett BREVO_SENDER_EMAIL til avsenderen du har lagt inn i Brevo.',
  'invalid-sender-name': 'BREVO_SENDER_NAME kan ha inntil 100 tegn og ingen linjeskift.',
  'api-key-rejected': 'Brevo avviste API-nøkkelen. Kontroller nøkkelen på serveren.',
  'access-denied': 'Brevo avviste tilgangen. Kontroller nøkkelrettigheter og tillatt server-IP i Brevo.',
  'provider-rate-limit': 'Brevo har begrenset antall forespørsler. Prøv igjen senere.',
  'sender-not-found': 'Avsenderadressen finnes ikke blant avsenderne i denne Brevo-kontoen.',
  'sender-not-active': 'Bekreft og aktiver avsenderen i Brevo først.',
  'network-or-timeout': 'Kunne ikke nå Brevo. Kontroller nettverk og prøv igjen.',
};
try {
  if (process.argv.slice(2).some(arg => arg !== '--sandbox')) throw new Error('Usage: node --env-file=/etc/elix-accounts.env check-brevo.mjs [--sandbox]');
  const result = await checkBrevo(readConfig(), {sandbox: process.argv.includes('--sandbox')});
  console.log('Brevo: API-tilgang og aktiv avsender er kontrollert.');
  if (result.sandbox) console.log('Brevo: testforespørselen ble godkjent i sandbox-modus.');
  console.log('Ingen e-post er sendt. Faktisk levering til innboksen er ikke testet.');
} catch (error) {
  console.error(error instanceof BrevoError ? (hints[error.reason] || `Brevo-kontrollen feilet (${error.reason}).`) : error.message);
  process.exitCode = 1;
}
