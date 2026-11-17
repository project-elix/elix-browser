export interface PendingEmailLogin {
  kind:'email';
  challenge:string;
  email:string;
  expiresAt:number;
}

/** Pending codes belong to one profile and one short-lived login attempt. */
export function readPendingLogin(raw:string|null,now=Date.now()):PendingEmailLogin|null {
  try {
    const value=JSON.parse(raw??'null');
    if(value?.kind!=='email'||typeof value.challenge!=='string'||
      !/^[A-Za-z0-9_-]{43}$/.test(value.challenge)||typeof value.email!=='string'||
      !/^[^\s@]+@[^\s@]+$/.test(value.email)||!Number.isSafeInteger(value.expiresAt)||
      value.expiresAt<=now||value.expiresAt>now+600_000)return null;
    return {kind:'email',challenge:value.challenge,email:value.email,expiresAt:value.expiresAt};
  }catch{return null;}
}

export function startPendingLogin(challenge:string,email:string,expiresIn:number,now=Date.now()):PendingEmailLogin {
  const value=readPendingLogin(JSON.stringify({kind:'email',challenge,email:email.trim().toLowerCase(),expiresAt:now+Math.min(expiresIn,600)*1000}),now);
  if(!value)throw new Error('Kontotjenesten returnerte en ugyldig innloggingskode. Prøv igjen.');
  return value;
}

export const expiredSessionMessage='Kontosesjonen er utløpt. Logg inn på nytt.';
