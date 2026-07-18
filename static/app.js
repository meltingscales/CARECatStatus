// CARECatStatus frontend
// Connects via WebSocket, keeps a local map of cats, and re-renders on change.

const cats = new Map(); // id → cat

// Fixed room order, mirrors the shelter's whiteboard layout top-to-bottom.
const ROOMS = ['Rm 4', 'Rm 6', 'Rm 10', 'Rm 12', 'Soc. Room', 'Iso', 'Intake', 'Cage in Hallway'];

// ── DOM refs ──────────────────────────────────────────────────────────────────
const catList    = document.getElementById('cat-list');
const addBtn     = document.getElementById('add-btn');
const modal      = document.getElementById('cat-modal');
const form       = document.getElementById('cat-form');
const modalTitle = document.getElementById('modal-title');
const fName      = document.getElementById('f-name');
const fRoom      = document.getElementById('f-room');
const fNotes     = document.getElementById('f-notes');
const fFood      = document.getElementById('f-food');
const connDot    = document.getElementById('conn-status');
const cancelBtn  = document.getElementById('modal-cancel');
const deleteModal   = document.getElementById('delete-modal');
const deleteMsg     = document.getElementById('delete-msg');
const deleteConfirm = document.getElementById('delete-confirm');
const deleteCancel  = document.getElementById('delete-cancel');
const helpBtn    = document.getElementById('help-btn');
const helpModal  = document.getElementById('help-modal');
const helpClose  = document.getElementById('help-close');
const exportBtn  = document.getElementById('export-btn');
const importInput = document.getElementById('import-input');
const pinScreen    = document.getElementById('pin-screen');
const pinDots      = document.getElementById('pin-dots');
const pinError     = document.getElementById('pin-error');
const pinUsername  = document.getElementById('pin-username');

let editingId = null; // null = create mode

fRoom.innerHTML = '<option value="">— No room —</option>' +
  ROOMS.map(r => `<option value="${esc(r)}">${esc(r)}</option>`).join('');

// ── PIN entry ─────────────────────────────────────────────────────────────────
let pinValue = '';
const PIN_MAX = 8;

function updatePinDots() {
  pinDots.innerHTML = Array.from({ length: pinValue.length }, () =>
    '<span class="pin-dot filled"></span>'
  ).join('');
}

function showPinError() {
  pinError.classList.remove('hidden');
  pinValue = '';
  updatePinDots();
  setTimeout(() => pinError.classList.add('hidden'), 2000);
}

async function submitPin() {
  const username = pinUsername.value.trim();
  if (!username || !pinValue) return;
  try {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, pin: pinValue }),
      credentials: 'same-origin',
    });
    if (res.ok) {
      pinScreen.classList.add('hidden');
      connect();
    } else {
      showPinError();
    }
  } catch {
    showPinError();
  }
}

function pinDigit(d) {
  if (pinValue.length < PIN_MAX) {
    pinValue += d;
    updatePinDots();
  }
}

function pinBackspace() {
  pinValue = pinValue.slice(0, -1);
  updatePinDots();
}

// Click on numpad buttons.
document.querySelector('.pin-pad').addEventListener('click', (e) => {
  const key = e.target.closest('.pin-key');
  if (!key) return;
  if (key.dataset.digit !== undefined) {
    pinDigit(key.dataset.digit);
  } else if (key.dataset.action === 'clear') {
    pinBackspace();
  } else if (key.dataset.action === 'submit') {
    submitPin();
  }
});

// Keyboard support: digits, backspace, enter — only when username field is not focused.
document.addEventListener('keydown', (e) => {
  if (!pinScreen.classList.contains('hidden') && document.activeElement !== pinUsername) {
    if (e.key >= '0' && e.key <= '9') {
      pinDigit(e.key);
    } else if (e.key === 'Backspace') {
      pinBackspace();
    } else if (e.key === 'Enter') {
      submitPin();
    }
  }
});

