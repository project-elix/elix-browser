import {useEffect, useRef, type MouseEvent, type PointerEvent} from 'react';
import type {Entry} from '../types';

type Gesture = {
 id: number;
 group: string;
 pointer: number;
 startX: number;
 startY: number;
 source: HTMLElement;
 active: boolean;
 target: HTMLElement | null;
};

export function useBookmarkDrag(profile: string, onMove: (id: number, group: string) => void) {
 const gesture = useRef<Gesture | null>(null);
 const suppressClick = useRef(false);

 function clear() {
  const drag = gesture.current;
  if (!drag) return;
  gesture.current = null;
  drag.target?.removeAttribute('data-bookmark-hover');
  drag.source.removeAttribute('data-bookmark-moving');
  if (drag.active) {
   suppressClick.current = true;
   delete document.documentElement.dataset.bookmarkDragging;
  }
  if (drag.source.hasPointerCapture(drag.pointer)) drag.source.releasePointerCapture(drag.pointer);
 }

 useEffect(() => {
  const escape = (event: KeyboardEvent) => {
   if (event.key === 'Escape' && gesture.current?.active) {
    event.preventDefault();
    clear();
   }
  };
  window.addEventListener('keydown', escape, true);
  window.addEventListener('blur', clear);
  return () => {
   window.removeEventListener('keydown', escape, true);
   window.removeEventListener('blur', clear);
   clear();
  };
 }, [profile]);

 function targetAt(event: PointerEvent, drag: Gesture) {
  const hit = document.elementFromPoint(event.clientX, event.clientY);
  const target = hit?.closest<HTMLElement>('[data-bookmark-group]');
  if (!target || target.dataset.bookmarkGroup === drag.group ||
      target.closest<HTMLElement>('[data-bookmark-profile]')?.dataset.bookmarkProfile !== profile) return null;
  return target;
 }

 function start(event: PointerEvent<HTMLElement>, item: Entry) {
  if (!event.isPrimary || event.button !== 0) return;
  clear();
  suppressClick.current = false;
  const source = event.currentTarget;
  source.setPointerCapture(event.pointerId);
  gesture.current = {id: item.id, group: item.group || '', pointer: event.pointerId,
   startX: event.clientX, startY: event.clientY, source, active: false, target: null};
 }

 function move(event: PointerEvent<HTMLElement>) {
  const drag = gesture.current;
  if (!drag || drag.pointer !== event.pointerId) return;
  if (!(event.buttons & 1)) {clear(); return;}
  if (!drag.active) {
   if (Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) <= 7) return;
   drag.active = true;
   document.documentElement.dataset.bookmarkDragging = 'true';
   drag.source.setAttribute('data-bookmark-moving', '');
  }
  event.preventDefault();
  const next = targetAt(event, drag);
  if (next !== drag.target) {
   drag.target?.removeAttribute('data-bookmark-hover');
   next?.setAttribute('data-bookmark-hover', '');
   drag.target = next;
  }
 }

 function finish(event: PointerEvent<HTMLElement>) {
  const drag = gesture.current;
  if (!drag || drag.pointer !== event.pointerId) return;
  const group = drag.active ? targetAt(event, drag)?.dataset.bookmarkGroup : undefined;
  clear();
  if (group !== undefined) onMove(drag.id, group);
 }

 function click(event: MouseEvent<HTMLElement>) {
  if (!suppressClick.current || event.detail === 0) return;
  event.preventDefault();
  event.stopPropagation();
  suppressClick.current = false;
 }
 return {start, move, finish, clear, click};
}
