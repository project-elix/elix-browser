// This module runs only on the account server. Never import it into the desktop UI.
const API = 'https://api.brevo.com/v3';
// Accept unquoted dot-atoms only, so mail libraries cannot reinterpret the localpart.
export const validEmail = value => typeof value === 'string' && value.length <= 254 && /^[\p{L}\p{N}\p{M}!#$%&'*+/=?^_`{|}~-]+(?:\.[\p{L}\p{N}\p{M}!#$%&'*+/=?^_`{|}~-]+)*@[^\s@<>(),;:\\"\[\]\x00-\x1f\x7f]+\.[^\s@<>(),;:\\"\[\]\x00-\x1f\x7f]+$/u.test(value);
const escape = value => value.replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));

export class BrevoError extends Error {
  constructor(reason) {
    super('Kunne ikke sende e-post akkurat nå. Prøv igjen senere.');
    this.status = 503;
    this.reason = reason;
  }
}

export function validateBrevo(config) {
  if (!config.brevoKey || /\s/.test(config.brevoKey)) throw new BrevoError('missing-api-key');
  validateSender(config);
}

export function validateSender(config) {
  if (!validEmail(config.senderEmail)) throw new BrevoError('invalid-sender');
  if (config.senderName && (typeof config.senderName !== 'string' || config.senderName.length > 100 || /[\x00-\x1f\x7f]/.test(config.senderName))) throw new BrevoError('invalid-sender-name');
}

export function loginMessage(config, email, code) {
  validateSender(config);
  if (!validEmail(email) || !/^\d{8}$/.test(code)) throw new BrevoError('invalid-message');
  const name = config.senderName || 'ELIX Browser';
  return {
    sender: {email: config.senderEmail, name},
    to: [{email}],
    subject: 'Din innloggingskode til ELIX',
    tags: ['elix-login'],
    textContent: `Din innloggingskode til ELIX Browser er ${code}.\n\nSkriv koden i ELIX-vinduet der du startet innloggingen. Koden er gyldig i 10 minutter og kan bare brukes én gang.\n\nIkke del koden med andre. Hvis du ikke ba om å logge inn, kan du se bort fra denne e-posten.`,
    htmlContent: `<!doctype html><html lang="nb"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head><body style="margin:0;background:#10131c;color:#edf2ff;font-family:Arial,sans-serif"><table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center" style="padding:32px 16px"><table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:480px;background:#1a2130;border:1px solid #35425c;border-radius:16px"><tr><td style="padding:32px"><p style="margin:0 0 24px;color:#73b8ff;font-size:16px;font-weight:bold">${escape(name)}</p><h1 style="font-size:24px;margin:0 0 16px">Logg inn på ELIX</h1><p style="line-height:1.6">Skriv denne koden i ELIX-vinduet der du startet innloggingen:</p><p style="font-size:32px;letter-spacing:5px;font-weight:bold;color:#91ccff;margin:28px 0">${code}</p><p style="line-height:1.6">Koden er gyldig i <strong>10 minutter</strong> og kan bare brukes én gang.</p><p style="color:#c1cada;font-size:13px;line-height:1.6;margin-top:28px">Ikke del koden med andre. Hvis du ikke ba om å logge inn, kan du se bort fra denne e-posten.</p></td></tr></table></td></tr></table></body></html>`,
  };
}

async function request(config, path, payload, fetchImpl) {
  let response;
  try {
    response = await fetchImpl(API + path, {
      method: payload ? 'POST' : 'GET',
      headers: {'api-key': config.brevoKey, accept: 'application/json', ...(payload ? {'content-type':'application/json'} : {})},
      body: payload ? JSON.stringify(payload) : undefined,
      redirect: 'error',
      signal: AbortSignal.timeout(15000),
    });
  } catch {
    // Do not expose provider errors, headers, recipients, codes or API keys in logs.
    throw new BrevoError('network-or-timeout');
  }
  if (!response.ok) {
    await response.body?.cancel();
    const reason = response.status === 401 ? 'api-key-rejected' : response.status === 403 ? 'access-denied' : response.status === 429 ? 'provider-rate-limit' : 'provider-rejected';
    throw new BrevoError(reason);
  }
  try { return await response.json(); } catch { throw new BrevoError('invalid-provider-response'); }
}

export async function sendLoginEmail(config, email, code, fetchImpl = fetch) {
  validateBrevo(config);
  const result = await request(config, '/smtp/email', loginMessage(config, email, code), fetchImpl);
  if (typeof result?.messageId !== 'string' || !result.messageId) throw new BrevoError('missing-message-id');
  // Acceptance by Brevo is not proof of inbox delivery. Never return the message ID to the UI.
}

export async function checkBrevo(config, {sandbox = false, fetchImpl = fetch} = {}) {
  validateBrevo(config);
  const result = await request(config, '/senders', undefined, fetchImpl);
  if (!Array.isArray(result?.senders)) throw new BrevoError('invalid-provider-response');
  const sender = result.senders.find(entry => typeof entry?.email === 'string' && entry.email.toLowerCase() === config.senderEmail.toLowerCase());
  if (!sender) throw new BrevoError('sender-not-found');
  if (sender.active !== true) throw new BrevoError('sender-not-active');
  if (sandbox) {
    // Fixed dummy code: this checks the request format, never creates a login challenge.
    const message = loginMessage(config, config.senderEmail, '00000000');
    message.headers = {'X-Sib-Sandbox': 'drop'};
    const accepted = await request(config, '/smtp/email', message, fetchImpl);
    if (typeof accepted?.messageId !== 'string' || !accepted.messageId) throw new BrevoError('missing-message-id');
  }
  return {sender: 'active', sandbox, deliveryVerified: false};
}

