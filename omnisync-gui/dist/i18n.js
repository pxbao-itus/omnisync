const translations = {
    en: {
        providers: "Providers",
        all_providers: "All Providers",
        soon: "Soon",
        system_theme: "System Theme",
        light_theme: "Light Theme",
        dark_theme: "Dark Theme",
        synced_folders: "Synced Folders",
        manage_directories: "Manage your synchronized directories",
        syncing: "Syncing...",
        add_folder: "Add Folder",
        no_folders: "No folders synced yet",
        no_folders_desc: 'Click "Add Folder" to start syncing a directory with your cloud provider.',
        back: "Back",
        add_file: "Add File",
        name: "Name",
        size: "Size",
        modified: "Modified",
        actions: "Actions",
        add_sync_folder: "Add Sync Folder",
        cloud_provider: "Cloud Provider",
        not_connected: "Not connected",
        connected: "Connected",
        connect_account: "Connect Account",
        connect_desc: "Securely connect your account to start syncing.",
        login_google: "Log in with Google",
        or: "or",
        manual_token: "Manual Access Token",
        paste_token: "Paste token...",
        connect: "Connect",
        account_connected: "Account Connected",
        disconnect: "Disconnect",
        local_path: "Local Folder Path",
        browse: "Browse",
        remote_folder: "Remote Folder",
        cancel: "Cancel",
        folder_synced: "folder synced",
        folders_synced: "folders synced",
        active: "Active",
        paused: "Paused",
        error: "Error",
        loading_folders: "Loading folders...",
        root_directory: "Root Directory",
        error_loading_folders: "Error loading folders",
        waiting_login: "Waiting for login...",
        account_connected_success: "Account connected successfully!",
        failed_connect: "Failed to connect:",
        failed_check_auth: "Failed to check auth:",
        are_you_sure_disconnect: "Are you sure you want to disconnect this account?",
        account_disconnected: "Account disconnected",
        failed_disconnect: "Failed to disconnect:",
        failed_load_pairs: "Failed to load sync pairs:",
        adding_file: "Adding file...",
        file_added: "File added successfully",
        failed_add_file: "Failed to add file:",
        confirm_delete_file: "Are you sure you want to delete this file from the cloud?",
        deleting: "Deleting...",
        file_deleted: "File deleted successfully",
        failed_delete: "Failed to delete:",
        confirm_remove_pair: "Are you sure you want to remove this sync pair?",
        removing: "Removing...",
        pair_removed: "Sync pair removed successfully",
        failed_remove_pair: "Failed to remove sync pair:"
    },
    vi: {
        providers: "Các Dịch Vụ Lưu Trữ Đám Mây",
        all_providers: "Tất Cả",
        soon: "Sắp có",
        system_theme: "Giao Diện Hệ Thống",
        light_theme: "Giao Diện Sáng",
        dark_theme: "Giao Diện Tối",
        synced_folders: "Thư Mục Đã Đồng Bộ",
        manage_directories: "Quản lý các thư mục đồng bộ của bạn",
        syncing: "Đang đồng bộ...",
        add_folder: "Thêm Thư Mục",
        no_folders: "Chưa có thư mục nào được đồng bộ",
        no_folders_desc: 'Nhấn "Thêm Thư Mục" để bắt đầu đồng bộ thư mục với đám mây của bạn.',
        back: "Quay lại",
        add_file: "Thêm Tệp",
        name: "Tên",
        size: "Kích thước",
        modified: "Ngày sửa",
        actions: "Hành động",
        add_sync_folder: "Thêm Thư Mục Đồng Bộ",
        cloud_provider: "Dịch Vụ Đám Mây",
        not_connected: "Chưa kết nối",
        connected: "Đã kết nối",
        connect_account: "Kết Nối Tài Khoản",
        connect_desc: "Kết nối tài khoản của bạn một cách an toàn để bắt đầu.",
        login_google: "Đăng nhập với Google",
        or: "hoặc",
        manual_token: "Mã token thủ công",
        paste_token: "Dán mã token...",
        connect: "Kết nối",
        account_connected: "Tài Khoản Đã Kết Nối",
        disconnect: "Ngắt Kết Nối",
        local_path: "Đường Dẫn Thư Mục Trên Máy",
        browse: "Duyệt",
        remote_folder: "Thư Mục Đám Mây",
        cancel: "Hủy",
        folder_synced: "thư mục đã đồng bộ",
        folders_synced: "thư mục đã đồng bộ",
        active: "Hoạt động",
        paused: "Tạm dừng",
        error: "Lỗi",
        loading_folders: "Đang tải thư mục...",
        root_directory: "Thư Mục Gốc",
        error_loading_folders: "Lỗi tải thư mục",
        waiting_login: "Đang chờ đăng nhập...",
        account_connected_success: "Kết nối tài khoản thành công!",
        failed_connect: "Kết nối thất bại:",
        failed_check_auth: "Kiểm tra xác thực thất bại:",
        are_you_sure_disconnect: "Bạn có chắc muốn ngắt kết nối tài khoản này không?",
        account_disconnected: "Đã ngắt kết nối tài khoản",
        failed_disconnect: "Ngắt kết nối thất bại:",
        failed_load_pairs: "Không thể tải danh sách đồng bộ:",
        adding_file: "Đang thêm tệp...",
        file_added: "Đã thêm tệp thành công",
        failed_add_file: "Thêm tệp thất bại:",
        confirm_delete_file: "Bạn có chắc muốn xóa tệp này khỏi đám mây không?",
        deleting: "Đang xóa...",
        file_deleted: "Đã xóa tệp thành công",
        failed_delete: "Xóa thất bại:",
        confirm_remove_pair: "Bạn có chắc muốn gỡ bỏ thư mục đồng bộ này không?",
        removing: "Đang gỡ bỏ...",
        pair_removed: "Đã gỡ bỏ thư mục đồng bộ thành công",
        failed_remove_pair: "Gỡ bỏ thất bại:"
    }
};

