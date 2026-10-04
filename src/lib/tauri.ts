/**
 * The only place the frontend touches Tauri.
 *
 * Two channels exist between the UI and the backend, and this module is the
 * single definition of both:
 *
 * - **commands** — request/response, for layout edits and cadence changes.
 *   Every layout-mutating command returns the *new* layout so the caller
 *   replaces its state wholesale instead of trying to mirror the mutation.
 * - **one event** — `hamon://snapshot`, pushed by the sampler thread once per
 *   tick. There is no polling anywhere in the UI.
 *
 * Outside a Tauri window (`npm run dev` in a plain browser) the same surface is
 * served by [`mock.ts`], so the frontend can be developed and inspected without
 * a Rust rebuild. The choice happens here, once, so no component ever has to ask
 * which environment it is in.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { MockBackend } from './mock';
import type { Bootstrap, Layout, RuntimeStatus, Snapshot } from './types';

/** Must match `commands::SNAPSHOT_EVENT`. */
export const SNAPSHOT_EVENT = 'hamon://snapshot';

/**
 * Whether this is running inside a Tauri webview.
 *
 * `__TAURI_INTERNALS__` is injected by `withGlobalTauri`, which is not enabled
 * in this app, so its presence is what distinguishes the two environments. The
 * check is guarded because `window` does not exist under SSR and this module is
 * imported at module scope.
 */
export const inTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const mock = inTauri ? null : new MockBackend();

/** Everything the UI needs at startup, in one round trip. */
export function bootstrap(): Promise<Bootstrap> {
  return mock ? mock.bootstrap() : invoke<Bootstrap>('bootstrap');
}

export function getLayout(): Promise<Layout> {
  return mock ? mock.getLayout() : invoke<Layout>('get_layout');
}

export function setLayout(layout: Layout): Promise<Layout> {
  return mock ? mock.setLayout(layout) : invoke<Layout>('set_layout', { layout });
}

export function addWidget(kind: string): Promise<Layout> {
  return mock ? mock.addWidget(kind) : invoke<Layout>('add_widget', { kind });
}

export function removeWidget(index: number): Promise<Layout> {
  return mock ? mock.removeWidget(index) : invoke<Layout>('remove_widget', { index });
}

export function moveWidget(from: number, to: number): Promise<Layout> {
  return mock ? mock.moveWidget(from, to) : invoke<Layout>('move_widget', { from, to });
}

export function configureWidget(index: number, config: unknown): Promise<Layout> {
  return mock
    ? mock.configureWidget(index, config)
    : invoke<Layout>('configure_widget', { index, config });
}

export function setSampleInterval(intervalMs: number): Promise<Layout> {
  return mock
    ? mock.setSampleInterval(intervalMs)
    : invoke<Layout>('set_sample_interval', { intervalMs });
}

export function resetLayout(): Promise<Layout> {
  return mock ? mock.resetLayout() : invoke<Layout>('reset_layout');
}

export function getStatus(): Promise<RuntimeStatus> {
  return mock ? mock.getStatus() : invoke<RuntimeStatus>('get_status');
}

/** Asks for a tick immediately, so a refocusing window is not stale. */
export function refresh(): Promise<void> {
  return mock ? mock.refresh() : invoke<void>('refresh');
}

export function pauseSampling(): Promise<void> {
  return mock ? mock.pauseSampling() : invoke<void>('pause_sampling');
}

export function resumeSampling(): Promise<void> {
  return mock ? mock.resumeSampling() : invoke<void>('resume_sampling');
}

/**
 * Subscribes to sampler output.
 *
 * `onSnapshot` is called on the main thread for every tick; it is expected to
 * assign reactive state and nothing else. Malformed payloads are dropped with a
 * console warning rather than allowed to throw inside the event handler, where
 * the error would otherwise vanish silently.
 */
export async function onSnapshot(handler: (snapshot: Snapshot) => void): Promise<UnlistenFn> {
  const deliver = (payload: Snapshot) => {
    if (!payload || typeof payload.seq !== 'number') {
      console.warn('ignoring malformed snapshot', payload);
      return;
    }
    handler(payload);
  };

  if (mock) return mock.onSnapshot(deliver);
  return listen<Snapshot>(SNAPSHOT_EVENT, (event) => deliver(event.payload));
}
