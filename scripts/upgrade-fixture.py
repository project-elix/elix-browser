"""Ephemeral HTTPS origin for a disposable Windows Actions runner; never a release server."""
import argparse, datetime, functools, http.server, ipaddress, json, pathlib, ssl
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID, ExtendedKeyUsageOID

p = argparse.ArgumentParser()
p.add_argument('mode', choices=['prepare', 'serve'])
p.add_argument('root', type=pathlib.Path)
args = p.parse_args()
root = args.root
root.mkdir(parents=True, exist_ok=True)
if args.mode == 'prepare':
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, 'ELIX disposable CI TLS')])
    now = datetime.datetime.now(datetime.timezone.utc)
    cert = (x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(key.public_key())
            .serial_number(x509.random_serial_number()).not_valid_before(now-datetime.timedelta(minutes=5))
            .not_valid_after(now+datetime.timedelta(days=1))
            .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
            .add_extension(x509.SubjectAlternativeName([x509.DNSName('localhost'), x509.IPAddress(ipaddress.ip_address('127.0.0.1'))]), critical=False)
            .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
            .sign(key, hashes.SHA256()))
    (root/'tls.pem').write_bytes(cert.public_bytes(serialization.Encoding.PEM))
    (root/'tls.cer').write_bytes(cert.public_bytes(serialization.Encoding.DER))
    (root/'tls.key').write_bytes(key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
    for case, artifact, signature in [('stable','new.exe','new.exe.sig'), ('corrupt','corrupt.exe','new.exe.sig'), ('replay','old.exe','old.exe.sig')]:
        data = {'version':'0.1.1', 'notes':'Disposable CI upgrade', 'pub_date':now.isoformat().replace('+00:00','Z'),
                'platforms':{'windows-x86_64':{'url':f'https://127.0.0.1:44335/{artifact}', 'signature':(root/signature).read_text().strip()}}}
        (root/f'{case}.json').write_text(json.dumps(data))
    (root/'cookie.html').write_text("""<!doctype html><title>cookie pending</title><script>
const seed = new URL(location.href).searchParams.get('seed');
const readCookie = () => document.cookie.split('; ').find(c=>c.startsWith('elix_upgrade_cookie='));
if(seed && (readCookie() || localStorage.getItem('elix_upgrade'))) {
  document.title = 'Unexpected data from another profile';
} else {
  if(seed) {
    document.cookie = 'elix_upgrade_cookie='+encodeURIComponent(seed)+'; Max-Age=3600; Path=/; Secure; SameSite=Lax';
    localStorage.setItem('elix_upgrade',seed);
  }
  const match = readCookie();
  document.title = 'cookie:'+(match ? decodeURIComponent(match.split('=')[1]) : 'missing')+':storage:'+localStorage.getItem('elix_upgrade');
}
</script>ELIX cookie persistence fixture""")
    raw = bytearray((root/'new.exe').read_bytes())
    raw[-1] ^= 1
    (root/'corrupt.exe').write_bytes(raw)
else:
    # Only the static artifact subdirectory is served; signing/TLS private keys live above it.
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(root/'public'))
    server = http.server.ThreadingHTTPServer(('127.0.0.1',44335), handler)
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(root/'tls.pem', root/'tls.key')
    server.socket = tls.wrap_socket(server.socket, server_side=True)
    server.serve_forever()