window.currentLang = localStorage.getItem('omnisync-lang') || 'en';

window.t = function (key) {
    if (translations[window.currentLang] && translations[window.currentLang][key]) {
        return translations[window.currentLang][key];
    }
    return key;
}

window.setLanguage = function (lang) {
    if (translations[lang]) {
        window.currentLang = lang;
        localStorage.setItem('omnisync-lang', lang);
        window.updateDOMTranslation();
        window.dispatchEvent(new Event('languageChanged'));
    }
}

window.updateDOMTranslation = function () {
    document.querySelectorAll('[data-i18n]').forEach(el => {
        const key = el.getAttribute('data-i18n');
        if (translations[window.currentLang] && translations[window.currentLang][key]) {
            if (el.tagName === 'INPUT' && el.hasAttribute('placeholder')) {
                el.placeholder = translations[window.currentLang][key];
            } else {
                // If the element has children, textContent will wipe them out.
                // We assume data-i18n elements only contain text, or an SVG inside a button.
                // If it's a button with an SVG icon, we should replace a specific text node or inner wrapper.
                // For simplicity, if we have an SVG, let's keep it.
                let svg = el.querySelector('svg');
                if (svg) {
                    el.innerHTML = '';
                    el.appendChild(svg);
                    el.appendChild(document.createTextNode(' ' + translations[window.currentLang][key]));
                } else {
                    el.textContent = translations[window.currentLang][key];
                }
            }
        }
    });

    document.querySelectorAll('[data-i18n-title]').forEach(el => {
        const key = el.getAttribute('data-i18n-title');
        if (translations[window.currentLang] && translations[window.currentLang][key]) {
            el.title = translations[window.currentLang][key];
        }
    });

    document.querySelectorAll('.lang-btn').forEach(btn => {
        if (btn.dataset.lang === window.currentLang) {
            btn.classList.add('active');
        } else {
            btn.classList.remove('active');
        }
    });
}

document.addEventListener('DOMContentLoaded', () => {
    window.updateDOMTranslation();
});
