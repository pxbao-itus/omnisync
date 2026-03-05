// ========================================
// OmniSync — Frontend Application Logic
// ========================================

const { invoke } = window.__TAURI__.core;
const { open, ask } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

// ---- State ----
let syncPairs = [];
let activeFilter = 'all';
let currentProvider = 'gdrive';
let currentPair = null;
let currentViewPath = null;
let pairSyncStatuses = {}; // { pair_id: { type, path, message } }
let connectedAccounts = []; // [{ account_id, provider_id, name, email, avatar }]

const mainContent = document.getElementById('main-content');
const detailView = document.getElementById('detail-view');
const fileListBody = document.getElementById('file-list-body');
const btnBack = document.getElementById('btn-back');
const btnAddFile = document.getElementById('btn-add-file');

// ---- Listen for Sync Status ----
listen('sync-status', (event) => {
    const status = event.payload;
    const type = status.type;
    const { pair_id, path, message, account_id } = status.data || {};

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
        if (currentPair && currentPair.id === pair_id) {
            loadFileTable();
        }
    } else if (type === 'AuthExpired') {
        // Token expired and could not be refreshed — auto-logout
        indicator.style.display = 'none';
        showToast(window.t('session_expired') || `Session expired. Please reconnect.`, 'error');
        // Refresh connected accounts
        loadAccounts();
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

    // If detail view is open for this pair, update row statuses
    if (currentPair && currentPair.id === pair_id) {
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
        let icon = '<span class="status-dot"></span>';

        if (statusObj.type === 'Syncing') {
            label = 'Syncing...';
            icon = '<div class="sync-spinner-mini"></div>';
        } else if (statusObj.type === 'Downloading') {
            label = 'Downloading...';
            icon = '<div class="sync-spinner-mini"></div>';
        } else if (statusObj.type === 'Uploaded') {
            label = 'Synced';
            icon = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" style="margin-right:4px;"><polyline points="20 6 9 17 4 12"></polyline></svg>';
        } else if (statusObj.type === 'Error') {
            label = 'Error';
            icon = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" style="margin-right:4px;"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>';
        }

        statusEl.innerHTML = `${icon}${label}`;
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
const subtitle = document.getElementById('subtitle');

const authSection = document.getElementById('auth-section');
const accountsContainer = document.getElementById('accounts-container');
const btnAddAccount = document.getElementById('btn-add-account');
const accountSelectorGroup = document.getElementById('account-selector-group');
const selectAccount = document.getElementById('select-account');
const syncFields = document.getElementById('sync-fields');
const btnAddSubmit = document.getElementById('btn-add-submit');

// Searchable remote folder picker
const inputRemoteSearch = document.getElementById('input-remote-search');
const selectRemote = document.getElementById('select-remote'); // hidden input
const remoteFolderList = document.getElementById('remote-folder-list');
let _remoteFolders = []; // [{ id, name }]
let _selectedRemoteName = '';

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

    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
        if (localStorage.getItem('omnisync-theme') === 'system') {
            applyTheme('system');
        }
    });

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

    // Find account info
    const account = connectedAccounts.find(a => a.account_id === pair.account_id);
    const accountEmail = account ? account.email : pair.account_id;

    return `
        <div class="folder-card" data-id="${pair.id}" onclick="if(!event.target.closest('button')) openFolderDetail(${pair.id})">
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
                        ${accountEmail || providerLabel}
                    </span>
                </div>
            </div>
            <div style="display: flex; align-items: center; gap: 16px;">
                <div class="folder-status ${statusClass}" id="card-status-${pair.id}">
                    <span class="status-dot"></span>
                    ${statusLabel}
                </div>
                <div class="card-actions" style="display: flex; gap: 8px;">
                    <button class="btn-sync-now" onclick="event.stopPropagation(); syncPairNow(event, ${pair.id})" title="${window.t('sync_now')}"
                        style="width: 32px; height: 32px; background: rgba(0, 210, 255, 0.1); color: var(--accent); border: 1px solid rgba(0, 210, 255, 0.2); border-radius: 50%; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s;">
                        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
                        </svg>
                    </button>
                    <button class="btn-remove" onclick="event.stopPropagation(); removePair(event, ${pair.id})" title="Remove"
                        style="width: 32px; height: 32px; background: rgba(255, 82, 82, 0.1); color: #ff5252; border: 1px solid rgba(255, 82, 82, 0.2); border-radius: 50%; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s;">
                        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                            <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        </svg>
                    </button>
                </div>
            </div>
        </div>
    `;
}

