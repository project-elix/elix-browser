import {randomBytes,createHash} from 'node:crypto';

const random=()=>randomBytes(32).toString('base64url');
const digest=value=>createHash('sha256').update(value).digest('hex');
const opaque=value=>typeof value==='string'&&/^[A-Za-z0-9_-]{43,128}$/.test(value);
const fail=(status,message)=>Object.assign(new Error(message),{status});

async function discordIdentity(config,redirect,code){
  const form=new URLSearchParams({grant_type:'authorization_code',code,redirect_uri:redirect});
  const basic=Buffer.from(config.discordId+':'+config.discordSecret).toString('base64');
  const token=await fetch('https://discord.com/api/v10/oauth2/token',{method:'POST',redirect:'error',
    headers:{authorization:'Basic '+basic,'content-type':'application/x-www-form-urlencoded'},body:form,signal:AbortSignal.timeout(10000)});
  if(!token.ok)throw fail(502,'Discord avviste innloggingen. Prøv igjen.');
  const credentials=await token.json();
  if(credentials.token_type?.toLowerCase()!=='bearer'||typeof credentials.access_token!=='string')throw fail(502,'Ugyldig svar fra Discord');
  const response=await fetch('https://discord.com/api/v10/users/@me',{redirect:'error',
    headers:{authorization:'Bearer '+credentials.access_token},signal:AbortSignal.timeout(10000)});
  if(!response.ok)throw fail(502,'Kunne ikke hente Discord-brukeren');
  const identity=await response.json();
  return {id:identity.id,name:identity.global_name||identity.username};
}

export function discordRoutes({config,base,q,body,json,limit,seconds,user,issue,identity=discordIdentity}){
  const enabled=Boolean(config.discordId&&config.discordSecret&&/^\d{17,20}$/.test(config.discordId));
  const redirect=new URL('/v1/auth/discord/callback',base).href;
  const error=()=>fail(401,'Discord-innloggingen er ugyldig eller utløpt. Prøv igjen.');
  return {enabled,async handle(req,res,url,ip){
    if(!enabled)throw fail(503,'Discord-innlogging er ikke konfigurert.');
    const path=url.pathname;
    if(path==='/v1/auth/discord/start'&&req.method==='POST'){
      limit('discord-start:'+ip,30,600);
      const {port,proof,localState}=await body(req);
      if(!Number.isInteger(port)||port<1024||port>65535||!opaque(proof)||!opaque(localState))throw fail(400,'Ugyldig lokal innlogging.');
      const state=random();
      q('INSERT INTO challenges(id,provider,subject,state,binding,expires) VALUES(?,?,?,?,?,?)')
        .run(digest(state),'discord',JSON.stringify({port,localState}),digest(state),digest(proof),seconds()+300);
      const authorize=new URL('https://discord.com/oauth2/authorize');
      authorize.search=new URLSearchParams({client_id:config.discordId,response_type:'code',scope:'identify',redirect_uri:redirect,state});
      return json(res,200,{authorize:authorize.href});
    }
    if(path==='/v1/auth/discord/callback'&&req.method==='GET'){
      const state=url.searchParams.get('state'),code=url.searchParams.get('code');
      if(!opaque(state))throw error();
      const row=q("UPDATE challenges SET status='exchanging' WHERE id=? AND provider='discord' AND status='pending' AND expires>? RETURNING *").get(digest(state),seconds());
      if(!row)throw error();
      const {port,localState}=JSON.parse(row.subject);
      const callback=new URL('http://127.0.0.1:'+port+'/elix/discord');
      callback.searchParams.set('state',localState);
      try{
        if(url.searchParams.has('error')||!code||code.length>1024)throw error();
        const person=await identity(config,redirect,code);
        if(typeof person?.id!=='string'||!/^\d{17,20}$/.test(person.id)||typeof person.name!=='string')throw error();
        const ticket=random();
        const changed=q("UPDATE challenges SET status='ready',code=?,user_id=?,expires=? WHERE id=? AND expires>?").run(digest(ticket),JSON.stringify(person),seconds()+45,row.id,seconds()).changes;
        if(!changed)throw error();
        callback.searchParams.set('ticket',ticket);
      }catch{
        q('DELETE FROM challenges WHERE id=?').run(row.id);
        callback.searchParams.set('error','cancelled');
      }
      res.writeHead(303,{Location:callback.href,'Cache-Control':'no-store','Referrer-Policy':'no-referrer','Content-Security-Policy':"default-src 'none'"});return res.end();
    }
    if(path==='/v1/auth/discord/complete'&&req.method==='POST'){
      limit('discord-complete:'+ip,60,600);
      const {ticket,proof}=await body(req);
      if(!opaque(ticket)||!opaque(proof))throw error();
      const row=q("DELETE FROM challenges WHERE provider='discord' AND status='ready' AND code=? AND binding=? AND expires>? RETURNING user_id")
        .get(digest(ticket),digest(proof),seconds());
      if(!row)throw error();
      const person=JSON.parse(row.user_id);
      return json(res,200,issue(user('discord',person.id,person.name)));
    }
    throw fail(404,'Fant ikke endepunktet');
  }};
}
