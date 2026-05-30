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

let editingId = null; // null = create mode

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

  ws.addEventListener('close', () => {
    connDot.className = 'conn-dot disconnected';
    connDot.title = 'Disconnected — reconnecting…';
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

  const notesHtml  = cat.notes     ? `<div class="card-field"><strong>Notes</strong>${esc(cat.notes)}</div>` : '';
  const foodHtml   = cat.food_notes ? `<div class="card-field"><strong>Food</strong>${esc(cat.food_notes)}</div>` : '';

  card.innerHTML = `
    <div class="card-header">
      <span class="cat-name">${esc(cat.name)}</span>
      <span class="chip ${cat.color}">${esc(cat.color)}</span>
      <div class="card-actions">
        <button class="btn-icon" title="Edit" data-edit="${cat.id}">✏️</button>
        <button class="btn-icon" title="Delete" data-delete="${cat.id}">🗑️</button>
      </div>
    </div>
    ${notesHtml}
    ${foodHtml}
  `;

  if (isNew) {
    // Remove empty placeholder if present.
    catList.querySelector('.empty-msg')?.remove();
    // Insert in alphabetical order.
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
  form.querySelector(`input[value="${cat.color}"]`).checked = true;
  modal.showModal();
  fName.focus();
}

function closeModal() { modal.close(); }

// ── Events ────────────────────────────────────────────────────────────────────
addBtn.addEventListener('click', openCreate);
cancelBtn.addEventListener('click', closeModal);

form.addEventListener('submit', (e) => {
  e.preventDefault();
  const color = form.querySelector('input[name="color"]:checked')?.value;
  if (!color) return;

  if (editingId) {
    send({
      type: 'update',
      id: editingId,
      patch: { name: fName.value, color, notes: fNotes.value, food_notes: fFood.value },
    });
  } else {
    send({
      type: 'create',
      cat: { name: fName.value, color, notes: fNotes.value, food_notes: fFood.value },
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

// Close modal on backdrop click.
modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });

// ── PWA service worker ────────────────────────────────────────────────────────
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('/sw.js').catch(() => {});
}

connect();
