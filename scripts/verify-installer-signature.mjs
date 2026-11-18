import assert from 'node:assert/strict';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync } from 'node:fs';

const [installer, version] = process.argv.slice(2);
assert(installer && version, 'Expected installer path and release version');
function unpack(file) {
  const envelope = readFileSync(file, 'utf8').trim();
  return Buffer.from(envelope, 'base64').toString('utf8').trimEnd().split(/\r?\n/);
}

const keyLines = unpack('docs/update-public-key.txt');
assert.equal(keyLines.length, 2);
const keyPacket = Buffer.from(keyLines[1], 'base64');
assert.equal(keyPacket.length, 42);
assert.equal(keyPacket.subarray(0, 2).toString(), 'Ed');
const key = createPublicKey({
  format: 'jwk',
  key: { kty: 'OKP', crv: 'Ed25519', x: keyPacket.subarray(10).toString('base64url') },
});

const lines = unpack(installer + '.sig');
assert.equal(lines.length, 4);
const packet = Buffer.from(lines[1], 'base64');
assert.equal(packet.length, 74);
assert.equal(packet.subarray(0, 2).toString(), 'ED');
assert(packet.subarray(2, 10).equals(keyPacket.subarray(2, 10)), 'Unexpected updater signing key');
const prefix = 'trusted comment: ';
assert(lines[2].startsWith(prefix), 'Missing signed comment');
const comment = lines[2].slice(prefix.length);
const signature = packet.subarray(10);
const signedComment = Buffer.concat([signature, Buffer.from(comment)]);
assert(verify(null, signedComment, key, Buffer.from(lines[3], 'base64')), 'Invalid signed comment');
assert.deepEqual([...comment.matchAll(/(?:^|\s)version:(\S+)/g)].map(match => match[1]), [version]);
const digest = createHash('blake2b512').update(readFileSync(installer)).digest();
assert(verify(null, digest, key, signature), 'Invalid installer signature');
console.log(`Verified installer bytes and signed version ${version} with the ELIX updater key.`);
