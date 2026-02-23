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
let currentViewPath = null;
let pairSyncStatuses = {}; // { pair_id: { type, path, message } }

const mainContent = document.getElementById('main-content');
const detailView = document.getElementById('detail-view');
const fileListBody = document.getElementById('file-list-body');
const btnBack = document.getElementById('btn-back');
const btnAddFile = document.getElementById('btn-add-file');

// ---- Listen for Sync Status ----
listen('sync-status', (event) => {
    const status = event.payload;
    const { pair_id, path, type, message } = status.data || {};

    // Store status for specific pair
    if (pair_id) {
        pairSyncStatuses[pair_id] = { type, path, message };
        updateCardStatus(pair_id);
    }

    // Overall status indicator (top)
    const indicator = document.getElementById('sync-status-indicator');
    const statusText = document.getElementById('sync-status-text');

    if (type === 'Idle') {
        indicator.style.display = 'none';
        // If we are in detail view for this file, refresh it
        if (currentPair && currentPair.id === pair_id) {
            loadFileTable();
        }
    } else {
        indicator.style.display = 'flex';
        if (type === 'Syncing' || type === 'Downloading') {
            statusText.textContent = `${type === 'Syncing' ? 'Syncing' : 'Downloading'} ${path ? path.split(/[\\/]/).pop() : ''}...`;
        } else if (type === 'Uploaded') {
            statusText.textContent = `File Synced!`;
        } else if (type === 'Error') {
            statusText.textContent = `Sync Error`;
            showToast(`Sync Failed: ${message}`, 'error');
        }
    }

    // If detail view is open for this pair, we might want to refresh individual rows
    if (currentPair && currentPair.id === pair_id) {
        // Debounced refresh or targeted row update would be better, but let's try row update
        const rows = document.querySelectorAll('#file-list-body tr');
        rows.forEach(row => {
            const rowPath = row.dataset.path;
            if (rowPath && path && (path === rowPath || path.startsWith(rowPath + '/') || path.startsWith(rowPath + '\\'))) {
                const statusCell = row.querySelector('.file-status-cell');
                if (statusCell) {
                    statusCell.innerHTML = renderFileStatus(type, message);
                }
            }
        });
    }
});

function updateCardStatus(pairId) {
    const card = document.querySelector(`.folder-card[data-id="${pairId}"]`);
    if (!card) return;

    const statusObj = pairSyncStatuses[pairId];
    const statusEl = card.querySelector('.folder-status');

    if (statusObj.type === 'Idle' || !statusObj.type) {
        statusEl.className = 'folder-status active';
        statusEl.innerHTML = `<span class="status-dot"></span>${window.t('active')}`;
    } else {
        statusEl.className = `folder-status ${statusObj.type.toLowerCase()}`;
        let label = statusObj.type;
        if (statusObj.type === 'Syncing') label = 'Syncing...';
        if (statusObj.type === 'Downloading') label = 'Downloading...';
        statusEl.innerHTML = `<span class="status-dot"></span>${label}`;
    }
}

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
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M17.5 19c-3.6 0-6.5-2.9-6.5-6.5s2.9-6.5,6.5-6.5c0.3 0 0.7 0 1 0.1C17.7 3.6 15.1 2 12 2C7.6 2 4 5.6 4 10c0 4.4 3.6 8 8 8 1.9 0 3.7-0.7 5.1-1.8 0.1 0.6 0.3 1.1 0.6 1.6C16.8 18.8 15.2 19 17.5 19z" fill="#5AC8FA"/></svg>`;
        case 'onedrive':
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M22 6.5C22 4.01 19.99 2 17.5 2C16.14 2 14.93 2.6 14.11 3.55C13.4 3.2 12.6 3 11.75 3C9.4 3 7.5 4.9 7.5 7.25C7.5 7.42 7.51 7.58 7.53 7.74C5.55 8.1 4 9.8 4 11.9C4 14.2 5.8 16 8 16H18C20.2 16 22 14.2 22 11.9C22 10.95 21.68 10.08 21.14 9.38C21.68 8.58 22 7.58 22 6.5Z" fill="#0078D4"/></svg>`;
        default:
            return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 8v4l3 3"/></svg>`;
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
    const localBasename = pair.local_path.split(/[\\/]/).filter(Boolean).pop() || pair.local_path;

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
            <div class="folder-status ${statusClass}" id="card-status-${pair.id}">
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
    currentViewPath = pair.local_path;
    document.getElementById('detail-folder-name').textContent = pair.local_path.split(/[\\/]/).pop() || pair.local_path;
    document.getElementById('detail-folder-path').textContent = pair.local_path;

    mainContent.style.display = 'none';
    detailView.style.display = 'block';

    loadFileTable();
}
window.openFolderDetail = openFolderDetail;