async function syncPairNow(e, id) {
    if (e) e.stopPropagation();
    try {
        await invoke('sync_pair_now', { id });
        showToast(window.t('syncing') || 'Syncing...', 'info');
    } catch (err) {
        showToast(window.t('failed_sync') || 'Sync failed: ' + err, 'error');
    }
}
window.syncPairNow = syncPairNow;


// ---- Multi-Account Logic ----
async function loadAccounts() {
    try {
        connectedAccounts = await invoke('get_all_accounts');
        renderAccountsList();
        updateAccountSelector();
        updateSidebarProfile();
    } catch (err) {
        console.error('Failed to load accounts:', err);
    }
}

function renderAccountsList() {
    const providerAccounts = connectedAccounts.filter(a => a.provider_id === currentProvider);

    if (providerAccounts.length === 0) {
        accountsContainer.innerHTML = `<p style="font-size: 12px; opacity: 0.5; margin: 0;">${window.t('no_accounts_connected') || 'No accounts connected yet.'}</p>`;
    } else {
        accountsContainer.innerHTML = providerAccounts.map(account => `
            <div class="account-item" style="display: flex; align-items: center; gap: 10px; background: rgba(0, 210, 255, 0.05); border: 1px solid rgba(0, 210, 255, 0.2); border-radius: 8px; padding: 10px 12px;">
                <div style="width: 32px; height: 32px; border-radius: 50%; overflow: hidden; background: var(--bg-tertiary); display: flex; align-items: center; justify-content: center; flex-shrink: 0;">
                    ${account.avatar ? `<img src="${account.avatar}" style="width: 100%; height: 100%; object-fit: cover;" />` : `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`}
                </div>
                <div style="flex: 1; min-width: 0;">
                    <div style="font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${account.name || window.t('connected')}</div>
                    <div style="font-size: 11px; opacity: 0.6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${account.email || account.account_id}</div>
                </div>
                <button type="button" onclick="disconnectAccount('${account.account_id}')" 
                    style="padding: 4px 10px; background: rgba(255, 82, 82, 0.1); color: #ff5252; border: 1px solid rgba(255, 82, 82, 0.3); border-radius: 6px; font-size: 10px; font-weight: 600; cursor: pointer; flex-shrink: 0;">
                    ${window.t('disconnect')}
                </button>
            </div>
        `).join('');
    }
}

function updateAccountSelector() {
    const providerAccounts = connectedAccounts.filter(a => a.provider_id === currentProvider);

    if (providerAccounts.length === 0) {
        accountSelectorGroup.style.display = 'none';
        syncFields.style.opacity = '0.5';
        syncFields.style.pointerEvents = 'none';
        btnAddSubmit.disabled = true;
    } else {
        accountSelectorGroup.style.display = 'block';

        selectAccount.innerHTML = `<option value="" disabled selected>${window.t('select_account') || 'Select an account...'}</option>` +
            providerAccounts.map(a => `<option value="${a.account_id}">${a.email || a.name || a.account_id}</option>`).join('');

        // Auto-select if only one account
        if (providerAccounts.length === 1) {
            selectAccount.value = providerAccounts[0].account_id;
            onAccountSelected(providerAccounts[0].account_id);
        }
    }
}

function onAccountSelected(accountId) {
    syncFields.style.opacity = '1';
    syncFields.style.pointerEvents = 'all';
    btnAddSubmit.disabled = false;
    fetchFolders(accountId);
}

selectAccount.addEventListener('change', () => {
    onAccountSelected(selectAccount.value);
});