// ── Boot: check auth status, then connect or show PIN screen ──────────────────
async function boot() {
  try {
    const res  = await fetch('/api/auth/status', { credentials: 'same-origin' });
    const data = await res.json();

    if (!data.required || data.authenticated) {
      connect();
    } else {
      pinScreen.classList.remove('hidden');
      pinUsername.focus();
    }
  } catch {
    // Server unreachable — try connecting anyway (WS will fail gracefully).
    connect();
  }
}

// ── WebSocket ─────────────────────────────────────────────────────────────────
let ws;
let reconnectDelay = 1000;

function connect() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  ws = new WebSocket(`${proto}://${location.host}/ws`);

  ws.addEventListener('open', () => {
    connDot.className = 'conn-dot connected';
    connDot.title = 'Connected';
    reconnectDelay = 1000;
  });

  ws.addEventListener('close', (ev) => {
    connDot.className = 'conn-dot disconnected';
    connDot.title = 'Disconnected — reconnecting…';

    // 4001 = custom "unauthorized" close — show PIN screen instead of reconnecting.
    if (ev.code === 4001) {
      pinScreen.classList.remove('hidden');
      return;
    }
    setTimeout(connect, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, 30000);
  });

  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    switch (msg.type) {
      case 'snapshot':
        cats.clear();
        for (const cat of msg.cats) cats.set(cat.id, cat);
        break;
      case 'upsert':
        cats.set(msg.cat.id, msg.cat);
        break;
      case 'delete':
        cats.delete(msg.id);
        break;
    }
    render();
  });
}

function send(msg) {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  }
}

// ── Render ────────────────────────────────────────────────────────────────────
// Cats are grouped into sections by room, in whiteboard order, with cats that
// have no matching room collected into an "Unassigned" section at the bottom.
function render() {
  if (cats.size === 0) {
    catList.innerHTML = '<p class="empty-msg">No cats yet. Add one!</p>';
    return;
  }

  const byRoom = new Map(ROOMS.map(r => [r, []]));
  const unassigned = [];
  for (const cat of cats.values()) {
    (byRoom.get(cat.room) ?? unassigned).push(cat);
  }

  const sections = [...ROOMS.map(r => [r, byRoom.get(r)]), ['Unassigned', unassigned]];

  catList.innerHTML = sections.map(([room, roomCats]) => {
    roomCats.sort((a, b) => a.name.localeCompare(b.name));
    const cardsHtml = roomCats.length
      ? roomCats.map(cardHtml).join('')
      : '<p class="empty-msg room-empty">No cats</p>';
    return `
      <section class="room-section" data-room="${esc(room)}">
        <h2 class="room-title">${esc(room)}</h2>
        <div class="room-cats">${cardsHtml}</div>
      </section>
    `;
  }).join('');
}

function cardHtml(cat) {
  const notesHtml = cat.notes      ? `<div class="card-field"><strong>Notes</strong>${esc(cat.notes)}</div>` : '';
  const foodHtml  = cat.food_notes ? `<div class="card-field"><strong>Food</strong>${esc(cat.food_notes)}</div>` : '';
  const locLabel  = cat.location === 'adoption center' ? 'Adoption Center' : 'Foster';
  const locClass  = cat.location === 'adoption center' ? 'loc-ac' : 'loc-foster';

  return `
    <article class="cat-card ${cat.color}" id="card-${cat.id}">
      <div class="card-header">
        <span class="cat-name">${esc(cat.name)}</span>
        <span class="chip ${cat.color}">${esc(cat.color)}</span>
        <span class="chip ${locClass}">${locLabel}</span>
        <div class="card-actions">
          <button class="btn-icon" title="Edit" data-edit="${cat.id}">✏️</button>
          <button class="btn-icon" title="Delete" data-delete="${cat.id}">🗑️</button>
        </div>
      </div>
      ${notesHtml}
      ${foodHtml}
    </article>
  `;
}

