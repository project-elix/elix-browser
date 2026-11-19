import { it, expect } from 'vitest';
import { createHash, generateKeyPairSync, randomBytes, sign } from 'node:crypto';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const verifier = fileURLToPath(new URL('../scripts/verify-installer-signature.mjs', import.meta.url));
const envelope = lines => Buffer.from(lines.join('\n')).toString('base64');

it.each(['0.1.4', '0.1.5-dev.10', '0.2.1-beta.2'])('checks installer bytes and exact signed version for %s', version => {
  const dir = mkdtempSync(join(tmpdir(), 'elix-signature-'));
  try {
    const { publicKey, privateKey } = generateKeyPairSync('ed25519');
    const keyId = randomBytes(8);
    const publicBytes = Buffer.from(publicKey.export({format:'jwk'}).x, 'base64url');
    mkdirSync(join(dir, 'docs'));
    const keyPacket = Buffer.concat([Buffer.from('Ed'), keyId, publicBytes]);
    writeFileSync(join(dir, 'docs', 'update-public-key.txt'), envelope(['untrusted comment: test key', keyPacket.toString('base64')]));

    const file = join(dir, 'candidate.exe');
    const bytes = Buffer.from('Synthetic installer');
    writeFileSync(file, bytes);
    const signature = sign(null, createHash('blake2b512').update(bytes).digest(), privateKey);
    const packet = Buffer.concat([Buffer.from('ED'), keyId, signature]);
    const comment = 'timestamp:1 file:candidate.exe version:' + version;
    const commentSignature = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
    writeFileSync(file + '.sig', envelope(['untrusted comment: test signature', packet.toString('base64'),
      'trusted comment: ' + comment, commentSignature.toString('base64')]));

    const check = expected => spawnSync(process.execPath, [verifier, file, expected], {cwd:dir}).status;
    expect(check(version)).toBe(0);
    expect(check('9.9.9')).not.toBe(0);
    writeFileSync(file, Buffer.from('Changed installer'));
    expect(check(version)).not.toBe(0);
  } finally {
    rmSync(dir, {recursive:true, force:true});
  }
});
