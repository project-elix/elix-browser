import {useEffect, useState, type KeyboardEvent} from 'react';
import {X} from 'lucide-react';
import {rpc, internalTitle} from '../browser/bridge';
import type {Session, Tab} from '../types';
import TabIcon from './TabIcon';
import './tab-search.css';

function title(tab: Tab) {
  return tab.url.startsWith('elix:') ? internalTitle(tab.url) : tab.title || tab.url;
}

function Preview({tab}: {tab: Tab}) {
  const [image, setImage] = useState<string | null>(null);
  useEffect(() => {
    if (tab.private || tab.suspended || tab.loading || !/^https?:\/\//.test(tab.url)) return;
    let current = true;
    const timer = window.setTimeout(() => {
      void rpc<string | null>('tab_preview', {id: tab.id, url: tab.url})
        .then(value => {if (current) setImage(value);}).catch(() => {});
    }, 160);
    return () => { current = false; window.clearTimeout(timer); };
  }, [tab.id, tab.url, tab.private, tab.loading, tab.suspended]);
  return <div className="tab-search-preview" aria-label="Forhåndsvisning">
    {image ? <img src={image} alt={`Forhåndsvisning av ${title(tab)}`}/>
      : <><TabIcon tab={tab}/><strong>{title(tab)}</strong><small>{tab.private ? 'Privat fane' : tab.workspace || 'Personlig'}</small></>}
  </div>;
}

export default function TabSearch({session,onClose,onError}: {
  session: Session; onClose: () => void; onError: (error: unknown) => void;
}) {
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(session.active);
  const term = query.trim().toLocaleLowerCase('nb');
  const matches = session.tabs.filter(tab => [title(tab), tab.url, tab.group || '', tab.workspace || 'Personlig']
    .some(value => value.toLocaleLowerCase('nb').includes(term)));
  const selected = matches.find(tab => tab.id === highlight) || matches[0];
  const activate = (id: string) => { void rpc('activate', {id}).then(onClose).catch(onError); };
  const navigate = (event: KeyboardEvent<HTMLInputElement>) => {
    if (!selected || event.nativeEvent.isComposing) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      activate(selected.id);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = matches.indexOf(selected) + (event.key === 'ArrowDown' ? 1 : -1);
      const id = matches[(next + matches.length) % matches.length].id;
      setHighlight(id);
      document.getElementById(`tab-result-${id}`)?.scrollIntoView({block:'nearest'});
    }
  };
  return <div className="tab-search" id="sidebar-tab-search">
    <div><input autoFocus aria-label="Søk blant alle åpne faner" placeholder="Alle arbeidsområder …" value={query}
      onChange={e => setQuery(e.target.value)} onKeyDown={navigate}/>
      <button aria-label="Lukk fanesøk" onClick={onClose}><X size={16}/></button></div>
    {selected && <Preview key={JSON.stringify([selected.id, selected.url, selected.private, selected.suspended, selected.loading])} tab={selected}/>}
    {matches.length ? <ul>{matches.map(tab => <li key={tab.id}><button id={`tab-result-${tab.id}`}
      className={tab.id === selected?.id ? 'tab-result-selected' : ''} title={tab.private?'Privat fane':tab.url}
      onMouseEnter={() => setHighlight(tab.id)} onFocus={() => setHighlight(tab.id)} onClick={() => activate(tab.id)}>
      <TabIcon tab={tab}/><span>{title(tab)}<small>{tab.workspace || 'Personlig'}{tab.group && ` / ${tab.group}`} · {tab.private?'Privat':tab.url}</small></span>
    </button></li>)}</ul> : <p role="status">Ingen åpne faner passer til søket.</p>}
  </div>;
}