function esc(str) {
  return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

// ── Modal helpers ─────────────────────────────────────────────────────────────
function openCreate() {
  editingId = null;
  modalTitle.textContent = 'Add Cat';
  form.reset();
  modal.showModal();
  fName.focus();
}

function openEdit(id) {
  const cat = cats.get(id);
  if (!cat) return;
  editingId = id;
  modalTitle.textContent = 'Edit Cat';
  fName.value  = cat.name;
  fRoom.value  = cat.room;
  fNotes.value = cat.notes;
  fFood.value  = cat.food_notes;
  form.querySelector(`input[name="color"][value="${cat.color}"]`).checked = true;
  form.querySelector(`input[name="location"][value="${cat.location}"]`).checked = true;
  modal.showModal();
  fName.focus();
}

function closeModal() { modal.close(); }

let pendingDeleteId = null;

function openDeleteConfirm(id) {
  const cat = cats.get(id);
  if (!cat) return;
  pendingDeleteId = id;
  deleteMsg.textContent = `Are you sure you want to delete ${cat.name}?`;
  deleteModal.showModal();
}

function closeDeleteModal() {
  pendingDeleteId = null;
  deleteModal.close();
}

deleteConfirm.addEventListener('click', () => {
  if (pendingDeleteId) send({ type: 'delete', id: pendingDeleteId });
  closeDeleteModal();
});
deleteCancel.addEventListener('click', closeDeleteModal);
deleteModal.addEventListener('click', (e) => { if (e.target === deleteModal) closeDeleteModal(); });

// ── Events ────────────────────────────────────────────────────────────────────
addBtn.addEventListener('click', openCreate);
cancelBtn.addEventListener('click', closeModal);

form.addEventListener('submit', (e) => {
  e.preventDefault();
  const color    = form.querySelector('input[name="color"]:checked')?.value;
  const location = form.querySelector('input[name="location"]:checked')?.value;
  if (!color || !location) return;

  if (editingId) {
    send({
      type: 'update',
      id: editingId,
      patch: { name: fName.value, color, location, room: fRoom.value, notes: fNotes.value, food_notes: fFood.value },
    });
  } else {
    send({
      type: 'create',
      cat: { name: fName.value, color, location, room: fRoom.value, notes: fNotes.value, food_notes: fFood.value },
    });
  }
  closeModal();
});

catList.addEventListener('click', (e) => {
  const editId   = e.target.closest('[data-edit]')?.dataset.edit;
  const deleteId = e.target.closest('[data-delete]')?.dataset.delete;
  if (editId)   openEdit(editId);
  if (deleteId) openDeleteConfirm(deleteId);
});

modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });

// ── Import / export / help ───────────────────────────────────────────────────
helpBtn.addEventListener('click', () => helpModal.showModal());
helpClose.addEventListener('click', () => helpModal.close());
helpModal.addEventListener('click', (e) => { if (e.target === helpModal) helpModal.close(); });

exportBtn.addEventListener('click', () => {
  location.href = '/api/cats/export.csv';
});

importInput.addEventListener('change', async () => {
  const file = importInput.files[0];
  importInput.value = '';
  if (!file) return;
  try {
    const res = await fetch('/api/cats/import', {
      method: 'POST',
      headers: { 'Content-Type': 'text/csv' },
      body: await file.text(),
      credentials: 'same-origin',
    });
    if (!res.ok) { alert('Import failed.'); return; }
    const { created, updated, errors } = await res.json();
    let msg = `Imported: ${created} created, ${updated} updated.`;
    if (errors.length) msg += `\n\n${errors.length} error(s):\n${errors.join('\n')}`;
    alert(msg);
  } catch {
    alert('Import failed.');
  }
});

// ── PWA service worker ────────────────────────────────────────────────────────
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('/sw.js').catch(() => {});
}

boot();
