// CARECatStatus frontend
// Connects via WebSocket, keeps a local map of cats, and re-renders on change.

const cats = new Map(); // id → cat

// ── DOM refs ──────────────────────────────────────────────────────────────────
const catList    = document.getElementById('cat-list');
const addBtn     = document.getElementById('add-btn');
const modal      = document.getElementById('cat-modal');
const form       = document.getElementById('cat-form');
const modalTitle = document.getElementById('modal-title');
const fName      = document.getElementById('f-name');
const fNotes     = document.getElementById('f-notes');
const fFood      = document.getElementById('f-food');
const connDot    = document.getElementById('conn-status');
const cancelBtn  = document.getElementById('modal-cancel');
const pinScreen    = document.getElementById('pin-screen');
const pinDots      = document.getElementById('pin-dots');
const pinError     = document.getElementById('pin-error');
const pinUsername  = document.getElementById('pin-username');

let editingId = null; // null = create mode

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
        render();
        break;
      case 'upsert':
        cats.set(msg.cat.id, msg.cat);
        renderCard(msg.cat);
        break;
      case 'delete':
        cats.delete(msg.id);
        document.getElementById(`card-${msg.id}`)?.remove();
        if (cats.size === 0) renderEmpty();
        break;
    }
  });
}

function send(msg) {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  }
}

// ── Render ────────────────────────────────────────────────────────────────────
function render() {
  catList.innerHTML = '';
  if (cats.size === 0) {
    renderEmpty();
    return;
  }
  const sorted = [...cats.values()].sort((a, b) => a.name.localeCompare(b.name));
  for (const cat of sorted) renderCard(cat);
}

function renderEmpty() {
  catList.innerHTML = '<p class="empty-msg">No cats yet. Add one!</p>';
}

function renderCard(cat) {
  const cardId = `card-${cat.id}`;
  let card = document.getElementById(cardId);
  const isNew = !card;

  if (isNew) {
    card = document.createElement('article');
    card.className = `cat-card ${cat.color}`;
    card.id = cardId;
  } else {
    card.className = `cat-card ${cat.color}`;
  }

  const notesHtml = cat.notes      ? `<div class="card-field"><strong>Notes</strong>${esc(cat.notes)}</div>` : '';
  const foodHtml  = cat.food_notes ? `<div class="card-field"><strong>Food</strong>${esc(cat.food_notes)}</div>` : '';
  const locLabel  = cat.location === 'adoption center' ? 'Adoption Center' : 'Foster';
  const locClass  = cat.location === 'adoption center' ? 'loc-ac' : 'loc-foster';

  card.innerHTML = `
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
  `;

  if (isNew) {
    catList.querySelector('.empty-msg')?.remove();
    const cards = [...catList.querySelectorAll('.cat-card')];
    const after = cards.find(c => c.querySelector('.cat-name').textContent > cat.name);
    catList.insertBefore(card, after ?? null);
  }
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
  fNotes.value = cat.notes;
  fFood.value  = cat.food_notes;
  form.querySelector(`input[name="color"][value="${cat.color}"]`).checked = true;
  form.querySelector(`input[name="location"][value="${cat.location}"]`).checked = true;
  modal.showModal();
  fName.focus();
}

function closeModal() { modal.close(); }

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
      patch: { name: fName.value, color, location, notes: fNotes.value, food_notes: fFood.value },
    });
  } else {
    send({
      type: 'create',
      cat: { name: fName.value, color, location, notes: fNotes.value, food_notes: fFood.value },
    });
  }
  closeModal();
});

catList.addEventListener('click', (e) => {
  const editId   = e.target.closest('[data-edit]')?.dataset.edit;
  const deleteId = e.target.closest('[data-delete]')?.dataset.delete;
  if (editId)   openEdit(editId);
  if (deleteId) { send({ type: 'delete', id: deleteId }); }
});

modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });

// ── PWA service worker ────────────────────────────────────────────────────────
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('/sw.js').catch(() => {});
}

boot();
