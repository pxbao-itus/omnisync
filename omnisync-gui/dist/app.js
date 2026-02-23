// ========================================
// OmniSync — Frontend Application Logic
// ========================================

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

// ---- State ----
let syncPairs = [];
let activeFilter = 'all';
let currentProvider = 'gdrive';
let isConnected = false;

// ---- Listen for Sync Status ----
listen('sync-status', (event) => {
    const status = event.payload;
    const indicator = document.getElementById('sync-status-indicator');
    const statusText = document.getElementById('sync-status-text');

    if (status.type === 'Idle') {
        indicator.style.display = 'none';
    } else {
        indicator.style.display = 'flex';
        if (status.type === 'Syncing') {
            statusText.textContent = `Syncing...`;
        } else if (status.type === 'Uploaded') {
            statusText.textContent = `Synced!`;
        } else if (status.type === 'Error') {
            statusText.textContent = `Sync Error`;
            showToast(`Sync Failed: ${status.data.message}`, 'error');
        }
    }
});

// ---- DOM Elements ----
const folderList = document.getElementById('folder-list');
const emptyState = document.getElementById('empty-state');
const modalOverlay = document.getElementById('modal-overlay');
const btnAdd = document.getElementById('btn-add');
const btnClose = document.getElementById('modal-close');
const btnCancel = document.getElementById('btn-cancel');
const btnBrowse = document.getElementById('btn-browse');
const addForm = document.getElementById('add-form');
const inputLocal = document.getElementById('input-local');
const selectRemote = document.getElementById('select-remote');
const subtitle = document.getElementById('subtitle');

const authSection = document.getElementById('auth-section');
const authDisconnected = document.getElementById('auth-disconnected');
const authConnected = document.getElementById('auth-connected');
const syncConfigSection = document.getElementById('sync-config-section');
const syncFields = document.getElementById('sync-fields');
const inputToken = document.getElementById('input-token');
const btnConnect = document.getElementById('btn-connect');
const btnOauth = document.getElementById('btn-oauth');
const btnDisconnect = document.getElementById('btn-disconnect');
const btnAddSubmit = document.getElementById('btn-add-submit');

// ---- Provider helpers ----
const providerLabels = {
    gdrive: 'Google Drive',
    icloud: 'iCloud',
    onedrive: 'OneDrive',
};

