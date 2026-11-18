//! The desktop binds its callback before opening the provider. The service returns
//! a short-lived ticket here and requires an independent native proof to redeem it.
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use url::Url;

static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn cancel() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}
fn secret() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}
fn provider_url(raw: &str) -> Result<Url, String> {
    let url=Url::parse(raw).map_err(|_| "Ugyldig Discord-adresse")?;
    if url.scheme()!="https" || url.host_str()!=Some("discord.com") || url.port().is_some()
        || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some()
        || url.path()!="/oauth2/authorize" {
        return Err("Ugyldig Discord-adresse".into());
    }
    Ok(url)
}
fn callback_target(raw:&str, port:u16, state:&str) -> Option<Result<String,String>> {
    let mut lines=raw.split("\r\n");
    let mut request=lines.next()?.split_whitespace();
    if request.next() != Some("GET") { return None; }
    let target=request.next()?;
    if !(target.starts_with("/elix/discord?") && request.next()==Some("HTTP/1.1") && request.next().is_none()) {return None;}
    let host=format!("127.0.0.1:{port}");
    let mut hosts=lines.filter_map(|line|line.split_once(':')).filter(|(name,_)|name.eq_ignore_ascii_case("Host"));
    if hosts.next().map(|(_,value)|value.trim())!=Some(host.as_str()) || hosts.next().is_some() {return None;}
    let url=Url::parse(&format!("http://{host}{target}")).ok()?;
    let fields=url.query_pairs().collect::<Vec<_>>();
    if fields.iter().filter(|(k,v)|k=="state"&&v==state).count()!=1 || fields.len()!=2 {return None;}
    if fields.iter().any(|(k,v)|k=="error"&&v=="cancelled") {return Some(Err("Discord-innloggingen ble avbrutt. Prøv igjen.".into()));}
    let ticket=&fields.iter().find(|(k,_)|k=="ticket")?.1;
    if ticket.len()!=43 || !ticket.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-'){return None;}
    Some(Ok(ticket.to_string()))
}
fn receive(stream:&mut TcpStream,port:u16,state:&str) -> Option<Result<String,String>> {
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(1))).ok()?;
    let mut request=Vec::with_capacity(1024);
    let mut buf=[0u8;512];
    let started=Instant::now();
    while request.len()<8192 && started.elapsed()<Duration::from_secs(2) {
        let n=stream.read(&mut buf).ok()?;
        if n==0 {break;}
        request.extend_from_slice(&buf[..n]);
        if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}
    }
    let (headers,_)=std::str::from_utf8(&request).ok()?.split_once("\r\n\r\n")?;
    let result=callback_target(headers,port,state);
    let (status,message)=if result.is_some(){("200 OK","Du kan lukke denne fanen og gå tilbake til ELIX.")}else{("404 Not Found","Ukjent returadresse.")};
    let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",message.len());
    result
}
fn wait(listener:TcpListener,port:u16,state:String,generation:u64) -> Result<String,String> {
    let start=Instant::now();
    while start.elapsed()<Duration::from_secs(300) {
        if GENERATION.load(Ordering::SeqCst)!=generation {return Err("Innloggingen ble avbrutt.".into());}
        match listener.accept() {
            Ok((mut stream,addr)) if addr.ip().is_loopback()=>{
                if let Some(result)=receive(&mut stream,port,&state){return result;}
            }
            Ok(_) => (),
            Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(100)),
            Err(_)=>return Err("Kunne ikke motta svar fra Discord.".into()),
        }
    }
    Err("Discord-innloggingen er utløpt. Prøv igjen.".into())
}
#[cfg(windows)]
fn open_provider(url:&Url)->Result<(),String>{
    std::process::Command::new("explorer.exe").arg(url.as_str()).spawn()
        .map(|_|()).map_err(|_|"Kunne ikke åpne systemnettleseren.".into())
}
#[cfg(not(windows))]
fn open_provider(_: &Url)->Result<(),String>{Err("Discord-innlogging krever Windows.".into())}

pub async fn begin(client:&reqwest::Client,base:&Url)->Result<(u64, Value),String>{
    let generation=GENERATION.fetch_add(1,Ordering::SeqCst).wrapping_add(1);
    let listener=TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,0)).map_err(|_|"Kunne ikke åpne lokal returadresse.")?;
    listener.set_nonblocking(true).map_err(|_|"Kunne ikke åpne lokal returadresse.")?;
    let port=listener.local_addr().map_err(|_|"Kunne ikke lese lokal returadresse.")?.port();
    let proof=secret();
    let state=secret();
    let response=client.post(base.join("v1/auth/discord/start").unwrap()).json(&json!({"port":port,"localState":state,"proof":proof}))
        .send().await.map_err(|_|"Kontotjenesten svarte ikke.")?;
    let (status,result)=super::read_response(response).await?;
    if !status.is_success(){return Err(super::error_message(&result));}
    let authorize=provider_url(result["authorize"].as_str().ok_or("Mangler Discord-adresse")?)?;
    active(generation)?;
    open_provider(&authorize)?;
    let ticket=tauri::async_runtime::spawn_blocking(move||wait(listener,port,state,generation)).await.map_err(|_|"Kunne ikke motta Discord-innlogging.")??;
    active(generation)?;
    Ok((generation,json!({"ticket":ticket,"proof":proof})))
}
pub fn active(generation:u64)->Result<(),String>{
    if GENERATION.load(Ordering::SeqCst)!=generation {return Err("Innloggingen ble avbrutt.".into());}
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn callback_requires_its_own_port_state_and_restricted_ticket(){
        let ticket="a".repeat(43);
        let request=format!("GET /elix/discord?state=my-state&ticket={ticket} HTTP/1.1\r\nHost: 127.0.0.1:45001");
        assert_eq!(callback_target(&request,45001,"my-state"),Some(Ok(ticket)));
        assert_eq!(callback_target(&request,45002,"my-state"),None);
        assert_eq!(callback_target(&request,45001,"different-state"),None);
        assert!(matches!(callback_target("GET /elix/discord?state=my-state&error=cancelled HTTP/1.1\r\nHost: 127.0.0.1:45001",45001,"my-state"),Some(Err(_))));
    }
    #[test]
    fn opens_only_discord_authorization(){
        assert!(provider_url("https://discord.com/oauth2/authorize?scope=identify").is_ok());
        assert!(provider_url("https://example.com/oauth2/authorize").is_err());
        assert!(provider_url("https://discord.com/other").is_err());
    }
}
