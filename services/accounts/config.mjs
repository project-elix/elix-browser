import {validateMail} from './mail.mjs';

export function validateOrigin(value, development = false) {
  let url;
  try { url = new URL(value); } catch { throw new Error('PUBLIC_URL must be an HTTPS origin'); }
  if ((url.protocol !== 'https:' && !(development && url.protocol === 'http:' && url.hostname === '127.0.0.1')) ||
      url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error('PUBLIC_URL must be an HTTPS origin without path, credentials or query');
  }
  return url;
}

export function readConfig(env = process.env) {
  const development = env.NODE_ENV === 'development';
  const base = validateOrigin(env.PUBLIC_URL, development);
  const pepper = env.AUTH_PEPPER;
  if (!pepper || pepper.length < 32 || /REPLACE|EXAMPLE|CHANGE_ME/i.test(pepper)) throw new Error('Set AUTH_PEPPER to at least 32 random characters');
  const port = Number(env.PORT ?? 3210);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be between 1 and 65535');
  if (env.TRUST_PROXY && !['0','1'].includes(env.TRUST_PROXY)) throw new Error('TRUST_PROXY must be 0 or 1');
  const config = {
    publicUrl: base.origin, pepper, development, port,
    discordId: env.DISCORD_CLIENT_ID?.trim(),
    discordSecret: env.DISCORD_CLIENT_SECRET?.trim(),
    database: env.DATABASE_PATH || './data/accounts.sqlite',
    brevoKey: env.BREVO_API_KEY?.trim(),
    mailTransport: env.MAIL_TRANSPORT || 'brevo-api',
    smtpHost: env.SMTP_HOST?.trim(),
    smtpPort: Number(env.SMTP_PORT ?? 587),
    smtpUser: env.SMTP_USER,
    smtpPassword: env.SMTP_PASSWORD,
    senderEmail: env.BREVO_SENDER_EMAIL?.trim().toLowerCase(),
    senderName: env.BREVO_SENDER_NAME?.trim() || 'ELIX Browser',
    trustProxy: env.TRUST_PROXY === '1',
  };
  if (!['brevo-api','smtp'].includes(config.mailTransport)) throw new Error('MAIL_TRANSPORT must be brevo-api or smtp');
  if (!development || config.brevoKey || config.senderEmail || config.mailTransport === 'smtp') validateMail(config);
  if (!development && config.database === ':memory:') throw new Error('Production requires a persistent DATABASE_PATH');
  return config;
}

