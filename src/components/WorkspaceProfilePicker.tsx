import { useState } from 'react';
import { desktop, rpc } from '../browser/bridge';
import type { Profile } from '../types';

type Props = {
  activeProfile?: Profile;
  onRefresh: () => Promise<unknown>;
  onError: (error: unknown) => void;
};

export default function WorkspaceProfilePicker({ activeProfile, onRefresh, onError }: Props) {
  const [switching, setSwitching] = useState(false);

  async function switchTo(preset: string) {
    setSwitching(true);
    try {
      const profile = await rpc<Profile>('workspace_profile', { preset });
      if (profile.id !== activeProfile?.id) {
        await rpc('switch_profile', { id: profile.id });
        await onRefresh();
      }
    } catch (error) {
      onError(error);
    } finally {
      setSwitching(false);
    }
  }

  return <label className="rail-field">Arbeidsområdeprofil
    <select aria-label="Arbeidsområdeprofil" value={activeProfile?.workspace ?? ''}
      disabled={switching || !desktop} onChange={event => void switchTo(event.target.value)}>
      <option value="" disabled>{activeProfile?.workspace ? 'Velg en profil' : activeProfile?.name || 'Velg en profil'}</option>
      <option value="work">Jobb</option><option value="personal">Privat</option><option value="gaming">Gaming</option>
    </select>
  </label>;
}
