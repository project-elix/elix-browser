import {sendLoginEmail, validateBrevo} from './brevo.mjs';
import {sendSmtpLoginEmail, validateSmtp} from './smtp.mjs';

export function validateMail(config) {
  if ((config.mailTransport ?? 'brevo-api') === 'smtp') validateSmtp(config);
  else if ((config.mailTransport ?? 'brevo-api') === 'brevo-api') validateBrevo(config);
  else throw new Error('MAIL_TRANSPORT must be brevo-api or smtp');
}

export function mailConfigured(config) {
  try { validateMail(config); return true; } catch { return false; }
}

export function sendMail(config, email, code) {
  validateMail(config);
  return config.mailTransport === 'smtp'
    ? sendSmtpLoginEmail(config, email, code)
    : sendLoginEmail(config, email, code);
}
