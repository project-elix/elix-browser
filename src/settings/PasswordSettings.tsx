import { useState } from 'react';
import { KeyRound, Trash2 } from 'lucide-react';
import { desktop, rpc } from '../browser/bridge';
import type { Profile, Settings } from '../types';

type Props = {
  profile?: Profile;
  settings: Settings;
  onSave: (settings: Settings) => Promise<void>;
  onError: (error: unknown) => void;
};

export default function PasswordSettings({ profile, settings, onSave, onError }: Props) {
  const [saving, setSaving] = useState(false);
  const [deletion, setDeletion] = useState<'idle' | 'confirm' | 'clearing' | 'cleared'>('idle');

  async function change(patch: Partial<Settings>) {
    setSaving(true);
    try {
      await onSave({ ...settings, ...patch });
    } catch (error) {
      onError(error);
    } finally {
      setSaving(false);
    }
  }

  async function clear() {
    if (!profile || deletion !== 'confirm') return;
    setDeletion('clearing');
    try {
      await rpc('clear_passwords', { profile: profile.id, consent: true });
      setDeletion('cleared');
    } catch (error) {
      setDeletion('idle');
      onError(error);
    }
  }

  return <div className="settings-block">
    <h2><KeyRound size={22}/>Passord og autofyll</h2>
    <p>Disse to innstillingene gjelder for alle vanlige profiler på denne installasjonen. Private faner lagrer aldri passord eller skjemadata for autofyll.</p>
    <label className="setting-row">
      <span><strong>Tilby å lagre passord</strong><small style={{display:'block'}}>Spør om nye og endrede passord når du logger inn på et nettsted.</small></span>
      <input type="checkbox" checked={settings.password_autosave} disabled={saving || !desktop} onChange={event => void change({password_autosave:event.target.checked})}/>
    </label>
    <label className="setting-row">
      <span><strong>Autofyll navn og adresser</strong><small style={{display:'block'}}>Husker skjemafelter for hver profil.</small></span>
      <input type="checkbox" checked={settings.autofill} disabled={saving || !desktop} onChange={event => void change({autofill:event.target.checked})}/>
    </label>

    <h3>{profile ? <>Lagrede passord i «{profile.name}»</> : 'Lagrede passord'}</h3>
    {profile ? <>
      <p>Velg et lagret brukernavn i innloggingsfeltet på nettstedet for å fylle ut passordet. Lagrede passord kan fortsatt fylles ut når du slår av passordlagring.</p>
      <p className="muted">WebView2 oppbevarer passordene lokalt, adskilt for hver profil. ELIX kan slette dem samlet. Visning og redigering av enkeltpassord er utilgjengelig.</p>
      {deletion === 'confirm' || deletion === 'clearing' ?
        <div className="notice" role="alert">
          <p>Vil du slette alle lagrede passord i «{profile.name}»? Handlingen kan ikke angres.</p>
          <div className="inline-actions">
            <button disabled={deletion === 'clearing'} onClick={() => void clear()}>{deletion === 'clearing' ? 'Sletter …' : 'Ja, slett passordene'}</button>
            <button disabled={deletion === 'clearing'} onClick={() => setDeletion('idle')}>Avbryt</button>
          </div>
        </div> :
        <button disabled={saving || !desktop} onClick={() => setDeletion('confirm')}><Trash2 size={16}/>Slett alle lagrede passord i denne profilen</button>}
      {deletion === 'cleared' && <p role="status">Lagrede passord er slettet i «{profile.name}».</p>}
    </> : <p>Åpne en profil i ELIX for Windows for å administrere lagrede passord.</p>}
  </div>;
}
