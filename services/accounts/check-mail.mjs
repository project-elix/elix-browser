import {readConfig} from './config.mjs';
import {BrevoError, checkBrevo} from './brevo.mjs';
import {checkSmtp} from './smtp.mjs';

try {
  if (process.argv.length !== 2) throw new Error('Usage: node --env-file=/etc/elix-accounts.env check-mail.mjs');
  const config = readConfig();
  if (config.mailTransport === 'smtp') {
    await checkSmtp(config);
    console.log('SMTP: TLS og innlogging er kontrollert. Avsendergodkjenning er ikke testet.');
  } else {
    await checkBrevo(config);
    console.log('Brevo API: tilgang og aktiv avsender er kontrollert.');
  }
  console.log('Ingen e-post er sendt. Levering til innboksen er ikke testet.');
} catch (error) {
  console.error(error instanceof BrevoError ? `E-postkontrollen feilet (${error.reason}).` : error.message);
  process.exitCode = 1;
}