function updateSidebarProfile() {
    const sidebarProfile = document.getElementById('user-profile');

    if (activeFilter !== 'all') {
        const providerAccounts = connectedAccounts.filter(a => a.provider_id === activeFilter);
        if (providerAccounts.length > 0) {
            sidebarProfile.style.display = 'flex';
            sidebarProfile.style.flexDirection = 'column';
            sidebarProfile.style.gap = '6px';
            sidebarProfile.innerHTML = providerAccounts.map(a => `
                <div style="display: flex; align-items: center; gap: 8px;">
                    <div class="profile-avatar" style="width: 24px; height: 24px; border-radius: 50%; overflow: hidden; background: var(--bg-tertiary); display: flex; align-items: center; justify-content: center; flex-shrink: 0;">
                        ${a.avatar ? `<img src="${a.avatar}" style="width: 100%; height: 100%; object-fit: cover;" />` : `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`}
                    </div>
                    <div style="flex: 1; min-width: 0;">
                        <div style="font-size: 11px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${a.name || window.t('connected')}</div>
                        <div style="font-size: 9px; opacity: 0.6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${a.email || ''}</div>
                    </div>
                </div>
            `).join('');
        } else {
            sidebarProfile.style.display = 'none';
        }
    } else {
        sidebarProfile.style.display = 'none';
    }
}

async function disconnectAccount(accountId) {
    if (!confirm(window.t('are_you_sure_disconnect'))) return;
    try {
        await invoke('disconnect_account', { accountId });
        showToast(window.t('account_disconnected'), 'success');
        await loadAccounts();
    } catch (err) {
        showToast(window.t('failed_disconnect') + ' ' + err, 'error');
    }
}
window.disconnectAccount = disconnectAccount;

// ---- Add Account (OAuth) ----
btnAddAccount.addEventListener('click', async () => {
    btnAddAccount.disabled = true;
    const originalContent = btnAddAccount.innerHTML;
    const svg = btnAddAccount.querySelector('svg');
    btnAddAccount.innerHTML = '';
    if (svg) btnAddAccount.appendChild(svg);
    btnAddAccount.appendChild(document.createTextNode(' ' + (window.t('waiting_login') || 'Waiting for login...')));

    try {
        const accountId = await invoke('start_oauth', { providerId: currentProvider });
        showToast(window.t('account_connected_success'), 'success');
        await loadAccounts();
    } catch (err) {
        showToast(err, 'error');
    } finally {
        btnAddAccount.disabled = false;
        btnAddAccount.innerHTML = originalContent;
    }
});

// ---- Folder fetching (searchable) ----
let _fetchingFolders = false;
async function fetchFolders(accountId) {
    if (_fetchingFolders) return;
    _fetchingFolders = true;
    _remoteFolders = [];
    _selectedRemoteName = '';
    selectRemote.value = '';
    inputRemoteSearch.value = '';
    remoteFolderList.style.display = 'none';

    try {
        inputRemoteSearch.placeholder = window.t('loading_folders') || 'Loading folders...';
        inputRemoteSearch.disabled = true;
        const folders = await invoke('list_remote_folders', { accountId });

        _remoteFolders = [{ id: 'root', name: window.t('root_directory') || 'Root Directory' }];
        if (folders.length > 0) {
            _remoteFolders = _remoteFolders.concat(folders.map(f => ({ id: f.id, name: f.name })));
        }
        inputRemoteSearch.placeholder = window.t('search_folders') || 'Search folders...';
        inputRemoteSearch.disabled = false;
        renderFolderList('');
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
        inputRemoteSearch.placeholder = window.t('error_loading_folders') || 'Error loading folders';
        inputRemoteSearch.disabled = true;
        await loadAccounts();
    } finally {
        _fetchingFolders = false;
    }
}

function renderFolderList(query) {
    const q = query.toLowerCase().trim();
    const filtered = q ? _remoteFolders.filter(f => f.name.toLowerCase().includes(q)) : _remoteFolders;

    if (filtered.length === 0) {
        remoteFolderList.innerHTML = `<div style="padding: 10px 14px; font-size: 12px; opacity: 0.5;">No folders found</div>`;
    } else {
        remoteFolderList.innerHTML = filtered.map(f => `
            <div class="remote-folder-item" data-id="${f.id}" data-name="${f.name}"
                style="padding: 8px 14px; font-size: 13px; cursor: pointer; border-bottom: 1px solid var(--border-subtle); transition: background 0.15s;"
                onmouseenter="this.style.background='var(--bg-tertiary)'"
                onmouseleave="this.style.background='transparent'"
                onclick="selectFolder('${f.id}', '${f.name.replace(/'/g, "\\'")}')"
            >${f.name}</div>
        `).join('');
    }
    remoteFolderList.style.display = 'block';
}