function providerIcon(id) {
    switch (id) {
        case 'gdrive':
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M12 2L2 19.5h7.5L12 14l2.5 5.5H22L12 2z" fill="#4285F4"/><path d="M2 19.5l3.5-6L12 14l-2.5 5.5H2z" fill="#FBBC04"/><path d="M9.5 19.5H22l-3.5-6H5.5l3.5 6h.5z" fill="#34A853"/></svg>`;
        case 'icloud':
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#5AC8FA" stroke-width="2"><path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/></svg>`;
        case 'onedrive':
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#0078D4" stroke-width="2"><path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/></svg>`;
        default:
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>`;
    }
}

// ---- Rendering ----
function render() {
    const filtered = activeFilter === 'all'
        ? syncPairs
        : syncPairs.filter(p => p.provider_id === activeFilter);

    // Update badges
    document.getElementById('badge-all').textContent = syncPairs.length;
    document.getElementById('badge-gdrive').textContent = syncPairs.filter(p => p.provider_id === 'gdrive').length;
    document.getElementById('badge-icloud').textContent = syncPairs.filter(p => p.provider_id === 'icloud').length;
    document.getElementById('badge-onedrive').textContent = syncPairs.filter(p => p.provider_id === 'onedrive').length;

    // Update subtitle
    subtitle.textContent = syncPairs.length === 0
        ? 'Manage your synchronized directories'
        : `${syncPairs.length} folder${syncPairs.length !== 1 ? 's' : ''} synced`;

    // Toggle empty state
    if (filtered.length === 0) {
        folderList.style.display = 'none';
        emptyState.style.display = 'flex';
    } else {
        folderList.style.display = 'flex';
        emptyState.style.display = 'none';
        folderList.innerHTML = filtered.map(pair => renderCard(pair)).join('');
    }
}

function renderCard(pair) {
    const statusClass = pair.status || 'active';
    const statusLabel = statusClass.charAt(0).toUpperCase() + statusClass.slice(1);
    const providerLabel = providerLabels[pair.provider_id] || pair.provider_id;
    const localBasename = pair.local_path.split('/').filter(Boolean).pop() || pair.local_path;

    return `
        <div class="folder-card" data-id="${pair.id}">
            <div class="folder-icon ${pair.provider_id}">
                ${providerIcon(pair.provider_id)}
            </div>
            <div class="folder-info">
                <div class="folder-path" title="${pair.local_path}">${localBasename}</div>
                <div class="folder-meta">
                    <span class="meta-item">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                        ${pair.local_path}
                    </span>
                    <span class="meta-item">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 3 21 3 21 9"/><path d="M21 3l-7 7"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>
                        ${pair.remote_path}
                    </span>
                    <span class="meta-item" style="color: var(--provider-${pair.provider_id}, var(--text-tertiary))">
                        ${providerLabel}
                    </span>
                </div>
            </div>
            <div class="folder-status ${statusClass}">
                <span class="status-dot"></span>
                ${statusLabel}
            </div>
            <button class="btn-remove" onclick="removePair(${pair.id})" title="Remove">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                    <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                </svg>
            </button>
        </div>
    `;
}

// ---- Auth & Provider logic ----
async function checkAuth(providerId) {
    try {
        const connected = await invoke('get_auth_status', { providerId });
        isConnected = connected;
        updateUIForStatus(providerId, connected);
        return connected;
    } catch (err) {
        console.error('Failed to check auth:', err);
        return false;
    }
}

function updateUIForStatus(providerId, connected) {
    const statusEl = document.getElementById(`status-${providerId}`);
    const card = document.querySelector(`.provider-card[data-provider="${providerId}"]`);

    if (statusEl) {
        statusEl.textContent = connected ? 'Connected' : 'Not connected';
        if (connected) {
            card.classList.add('connected');
        } else {
            card.classList.remove('connected');
        }
    }

    if (currentProvider === providerId) {
        authSection.style.display = 'block';
        if (connected) {
            authDisconnected.style.display = 'none';
            authConnected.style.display = 'block';
            syncFields.style.opacity = '1';
            syncFields.style.pointerEvents = 'all';
            btnAddSubmit.disabled = false;
            fetchFolders(providerId);
        } else {
            authDisconnected.style.display = 'block';
            authConnected.style.display = 'none';
            syncFields.style.opacity = '0.5';
            syncFields.style.pointerEvents = 'none';
            btnAddSubmit.disabled = true;
        }
    }
}

async function fetchFolders(providerId) {
    try {
        selectRemote.innerHTML = '<option disabled selected>Loading folders...</option>';
        const folders = await invoke('list_remote_folders', { providerId });

        if (folders.length === 0) {
            selectRemote.innerHTML = '<option value="root">Root Directory</option>';
        } else {
            selectRemote.innerHTML = '<option value="root">Root Directory</option>' +
                folders.map(f => `<option value="${f.id}">${f.name}</option>`).join('');
        }
    } catch (err) {
        showToast('Failed to list folders: ' + err, 'error');
        selectRemote.innerHTML = '<option disabled selected>Error loading folders</option>';
    }
}

btnConnect.addEventListener('click', async () => {
    const token = inputToken.value.trim();
    if (!token) return;

    btnConnect.disabled = true;
    btnConnect.textContent = '...';

    try {
        await invoke('connect_provider', { providerId: currentProvider, token });
        showToast('Account connected successfully!', 'success');
        inputToken.value = '';
        await checkAuth(currentProvider);
    } catch (err) {
        showToast('Failed to connect: ' + err, 'error');
    } finally {
        btnConnect.disabled = false;
        btnConnect.textContent = 'Connect';
    }
});

btnOauth.addEventListener('click', async () => {
    btnOauth.disabled = true;
    const originalContent = btnOauth.innerHTML;
    btnOauth.textContent = 'Waiting for login...';

    try {
        await invoke('start_oauth', { providerId: currentProvider });
        showToast('Account connected successfully!', 'success');
        await checkAuth(currentProvider);
    } catch (err) {
        showToast(err, 'error');
    } finally {
        btnOauth.disabled = false;
        btnOauth.innerHTML = originalContent;
    }
});

btnDisconnect.addEventListener('click', async () => {
    if (!confirm('Are you sure you want to disconnect this account?')) return;

    try {
        await invoke('disconnect_provider', { providerId: currentProvider });
        showToast('Account disconnected', 'success');
        await checkAuth(currentProvider);
    } catch (err) {
        showToast('Failed to disconnect: ' + err, 'error');
    }
});

// ---- Data Operations ----
async function loadPairs() {
    try {
        syncPairs = await invoke('get_sync_pairs');
        render();
    } catch (err) {
        showToast('Failed to load sync pairs: ' + err, 'error');
    }
}

async function addPair(local, remote, provider) {
    try {
        await invoke('add_sync_pair', {
            localPath: local,
            remotePath: remote,
            providerId: provider,
        });
        showToast('Folder added to sync list', 'success');
        await loadPairs();
        closeModal();
    } catch (err) {
        showToast('Failed to add folder: ' + err, 'error');
    }
}

async function removePair(id) {
    try {
        await invoke('remove_sync_pair', { id });
        showToast('Folder removed from sync list', 'success');
        await loadPairs();
    } catch (err) {
        showToast('Failed to remove folder: ' + err, 'error');
    }
}
window.removePair = removePair;

// ---- Modal events ----
btnAdd.addEventListener('click', () => {
    modalOverlay.classList.add('open');
    checkAuth(currentProvider);
});

btnClose.addEventListener('click', closeModal);
btnCancel.addEventListener('click', closeModal);

function closeModal() {
    modalOverlay.classList.remove('open');
}

btnBrowse.addEventListener('click', async () => {
    try {
        const selected = await open({ directory: true, multiple: false });
        if (selected) inputLocal.value = selected;
    } catch (err) {
        console.warn('Dialog error:', err);
    }
});

document.querySelectorAll('.provider-card').forEach(card => {
    card.addEventListener('click', () => {
        const provider = card.dataset.provider;
        if (provider === 'icloud' || provider === 'onedrive') return; // Not implemented

        currentProvider = provider;
        document.querySelectorAll('.provider-card').forEach(c => c.classList.remove('selected'));
        card.classList.add('selected');
        card.querySelector('input').checked = true;

        checkAuth(provider);
    });
});

addForm.addEventListener('submit', async e => {
    e.preventDefault();
    if (btnAddSubmit.disabled) return;

    const local = inputLocal.value.trim();
    const remote = selectRemote.value;
    const provider = currentProvider;

    if (!local || !remote) return;
    await addPair(local, remote, provider);
});

// ---- Sidebar ----
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', () => {
        document.querySelectorAll('.nav-item').forEach(i => i.classList.remove('active'));
        item.classList.add('active');
        activeFilter = item.dataset.provider;
        render();
    });
});

// ---- Toast ----
function showToast(message, type = 'success') {
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;
    document.body.appendChild(toast);
    setTimeout(() => toast.remove(), 3000);
}

// ---- Init ----
document.addEventListener('DOMContentLoaded', () => {
    loadPairs();
});
