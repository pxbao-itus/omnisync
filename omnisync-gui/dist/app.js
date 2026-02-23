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
let currentPair = null;

const mainContent = document.getElementById('main-content');
const detailView = document.getElementById('detail-view');
const fileListBody = document.getElementById('file-list-body');
const btnBack = document.getElementById('btn-back');
const btnAddFile = document.getElementById('btn-add-file');

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

// ---- Theme Management ----
function setupTheme() {
    const themeBtns = document.querySelectorAll('.theme-btn');
    const savedTheme = localStorage.getItem('omnisync-theme') || 'system';

    const applyTheme = (mode) => {
        let themeToApply = mode;
        if (mode === 'system') {
            themeToApply = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
        }

        document.documentElement.setAttribute('data-theme', themeToApply);

        // Update UI
        themeBtns.forEach(btn => {
            if (btn.dataset.themeMode === mode) {
                btn.classList.add('active');
            } else {
                btn.classList.remove('active');
            }
        });

        localStorage.setItem('omnisync-theme', mode);
    };

    themeBtns.forEach(btn => {
        btn.addEventListener('click', () => applyTheme(btn.dataset.themeMode));
    });

    // Listen for system theme changes
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
        if (localStorage.getItem('omnisync-theme') === 'system') {
            applyTheme('system');
        }
    });

    // Initial apply
    applyTheme(savedTheme);
}

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
    const statusLabel = window.t(statusClass);
    const providerLabel = providerLabels[pair.provider_id] || pair.provider_id;
    const localBasename = pair.local_path.split('/').filter(Boolean).pop() || pair.local_path;

    return `
        <div class="folder-card" data-id="${pair.id}" onclick="openFolderDetail(${pair.id})">
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
                        ${pair.remote_name}
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
            <button class="btn-remove" onclick="event.stopPropagation(); removePair(${pair.id})" title="Remove">
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
        const userInfo = await invoke('get_auth_status', { providerId });
        isConnected = !!userInfo;
        updateUIForStatus(providerId, userInfo);
        return isConnected;
    } catch (err) {
        console.error('Failed to check auth:', err);
        return false;
    }
}

function updateUIForStatus(providerId, userInfo) {
    const connected = !!userInfo;
    const statusEl = document.getElementById(`status-${providerId}`);
    const card = document.querySelector(`.provider-card[data-provider="${providerId}"]`);

    if (statusEl) {
        statusEl.textContent = connected ? window.t('connected') : window.t('not_connected');
        if (connected) {
            card.classList.add('connected');
        } else {
            card.classList.remove('connected');
        }
    }

    // Update sidebar profile if this is the active filter
    const sidebarProfile = document.getElementById('user-profile');
    if (activeFilter === providerId) {
        if (connected) {
            sidebarProfile.style.display = 'flex';
            sidebarProfile.innerHTML = `
                <div class="profile-avatar">
                    ${userInfo.avatar ? `<img src="${userInfo.avatar}" style="width: 100%; height: 100%; object-fit: cover;" />` : `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`}
                </div>
                <div class="profile-info">
                    <div class="profile-name">${userInfo.name || window.t('connected')}</div>
                    <div class="profile-email">${userInfo.email || providerLabels[providerId]}</div>
                </div>
            `;
        } else {
            sidebarProfile.style.display = 'none';
        }
    } else if (activeFilter === 'all') {
        sidebarProfile.style.display = 'none';
    }

    if (currentProvider === providerId) {
        authSection.style.display = 'block';
        if (connected) {
            authDisconnected.style.display = 'none';
            authConnected.style.display = 'block';
            syncFields.style.opacity = '1';
            syncFields.style.pointerEvents = 'all';
            btnAddSubmit.disabled = false;

            // Update user info display
            const avatarEl = authConnected.querySelector('img') || authConnected.querySelector('svg');
            const nameEl = document.getElementById('connected-account');

            if (userInfo.avatar) {
                authConnected.querySelector('div[style*="width: 40px"]').innerHTML = `<img src="${userInfo.avatar}" style="width: 100%; height: 100%; border-radius: 50%; object-fit: cover;" />`;
            } else {
                authConnected.querySelector('div[style*="width: 40px"]').innerHTML = `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`;
            }

            nameEl.innerHTML = `
                <div style="font-weight: 600;">${userInfo.name || window.t('connected')}</div>
                <div style="font-size: 11px; opacity: 0.7;">${userInfo.email || providerLabels[providerId]}</div>
            `;

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
        selectRemote.innerHTML = `<option disabled selected>${window.t('loading_folders')}</option>`;
        const folders = await invoke('list_remote_folders', { providerId });

        if (folders.length === 0) {
            selectRemote.innerHTML = `<option value="root">${window.t('root_directory')}</option>`;
        } else {
            selectRemote.innerHTML = `<option value="root">${window.t('root_directory')}</option>` +
                folders.map(f => `<option value="${f.id}">${f.name}</option>`).join('');
        }
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
        selectRemote.innerHTML = `<option disabled selected>${window.t('error_loading_folders')}</option>`;
    }
}

btnConnect.addEventListener('click', async () => {
    const token = inputToken.value.trim();
    if (!token) return;

    btnConnect.disabled = true;
    btnConnect.textContent = '...';

    try {
        await invoke('connect_provider', { providerId: currentProvider, token });
        showToast(window.t('account_connected_success'), 'success');
        inputToken.value = '';
        await checkAuth(currentProvider);
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
    } finally {
        btnConnect.disabled = false;
        btnConnect.textContent = window.t('connect');
    }
});

btnOauth.addEventListener('click', async () => {
    btnOauth.disabled = true;
    const originalContent = btnOauth.innerHTML;
    // Replace text while preserving SVG
    const svg = btnOauth.querySelector('svg');
    btnOauth.innerHTML = '';
    if (svg) btnOauth.appendChild(svg);
    btnOauth.appendChild(document.createTextNode(' ' + window.t('waiting_login')));

    try {
        await invoke('start_oauth', { providerId: currentProvider });
        showToast(window.t('account_connected_success'), 'success');
        await checkAuth(currentProvider);
    } catch (err) {
        showToast(err, 'error');
    } finally {
        btnOauth.disabled = false;
        btnOauth.innerHTML = originalContent;
    }
});

btnDisconnect.addEventListener('click', async () => {
    if (!confirm(window.t('are_you_sure_disconnect'))) return;

    try {
        await invoke('disconnect_provider', { providerId: currentProvider });
        showToast(window.t('account_disconnected'), 'success');
        await checkAuth(currentProvider);
    } catch (err) {
        showToast(window.t('failed_disconnect') + ' ' + err, 'error');
    }
});

// ---- Data Operations ----
async function loadPairs() {
    try {
        syncPairs = await invoke('get_sync_pairs');
        render();
    } catch (err) {
        showToast(window.t('failed_load_pairs') + ' ' + err, 'error');
    }
}

async function addPair(local, remote, remoteName, provider) {
    try {
        await invoke('add_sync_pair', {
            localPath: local,
            remotePath: remote,
            remoteName: remoteName,
            providerId: provider,
        });
        showToast(window.t('folder_synced'), 'success');
        await loadPairs();
        closeModal();
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
    }
}

async function removePair(id) {
    if (!confirm(window.t('confirm_remove_pair'))) return;

    try {
        await invoke('remove_sync_pair', { id });
        showToast(window.t('pair_removed'), 'success');
        await loadPairs();
    } catch (err) {
        showToast(window.t('failed_remove_pair') + ' ' + err, 'error');
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
    const remoteName = selectRemote.options[selectRemote.selectedIndex].text;
    const provider = currentProvider;

    if (!local || !remote) return;
    await addPair(local, remote, remoteName, provider);
});

// ---- Sidebar ----
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', async () => {
        document.querySelectorAll('.nav-item').forEach(i => i.classList.remove('active'));
        item.classList.add('active');
        activeFilter = item.dataset.provider;

        if (activeFilter !== 'all') {
            await checkAuth(activeFilter);
        } else {
            document.getElementById('user-profile').style.display = 'none';
        }

        render();
    });
});

// ---- Detail View Logic ----
async function openFolderDetail(id) {
    const pair = syncPairs.find(p => p.id === id);
    if (!pair) return;

    currentPair = pair;
    document.getElementById('detail-folder-name').textContent = pair.local_path.split('/').pop() || pair.local_path;
    document.getElementById('detail-folder-path').textContent = pair.local_path;

    mainContent.style.display = 'none';
    detailView.style.display = 'block';

    loadFileTable();
}
window.openFolderDetail = openFolderDetail;

async function loadFileTable() {
    if (!currentPair) return;

    try {
        const files = await invoke('list_local_files', { path: currentPair.local_path });

        // Sort: dirs first, then by name
        files.sort((a, b) => {
            if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
            return a.name.localeCompare(b.name);
        });

        fileListBody.innerHTML = files.map(file => renderFileRow(file)).join('');
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
    }
}

function renderFileRow(file) {
    const sizeStr = file.is_dir ? '--' : formatBytes(file.size);
    const dateStr = new Date(file.modified_at * 1000).toLocaleString();
    const icon = file.is_dir
        ? '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>'
        : '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>';

    return `
        <tr>
            <td>
                <div class="file-name-cell">
                    <span class="file-icon">${icon}</span>
                    <span>${file.name}</span>
                </div>
            </td>
            <td>${sizeStr}</td>
            <td>${dateStr}</td>
            <td>
                <div class="file-actions">
                    <button class="btn-file-action delete" onclick="deleteFile('${file.path.replace(/\\/g, '/')}')" title="Delete">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                    </button>
                </div>
            </td>
        </tr>
    `;
}

async function deleteFile(path) {
    if (!confirm(window.t('confirm_delete_file'))) return;

    try {
        showToast(window.t('deleting'));
        await invoke('delete_local_file', { path });
        showToast(window.t('file_deleted'), 'success');
        loadFileTable();
    } catch (err) {
        showToast(window.t('failed_delete') + ' ' + err, 'error');
    }
}
window.deleteFile = deleteFile;

btnBack.addEventListener('click', () => {
    mainContent.style.display = 'block';
    detailView.style.display = 'none';
    currentPair = null;
    loadPairs(); // Refresh list
});

btnAddFile.addEventListener('click', async () => {
    if (!currentPair) return;

    try {
        const selected = await open({ multiple: false });
        if (selected) {
            showToast(window.t('adding_file'));
            const filename = selected.split(/[\\/]/).pop();
            const dest = `${currentPair.local_path}/${filename}`;

            await invoke('copy_file', { src: selected, dest });
            showToast(window.t('file_added'), 'success');
            loadFileTable();
        }
    } catch (err) {
        showToast(window.t('failed_add_file') + ' ' + err, 'error');
    }
});

function formatBytes(bytes, decimals = 2) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

// ---- Toast ----
function showToast(message, type = 'success') {
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;
    document.body.appendChild(toast);
    setTimeout(() => toast.remove(), 3000);
}

// ---- Init ----
document.addEventListener('DOMContentLoaded', async () => {
    setupTheme();
    await loadPairs();

    // Check auth for whatever is selected by default
    if (activeFilter !== 'all') {
        checkAuth(activeFilter);
    }
});
