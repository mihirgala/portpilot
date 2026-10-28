import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './style.css';
const app = document.querySelector('#app');
app.innerHTML = `<main><header><div class="brand"><span class="logo">◈</span><div><h1>PortPilot</h1><p>Development servers</p></div></div><button id="refresh" class="refresh" aria-label="Refresh servers" title="Refresh">↻</button></header><div class="intro"><div><h2>Your servers</h2><p>Recognized web servers listening on this device.</p></div><span id="count"></span></div><label class="search"><span aria-hidden="true">⌕</span><input id="search" placeholder="Search name, port, or PID" aria-label="Search servers"></label><div id="notice" role="status" aria-live="polite"></div><div id="list" class="list" aria-label="Listening development servers"></div><footer><span><i></i> Refreshes automatically</span><span>Right-click tray icon to quit</span></footer></main><dialog id="confirm" aria-labelledby="dialog-title" aria-describedby="confirm-text warning"><form method="dialog"><div class="dialog-icon" aria-hidden="true">!</div><h2 id="dialog-title">Stop this server?</h2><p id="confirm-text"></p><p id="warning" class="warning">The process and its child processes will be forcibly stopped. Unsaved work may be lost.</p><div class="actions"><button value="cancel" class="secondary" autofocus>Cancel</button><button value="stop" class="danger">Stop server</button></div></form></dialog>`;
const list = document.querySelector('#list'), notice = document.querySelector('#notice'), search = document.querySelector('#search'), count = document.querySelector('#count'), refresh = document.querySelector('#refresh'), dialog = document.querySelector('#confirm');
let servers = [], busy = false, selected = null, alreadyRunning = false;
const escape = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function render() {
 const query = search.value.toLowerCase().trim();
 const visible = servers.filter(s => `${s.name} ${s.pid} ${s.addresses.join(' ')}`.toLowerCase().includes(query));
 count.textContent = `${visible.length} running`;
 list.innerHTML = visible.length ? visible.map(s => `<article class="row"><div class="server"><span class="process-icon" aria-hidden="true">${escape(s.name.slice(0,1))}</span><div><strong>${escape(s.name)}</strong><small>PID ${s.pid}</small></div><button class="stop" data-pid="${s.pid}" aria-label="Stop ${escape(s.name)} PID ${s.pid}">Stop</button></div><div class="addresses">${s.addresses.map(a => `<span>${escape(a)}</span>`).join('')}</div></article>`).join('') : `<div class="empty">${query ? 'No matching servers.' : 'No recognized web servers running.'}</div>`;
 list.querySelectorAll('.stop').forEach(b => b.addEventListener('click', () => { selected = servers.find(s => s.pid === Number(b.dataset.pid)); document.querySelector('#confirm-text').textContent = `Stop ${selected.name} (PID ${selected.pid})? Listening on ${selected.addresses.join(', ')}.${selected.launcher ? ` The CodeIgniter launcher (PID ${selected.launcher.pid}) will also be stopped to prevent a restart.` : ''}`; dialog.showModal(); }));
}
async function load() { if (busy) return; busy = true; refresh.disabled = true; try { servers = await invoke('list_listeners'); notice.textContent = alreadyRunning ? 'PortPilot is already open. Use this window or the tray icon.' : ''; render(); } catch (e) { notice.textContent = `Could not refresh: ${e}`; } finally { busy = false; refresh.disabled = false; } }
refresh.addEventListener('click', load); search.addEventListener('input', render);
dialog.addEventListener('close', async () => { if (dialog.returnValue !== 'stop' || !selected) return; const target = selected; selected = null; notice.textContent = `Stopping ${target.name}…`; try { await invoke('stop_listener', { pid: target.pid, started: target.started, launcher: target.launcher }); await load(); notice.textContent = `Stopped ${target.name} (PID ${target.pid}).`; } catch(e) { notice.textContent = `Could not stop ${target.name}: ${e}`; } });
listen('already-running', () => { alreadyRunning = true; notice.textContent = 'PortPilot is already open. Use this window or the tray icon.'; });
document.addEventListener('visibilitychange', () => { if (!document.hidden) load(); });
load(); setInterval(() => { if (!dialog.open && !document.hidden) load(); }, 10000);