async function loadFileTable() {
    if (!currentPair || !currentViewPath) return;

    try {
        const files = await invoke('list_local_files', { path: currentViewPath });
        document.getElementById('detail-folder-path').textContent = currentViewPath;

        // Sort: dirs first, then by name
        files.sort((a, b) => {
            if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
            return a.name.localeCompare(b.name);
        });

        fileListBody.innerHTML = files.map(file => renderFileRow(file)).join('');

        // Apply current sync status to rows if any
        if (currentPair && pairSyncStatuses[currentPair.id]) {
            const statusObj = pairSyncStatuses[currentPair.id];
            const rows = document.querySelectorAll('#file-list-body tr');
            rows.forEach(row => {
                const rowPath = row.dataset.path;
                if (rowPath && statusObj.path && (statusObj.path === rowPath || statusObj.path.startsWith(rowPath + '/') || statusObj.path.startsWith(rowPath + '\\'))) {
                    const statusCell = row.querySelector('.file-status-cell');
                    if (statusCell) {
                        statusCell.innerHTML = renderFileStatus(statusObj.type, statusObj.message);
                    }
                }
            });
        }
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
    }
}

function renderFileRow(file) {
    const sizeStr = file.is_dir ? '--' : formatBytes(file.size);
    const dateStr = new Date(file.modified_at * 1000).toLocaleString();
    const icon = file.is_dir
        ? '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>'
        : '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-secondary)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>';

    return `
        <tr data-path="${file.path}">
            <td>
                <div class="file-name-cell">
                    <span class="file-icon">${icon}</span>
                    <span class="${file.is_dir ? 'dir-link' : ''}" onclick="${file.is_dir ? `navigateToSubfolder('${file.path.replace(/\\/g, '/')}')` : ''}">${file.name}</span>
                </div>
            </td>
            <td>${sizeStr}</td>
            <td class="file-status-cell">${renderFileStatus('Idle')}</td>
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

function navigateToSubfolder(path) {
    currentViewPath = path;
    loadFileTable();
}
window.navigateToSubfolder = navigateToSubfolder;

function renderFileStatus(type, message) {
    if (type === 'Idle') return `<span class="file-status-idle">-</span>`;
    if (type === 'Syncing') return `<span class="file-status-syncing">Uploading...</span>`;
    if (type === 'Downloading') return `<span class="file-status-syncing">Downloading...</span>`;
    if (type === 'Uploaded') return `<span class="file-status-uploaded">Synced</span>`;
    if (type === 'Deleted') return `<span class="file-status-deleted">Deleted</span>`;
    if (type === 'Error') return `<span class="file-status-error" title="${message}">Error</span>`;
    return type;
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
    if (currentPair && currentViewPath && currentViewPath !== currentPair.local_path) {
        // Go up one level
        const path = currentViewPath.replace(/\\/g, '/');
        const parts = path.split('/').filter(Boolean);
        parts.pop();
        currentViewPath = (path.startsWith('/') ? '/' : '') + parts.join('/');
        loadFileTable();
    } else {
        mainContent.style.display = 'block';
        detailView.style.display = 'none';
        currentPair = null;
        currentViewPath = null;
        loadPairs(); // Refresh list
    }
});

btnAddFile.addEventListener('click', async () => {
    if (!currentPair || !currentViewPath) return;

    try {
        const selected = await open({ multiple: false });
        if (selected) {
            showToast(window.t('adding_file'));
            const filename = selected.split(/[\\/]/).pop();
            const dest = `${currentViewPath}/${filename}`;

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