function selectFolder(id, name) {
    selectRemote.value = id;
    _selectedRemoteName = name;
    inputRemoteSearch.value = name;
    remoteFolderList.style.display = 'none';
}
window.selectFolder = selectFolder;

inputRemoteSearch.addEventListener('input', () => {
    selectRemote.value = '';
    _selectedRemoteName = '';
    renderFolderList(inputRemoteSearch.value);
});

inputRemoteSearch.addEventListener('focus', () => {
    if (_remoteFolders.length > 0) {
        renderFolderList(inputRemoteSearch.value);
    }
});

// Close folder list when clicking outside
document.addEventListener('click', (e) => {
    if (!e.target.closest('#remote-folder-picker')) {
        remoteFolderList.style.display = 'none';
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

async function addPair(local, remote, remoteName, provider, accountId) {
    try {
        await invoke('add_sync_pair', {
            localPath: local,
            remotePath: remote,
            remoteName: remoteName,
            providerId: provider,
            accountId: accountId,
        });
        showToast(window.t('folder_synced'), 'success');
        await loadPairs();
        closeModal();
    } catch (err) {
        showToast(window.t('failed_connect') + ' ' + err, 'error');
    }
}

async function removePair(e, id) {
    if (e) {
        e.stopPropagation();
        e.preventDefault();
    }

    const confirmed = await ask(window.t('confirm_remove_pair'), {
        title: 'OmniSync',
        kind: 'warning'
    });

    if (!confirmed) return;

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
    loadAccounts();
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
        if (provider === 'icloud' || provider === 'onedrive') return;

        currentProvider = provider;
        document.querySelectorAll('.provider-card').forEach(c => c.classList.remove('selected'));
        card.classList.add('selected');
        card.querySelector('input').checked = true;

        loadAccounts();
    });
});

addForm.addEventListener('submit', async e => {
    e.preventDefault();
    if (btnAddSubmit.disabled) return;

    const local = inputLocal.value.trim();
    const remote = selectRemote.value;
    const remoteName = _selectedRemoteName || inputRemoteSearch.value || 'Root Directory';
    const accountId = selectAccount.value;
    const provider = currentProvider;

    if (!local || !remote || !accountId) return;
    await addPair(local, remote, remoteName, provider, accountId);
});

// ---- Sidebar ----
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', async () => {
        document.querySelectorAll('.nav-item').forEach(i => i.classList.remove('active'));
        item.classList.add('active');
        activeFilter = item.dataset.provider;

        updateSidebarProfile();
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

        files.sort((a, b) => {
            if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
            return a.name.localeCompare(b.name);
        });

        fileListBody.innerHTML = files.map(file => renderFileRow(file)).join('');

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
                    <button class="btn-file-action delete" onclick="deleteFile(event, '${file.path.replace(/\\/g, '/')}')" title="Delete">
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

    if (type === 'Syncing' || type === 'Downloading') {
        const label = type === 'Syncing' ? 'Syncing...' : 'Downloading...';
        return `
            <div class="file-status-container syncing">
                <div class="sync-spinner-mini"></div>
                <span class="file-status-syncing">${label}</span>
            </div>
        `;
    }

    if (type === 'Uploaded') {
        return `
            <div class="file-status-container uploaded">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
                <span class="file-status-uploaded">Synced</span>
            </div>
        `;
    }

    if (type === 'Deleted') return `<span class="file-status-deleted">Deleted</span>`;

    if (type === 'Error') {
        return `
            <div class="file-status-container error" title="${message}">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
                <span class="file-status-error">Error</span>
            </div>
        `;
    }

    return type;
}

async function deleteFile(event, path) {
    if (event) {
        event.stopPropagation();
        event.preventDefault();
    }

    const confirmed = await ask(window.t('confirm_delete_file'), {
        title: 'OmniSync',
        kind: 'warning'
    });

    if (!confirmed) return;

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
        loadPairs();
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
    await loadAccounts();
    await loadPairs();
});
