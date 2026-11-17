import {describe,it,expect} from 'vitest';
import {readPendingLogin,startPendingLogin} from '../src/profiles/login';

describe('Pending email login',()=>{
 const now=1_800_000_000_000,challenge='a'.repeat(43);
 it('keeps the address and original deadline when account settings are reopened',()=>{
  const pending=startPendingLogin(challenge,'  Person@EXAMPLE.com  ',600,now);
  expect(readPendingLogin(JSON.stringify(pending),now+120_000)).toEqual({...pending,email:'person@example.com',expiresAt:now+600_000});
  expect(readPendingLogin(JSON.stringify(pending),now+600_000)).toBeNull();
 });
 it('rejects expired, malformed and legacy pending state without reusing its code',()=>{
  for(const raw of ['invalid','null','{}',JSON.stringify({kind:'email',challenge}),JSON.stringify({kind:'email',challenge,email:'person@example.com',expiresAt:now-1}),JSON.stringify({kind:'email',challenge,email:'person@example.com',expiresAt:now+3_600_000})])expect(readPendingLogin(raw,now)).toBeNull();
 });
 it('limits challenge lifetimes to ten minutes and rejects invalid server challenges',()=>{
  expect(startPendingLogin(challenge,'person@example.com',3600,now).expiresAt).toBe(now+600_000);
  expect(()=>startPendingLogin('invalid','person@example.com',600,now)).toThrow();
  expect(()=>startPendingLogin(challenge,'person@example.com',0,now)).toThrow();
 });
});
