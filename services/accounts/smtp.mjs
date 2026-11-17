// Server-only transport. SMTP credentials never reach the desktop or provider logs.
import {domainToASCII} from 'node:url';
import {BrevoError, loginMessage, validateSender} from './brevo.mjs';

function canonicalRecipient(value) {
  if (typeof value !== 'string') return null;
  const at = value.lastIndexOf('@');
  if (at < 1) return null;
  const domain = domainToASCII(value.slice(at + 1));
  return domain ? value.slice(0, at) + '@' + domain.toLowerCase() : null;
}

export function validateSmtp(config) {
  validateSender(config);
  if (typeof config.smtpHost !== 'string' || config.smtpHost.length > 253 ||
      !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(config.smtpHost)) throw new BrevoError('invalid-smtp-host');
  if (!Number.isInteger(config.smtpPort) || config.smtpPort < 1 || config.smtpPort > 65535) throw new BrevoError('invalid-smtp-port');
  for (const key of ['smtpUser', 'smtpPassword']) {
    if (typeof config[key] !== 'string' || !config[key] || config[key].length > 4096 || /[\r\n\0]/.test(config[key])) throw new BrevoError('missing-smtp-credentials');
  }
}

function options(config) {
  return {
    host: config.smtpHost, port: config.smtpPort,
    secure: config.smtpPort === 465, requireTLS: true, ignoreTLS: false,
    opportunisticTLS: false, tls: {rejectUnauthorized: true, minVersion: 'TLSv1.2', servername: config.smtpHost},
    auth: {user: config.smtpUser, pass: config.smtpPassword}, forceAuth: true,
    pool: false, logger: false, debug: false, transactionLog: false,
    disableFileAccess: true, disableUrlAccess: true, maxRecipients: 1,
    connectionTimeout: 10000, greetingTimeout: 10000, socketTimeout: 15000, dnsTimeout: 10000,
  };
}

async function run(config, action, createTransport) {
  validateSmtp(config);
  let transport;
  try {
    createTransport ??= (await import('nodemailer')).default.createTransport;
    transport = createTransport(options(config));
    return await action(transport);
  } catch (error) {
    if (error instanceof BrevoError) throw error;
    // SMTP errors may include recipients, protocol responses and credentials.
    throw new BrevoError(error?.code === 'EAUTH' ? 'smtp-auth-rejected' : error?.code === 'EABORT' ? 'smtp-timeout' : 'smtp-unavailable');
  } finally {
    // Never let cleanup expose an unsanitized provider exception.
    try { transport?.close(); } catch {}
  }
}

export async function sendSmtpLoginEmail(config, email, code, createTransport) {
  const message = loginMessage(config, email, code);
  const recipient = canonicalRecipient(email);
  if (!recipient) throw new BrevoError('invalid-message');
  await run(config, async transport => {
    const result = await transport.sendMail({
      from: {name: message.sender.name, address: message.sender.email},
      to: [{address: email}], envelope: {from: {address: config.senderEmail}, to: [{address: email}]},
      subject: message.subject, text: message.textContent, html: message.htmlContent,
      // Nodemailer aborts the active connection; a Promise.race alone cannot do so.
      signal: AbortSignal.timeout(18000),
    });
    if (!Array.isArray(result?.accepted) || result.accepted.length !== 1 ||
        canonicalRecipient(result.accepted[0]) !== recipient || result.rejected?.length) {
      throw new BrevoError('smtp-recipient-rejected');
    }
    // Provider acceptance is not proof of inbox delivery. Never retry here.
  }, createTransport);
}

export async function checkSmtp(config, {createTransport} = {}) {
  await run(config, transport => transport.verify(), createTransport);
  return {transport: 'smtp', authenticated: true, senderVerified: false, deliveryVerified: false};
}
