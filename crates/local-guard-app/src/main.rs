#![warn(missing_docs)]
//! # local-guard-app binary
//!
//! Desktop entry point for local-guard.

/// CLI entry point.
fn main() {
    #[cfg(windows)]
    {
        if let Err(error) = win32_ui::run_main_window() {
            eprintln!("failed to start local-guard UI: {error}");
            std::process::exit(1);
        }
    }

    #[cfg(not(windows))]
    {
        println!("local-guard-app {}", local_guard_app::app_version());
        println!(
            "capture_enabled={} (LOCAL_GUARD_CAPTURE_ENABLED)",
            local_guard_app::capture_enabled_from_env()
        );
    }
}

#[cfg(windows)]
mod win32_ui {
    //! Native Win32 MVP shell with login, consent, display selection, capture
    //! controls, runtime status projection, and per-run file logging.

    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use base64::Engine as _;
    use local_guard_app::{
        app_version, batch_to_payload, capture_enabled_from_env, project_runtime_status,
    };
    use local_guard_auth::{
        AuthClient, AuthError, AuthState, AuthStateMachine, AuthTransport, Credentials,
        LoginRequest, LoginResponse, SessionToken,
    };
    use local_guard_capture::{CaptureBackend, DisplayInfo, RealCaptureBackend};
    use local_guard_core::{FrameBatch, MosaicPayload};
    use local_guard_ui::{StageStatus, UiAuthState, UiState};
    use time::OffsetDateTime;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_ERR,
        CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL,
        ES_PASSWORD, GetMessageW, GetWindowTextLengthW, GetWindowTextW, IDC_ARROW, KillTimer,
        LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW, SetTimer,
        SetWindowTextW, ShowWindow, TranslateMessage, WM_COMMAND, WM_DESTROY, WM_PAINT, WM_TIMER,
        WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    };

    const CONTROL_ID_USERNAME_EDIT: i32 = 1001;
    const CONTROL_ID_PASSWORD_EDIT: i32 = 1002;
    const CONTROL_ID_LOGIN_BUTTON: i32 = 1003;
    const CONTROL_ID_CONSENT_CHECKBOX: i32 = 1004;
    const CONTROL_ID_DISPLAY_COMBO: i32 = 1005;
    const CONTROL_ID_START_BUTTON: i32 = 1006;
    const CONTROL_ID_STOP_BUTTON: i32 = 1007;

    const TIMER_CAPTURE_ID: usize = 1;
    const DEFAULT_CAPTURE_FPS: u32 = 1;
    const MOSAIC_JPEG_QUALITY: u8 = 9;

    const AUTH_ENDPOINT: &str = "https://auth.local-guard.test/r1/cstore-auth";
    static RUN_LOGGER: OnceLock<RunLogger> = OnceLock::new();
    static FIRST_PAINT_LOGGED: AtomicBool = AtomicBool::new(false);

    std::thread_local! {
        static APP_CONTROLLER: RefCell<Option<AppController>> = const { RefCell::new(None) };
    }

    struct RunLogger {
        file: Mutex<File>,
        path: PathBuf,
    }

    impl RunLogger {
        fn new() -> Result<Self, String> {
            let exe_path = std::env::current_exe()
                .map_err(|error| format!("unable to resolve executable path: {error}"))?;
            let exe_dir = exe_path
                .parent()
                .ok_or_else(|| "executable parent directory is missing".to_string())?
                .to_path_buf();

            let timestamp = timestamp_compact_utc();
            let path = exe_dir.join(format!("{timestamp}_log.txt"));
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| {
                    format!("unable to create log file '{}': {error}", path.display())
                })?;

            Ok(Self {
                file: Mutex::new(file),
                path,
            })
        }

        fn write_line(&self, level: &str, stage: &str, action: &str, detail: &str) {
            let timestamp = timestamp_compact_utc();
            let line = format!("{timestamp} | {level} | {stage} | {action} | {detail}\n");

            if let Ok(mut file) = self.file.lock() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }

    #[derive(Default)]
    struct ControlHandles {
        username_edit: HWND,
        password_edit: HWND,
        login_button: HWND,
        consent_checkbox: HWND,
        display_combo: HWND,
        start_button: HWND,
        stop_button: HWND,
        version_status: HWND,
        auth_status: HWND,
        capture_status: HWND,
        network_status: HWND,
        upload_status: HWND,
        analysis_status: HWND,
        pipeline_status: HWND,
    }

    struct AppController {
        ui_state: UiState,
        auth_machine: AuthStateMachine,
        session_token: Option<SessionToken>,
        capture_backend: Box<dyn CaptureBackend>,
        frame_batch: FrameBatch,
        displays: Vec<DisplayInfo>,
        controls: ControlHandles,
        capturing: bool,
        capture_timer_interval_ms: u32,
        frames_buffered: usize,
        prepared_batches: u64,
        capture_backend_name: String,
        last_prepared_jpeg: Option<PathBuf>,
        last_prepared_json: Option<PathBuf>,
    }

    impl AppController {
        fn new() -> Result<Self, String> {
            let capture_backend = RealCaptureBackend::discover()
                .map_err(|error| format!("real display backend initialization failed: {error}"))?;
            let displays = capture_backend.list_displays();
            if displays.is_empty() {
                return Err("real display backend reported zero displays".to_string());
            }

            let mut ui_state = UiState::new(app_version());
            if let Some(first_display) = displays.first() {
                ui_state.select_display(first_display.id.clone());
            }

            Ok(Self {
                ui_state,
                auth_machine: AuthStateMachine::new(),
                session_token: None,
                capture_backend: Box::new(capture_backend),
                frame_batch: FrameBatch::new(9)
                    .map_err(|error| format!("frame batch initialization failed: {error}"))?,
                displays,
                controls: ControlHandles::default(),
                capturing: false,
                capture_timer_interval_ms: 1_000,
                frames_buffered: 0,
                prepared_batches: 0,
                capture_backend_name: "real".to_string(),
                last_prepared_jpeg: None,
                last_prepared_json: None,
            })
        }
    }

    #[derive(Default)]
    struct MockAuthTransport;

    impl AuthTransport for MockAuthTransport {
        fn authenticate(
            &self,
            _endpoint: &str,
            request: &LoginRequest,
        ) -> Result<LoginResponse, AuthError> {
            if request.username.trim().eq_ignore_ascii_case("fail")
                || request.password.trim().eq_ignore_ascii_case("fail")
            {
                return Err(AuthError::Transport(
                    "credentials rejected by mock auth transport".to_string(),
                ));
            }

            Ok(LoginResponse {
                access_token: format!("mock-token-{}", request.username.trim()),
                session_id: format!("session-{}", timestamp_compact_utc()),
                expires_in_seconds: 60 * 30,
            })
        }
    }

    /// Starts the UI event loop and blocks until the user closes the window.
    pub fn run_main_window() -> Result<(), String> {
        initialize_logger()?;
        log_info(
            "bootstrap",
            "startup",
            &format!(
                "version={} capture_enabled={}",
                app_version(),
                capture_enabled_from_env()
            ),
        );

        let controller = AppController::new()?;
        APP_CONTROLLER.with(|slot| {
            *slot.borrow_mut() = Some(controller);
        });

        let instance = unsafe {
            // Safety:
            // - Passing null requests the current process module instance handle.
            GetModuleHandleW(null())
        };
        if instance.is_null() {
            let message = "GetModuleHandleW returned null".to_string();
            log_error("startup", "module_handle", &message);
            return Err(message);
        }

        let class_name = to_wide("LocalGuardMainWindowClass");
        let cursor = unsafe {
            // Safety:
            // - Uses predefined system cursor identifier.
            LoadCursorW(null_mut(), IDC_ARROW)
        };

        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: cursor,
            ..unsafe {
                // Safety:
                // - Zero-initialization for unused optional fields is valid.
                std::mem::zeroed()
            }
        };

        let atom = unsafe {
            // Safety:
            // - `window_class` is fully initialized and points to stable memory.
            RegisterClassW(&window_class)
        };
        if atom == 0 {
            let message = "RegisterClassW failed".to_string();
            log_error("startup", "register_class", &message);
            return Err(message);
        }

        let title = to_wide(&format!("local-guard {}", app_version()));
        let hwnd = unsafe {
            // Safety:
            // - Class and title pointers are valid for the call.
            // - `instance` is a process module handle returned by Win32.
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                980,
                700,
                null_mut(),
                null_mut(),
                instance,
                null_mut(),
            )
        };
        if hwnd.is_null() {
            let message = "CreateWindowExW failed".to_string();
            log_error("startup", "create_window", &message);
            return Err(message);
        }

        create_ui_controls(hwnd, instance)?;

        unsafe {
            // Safety:
            // - `hwnd` is a valid window handle created above.
            ShowWindow(hwnd, SW_SHOW);
        }

        refresh_status_texts().map_err(|error| {
            log_error("ui", "refresh_status_texts", &error);
            error
        })?;

        log_info("event_loop", "begin", "message loop started");
        let mut message: MSG = unsafe {
            // Safety:
            // - Zero-initialization before first `GetMessageW` is valid.
            std::mem::zeroed()
        };

        loop {
            let result = unsafe {
                // Safety:
                // - `message` pointer remains valid across loop iterations.
                GetMessageW(&mut message, null_mut(), 0, 0)
            };
            if result == -1 {
                let message = "GetMessageW returned -1".to_string();
                log_error("event_loop", "get_message", &message);
                return Err(message);
            }
            if result == 0 {
                log_info("event_loop", "end", "WM_QUIT received");
                break;
            }

            unsafe {
                // Safety:
                // - `message` contents came from `GetMessageW`.
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        Ok(())
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        match message {
            WM_COMMAND => {
                handle_command(hwnd, w_param);
                0
            }
            WM_TIMER => {
                if w_param == TIMER_CAPTURE_ID {
                    handle_capture_timer_tick(hwnd);
                }
                0
            }
            WM_PAINT => {
                if !FIRST_PAINT_LOGGED.swap(true, Ordering::Relaxed) {
                    log_info("ui", "first_paint", "first paint message processed");
                }

                let mut paint = unsafe {
                    // Safety:
                    // - Zero-initialization is valid for `PAINTSTRUCT`.
                    std::mem::zeroed::<PAINTSTRUCT>()
                };
                unsafe {
                    // Safety:
                    // - `hwnd` is provided by Win32 for paint processing.
                    BeginPaint(hwnd, &mut paint);
                    EndPaint(hwnd, &paint);
                }
                0
            }
            WM_DESTROY => {
                let _ = with_controller_mut(|controller| {
                    stop_capture_timer(hwnd, controller);
                    Ok(())
                });
                log_info("ui", "destroy", "window destroyed; posting quit");
                unsafe {
                    // Safety:
                    // - Ends the message loop on main thread.
                    PostQuitMessage(0);
                }
                0
            }
            _ => unsafe {
                // Safety:
                // - Delegate unhandled messages to default Win32 behavior.
                DefWindowProcW(hwnd, message, w_param, l_param)
            },
        }
    }

    fn create_ui_controls(hwnd: HWND, instance: *mut c_void) -> Result<(), String> {
        with_controller_mut(|controller| {
            let mut controls = ControlHandles::default();

            let static_style = WS_CHILD | WS_VISIBLE;
            let edit_style = WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32;
            let password_style = edit_style | ES_PASSWORD as u32;
            let button_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32;
            let checkbox_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32;
            let combo_style =
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32;

            let _title = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Local Guard MVP Client",
                static_style,
                20,
                16,
                340,
                24,
                0,
            )?;

            let _username_label = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Username:",
                static_style,
                20,
                56,
                100,
                22,
                0,
            )?;

            controls.username_edit = create_child_control(
                hwnd,
                instance,
                "EDIT",
                "",
                edit_style,
                120,
                54,
                240,
                24,
                CONTROL_ID_USERNAME_EDIT,
            )?;

            let _password_label = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Password:",
                static_style,
                380,
                56,
                100,
                22,
                0,
            )?;

            controls.password_edit = create_child_control(
                hwnd,
                instance,
                "EDIT",
                "",
                password_style,
                480,
                54,
                240,
                24,
                CONTROL_ID_PASSWORD_EDIT,
            )?;

            controls.login_button = create_child_control(
                hwnd,
                instance,
                "BUTTON",
                "Login",
                button_style,
                740,
                52,
                110,
                28,
                CONTROL_ID_LOGIN_BUTTON,
            )?;

            controls.consent_checkbox = create_child_control(
                hwnd,
                instance,
                "BUTTON",
                "I explicitly consent to screen capture.",
                checkbox_style,
                20,
                98,
                320,
                24,
                CONTROL_ID_CONSENT_CHECKBOX,
            )?;

            let _display_label = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Display:",
                static_style,
                20,
                138,
                100,
                22,
                0,
            )?;

            controls.display_combo = create_child_control(
                hwnd,
                instance,
                "COMBOBOX",
                "",
                combo_style,
                120,
                136,
                300,
                220,
                CONTROL_ID_DISPLAY_COMBO,
            )?;

            controls.start_button = create_child_control(
                hwnd,
                instance,
                "BUTTON",
                "Start Capture",
                button_style,
                440,
                134,
                150,
                30,
                CONTROL_ID_START_BUTTON,
            )?;

            controls.stop_button = create_child_control(
                hwnd,
                instance,
                "BUTTON",
                "Stop Capture",
                button_style,
                610,
                134,
                150,
                30,
                CONTROL_ID_STOP_BUTTON,
            )?;

            let _status_title = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Runtime Status",
                static_style,
                20,
                188,
                180,
                22,
                0,
            )?;

            controls.version_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                218,
                760,
                22,
                0,
            )?;
            controls.auth_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                246,
                920,
                22,
                0,
            )?;
            controls.capture_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                274,
                920,
                22,
                0,
            )?;
            controls.network_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                302,
                920,
                22,
                0,
            )?;
            controls.upload_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                330,
                920,
                22,
                0,
            )?;
            controls.analysis_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                358,
                920,
                22,
                0,
            )?;
            controls.pipeline_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                386,
                920,
                22,
                0,
            )?;

            unsafe {
                // Safety:
                // - Checkbox is a valid control handle created above.
                SendMessageW(
                    controls.consent_checkbox,
                    BM_SETCHECK,
                    BST_UNCHECKED as usize,
                    0,
                );
            }

            for display in &controller.displays {
                let label = format!("{} ({}x{})", display.name, display.width, display.height);
                let wide = to_wide(&label);
                unsafe {
                    // Safety:
                    // - Combo box handle is valid; strings are copied by control.
                    SendMessageW(
                        controls.display_combo,
                        CB_ADDSTRING,
                        0,
                        wide.as_ptr() as LPARAM,
                    );
                }
            }

            if !controller.displays.is_empty() {
                unsafe {
                    // Safety:
                    // - Valid combo box handle and valid first index.
                    SendMessageW(controls.display_combo, CB_SETCURSEL, 0, 0);
                }
            }

            unsafe {
                // Safety:
                // - Start/stop button handles are valid.
                EnableWindow(controls.start_button, 1);
                EnableWindow(controls.stop_button, 0);
            }

            controller.controls = controls;
            controller.ui_state.capture = StageStatus::Idle;
            controller.ui_state.network = StageStatus::Idle;
            controller.ui_state.upload = StageStatus::Idle;
            controller.ui_state.analysis_status =
                "Waiting for first 9-frame batch to prepare upload artifact.".to_string();

            log_info(
                "ui",
                "controls_created",
                &format!(
                    "display_count={} capture_backend={}",
                    controller.displays.len(),
                    controller.capture_backend_name
                ),
            );

            Ok(())
        })
    }

    fn handle_command(hwnd: HWND, w_param: WPARAM) {
        let command_bits = w_param;
        let control_id = loword(command_bits) as i32;
        let notification = hiword(command_bits) as u32;

        let result = match control_id {
            CONTROL_ID_LOGIN_BUTTON if notification == BN_CLICKED as u32 => handle_login_click(),
            CONTROL_ID_CONSENT_CHECKBOX if notification == BN_CLICKED as u32 => {
                handle_consent_toggle()
            }
            CONTROL_ID_DISPLAY_COMBO if notification == CBN_SELCHANGE as u32 => {
                handle_display_selection_change()
            }
            CONTROL_ID_START_BUTTON if notification == BN_CLICKED as u32 => {
                handle_start_capture(hwnd)
            }
            CONTROL_ID_STOP_BUTTON if notification == BN_CLICKED as u32 => {
                handle_stop_capture(hwnd)
            }
            _ => Ok(()),
        };

        if let Err(error) = result {
            log_error("ui", "command", &error);
            let _ = with_controller_mut(|controller| {
                controller.ui_state.analysis_status = format!("Error: {error}");
                Ok(())
            });
        }

        let _ = refresh_status_texts();
    }

    fn handle_login_click() -> Result<(), String> {
        with_controller_mut(|controller| {
            let username = read_control_text(controller.controls.username_edit)?;
            let password = read_control_text(controller.controls.password_edit)?;

            log_info(
                "auth",
                "login_attempt",
                &format!(
                    "username_len={} password_len={}",
                    username.trim().len(),
                    password.trim().len()
                ),
            );

            let auth_client =
                AuthClient::new(AUTH_ENDPOINT, Arc::new(MockAuthTransport::default()))
                    .map_err(|error| format!("auth client init failed: {error}"))?;

            let credentials = Credentials { username, password };
            match auth_client.login(&credentials, unix_timestamp_millis() as u64) {
                Ok(token) => {
                    controller.auth_machine.on_login_success(token.clone());
                    controller.session_token = Some(token);
                    sync_auth_state(controller);
                    controller.ui_state.analysis_status =
                        "Login successful. Ready for consent + capture.".to_string();
                    log_info("auth", "login_success", "session established");
                    Ok(())
                }
                Err(error) => {
                    controller.auth_machine.logout();
                    controller.session_token = None;
                    sync_auth_state(controller);
                    controller.ui_state.analysis_status = "Login failed.".to_string();
                    log_error("auth", "login_failed", &error.to_string());
                    Err(format!("login failed: {error}"))
                }
            }
        })
    }

    fn handle_consent_toggle() -> Result<(), String> {
        with_controller_mut(|controller| {
            let checked = unsafe {
                // Safety:
                // - Control handle is valid and belongs to current window.
                SendMessageW(controller.controls.consent_checkbox, BM_GETCHECK, 0, 0)
            } as u32
                == BST_CHECKED;

            controller.ui_state.set_consent(checked);
            log_info("consent", "toggle", &format!("consent_granted={checked}"));
            Ok(())
        })
    }

    fn handle_display_selection_change() -> Result<(), String> {
        with_controller_mut(|controller| {
            let selection_index = unsafe {
                // Safety:
                // - Combo box handle is valid.
                SendMessageW(controller.controls.display_combo, CB_GETCURSEL, 0, 0)
            } as isize;

            if selection_index == CB_ERR as isize {
                return Ok(());
            }

            let selection_index = selection_index as usize;
            if let Some(display) = controller.displays.get(selection_index) {
                controller.ui_state.select_display(display.id.clone());
                log_info(
                    "display",
                    "selection_changed",
                    &format!("selected_display={}", display.id),
                );
            }
            Ok(())
        })
    }

    fn handle_start_capture(hwnd: HWND) -> Result<(), String> {
        with_controller_mut(|controller| {
            sync_auth_state(controller);

            if !capture_enabled_from_env() {
                controller.ui_state.capture = StageStatus::Degraded;
                return Err(
                    "capture blocked by LOCAL_GUARD_CAPTURE_ENABLED kill-switch".to_string()
                );
            }

            if controller.capturing {
                log_info("capture", "start_ignored", "capture already running");
                return Ok(());
            }

            if !controller.ui_state.can_start_capture() {
                return Err(
                    "capture start blocked (requires authenticated session, consent, and display)"
                        .to_string(),
                );
            }

            let fps = capture_fps_from_env();
            let interval_ms = (1_000 / fps.max(1)).max(1);

            controller.frame_batch =
                FrameBatch::new(9).map_err(|error| format!("frame batch reset failed: {error}"))?;
            controller.frames_buffered = 0;
            controller.prepared_batches = 0;
            controller.last_prepared_jpeg = None;
            controller.last_prepared_json = None;
            controller.ui_state.capture = StageStatus::Running;
            controller.ui_state.network = StageStatus::Idle;
            controller.ui_state.upload = StageStatus::Idle;
            controller.ui_state.analysis_status =
                "Capture started. Preparing first 9-frame mosaic.".to_string();

            let timer = unsafe {
                // Safety:
                // - Main window handle is valid, timer id is process-local.
                SetTimer(hwnd, TIMER_CAPTURE_ID, interval_ms, None)
            };
            if timer == 0 {
                controller.ui_state.capture = StageStatus::Degraded;
                return Err("SetTimer failed for capture scheduler".to_string());
            }

            controller.capture_timer_interval_ms = interval_ms;
            controller.capturing = true;

            unsafe {
                // Safety:
                // - Start/stop control handles are valid.
                EnableWindow(controller.controls.start_button, 0);
                EnableWindow(controller.controls.stop_button, 1);
            }

            log_info(
                "capture",
                "start",
                &format!(
                    "fps={fps} interval_ms={interval_ms} backend={}",
                    controller.capture_backend_name
                ),
            );
            Ok(())
        })
    }

    fn handle_stop_capture(hwnd: HWND) -> Result<(), String> {
        with_controller_mut(|controller| {
            stop_capture_timer(hwnd, controller);
            controller.ui_state.capture = StageStatus::Idle;
            controller.ui_state.analysis_status = "Capture stopped.".to_string();

            unsafe {
                // Safety:
                // - Start/stop control handles are valid.
                EnableWindow(controller.controls.start_button, 1);
                EnableWindow(controller.controls.stop_button, 0);
            }

            log_info("capture", "stop", "capture stopped by user action");
            Ok(())
        })
    }

    fn handle_capture_timer_tick(hwnd: HWND) {
        let result = with_controller_mut(|controller| {
            if !controller.capturing {
                return Ok(());
            }

            sync_auth_state(controller);
            if controller.ui_state.auth != UiAuthState::Authenticated {
                stop_capture_timer(hwnd, controller);
                controller.ui_state.capture = StageStatus::Degraded;
                controller.ui_state.analysis_status =
                    "Session expired; reauthentication required.".to_string();
                unsafe {
                    // Safety:
                    // - Start/stop handles are valid.
                    EnableWindow(controller.controls.start_button, 1);
                    EnableWindow(controller.controls.stop_button, 0);
                }
                log_error(
                    "capture",
                    "session_invalid",
                    "capture stopped due to expired auth",
                );
                return Ok(());
            }

            let selected_display = controller
                .ui_state
                .selected_display
                .clone()
                .ok_or_else(|| "no display selected for capture tick".to_string())?;

            let frame = controller
                .capture_backend
                .capture_frame(&selected_display, unix_timestamp_millis() as u64)
                .map_err(|error| format!("frame capture failed: {error}"))?;

            let maybe_batch = controller
                .frame_batch
                .push_frame(frame)
                .map_err(|error| format!("frame batch push failed: {error}"))?;
            controller.frames_buffered = controller.frame_batch.len();

            log_info(
                "capture",
                "frame_acquired",
                &format!(
                    "display={} buffered_frames={}",
                    selected_display, controller.frames_buffered
                ),
            );

            if let Some(batch) = maybe_batch {
                controller.ui_state.upload = StageStatus::Running;
                controller.ui_state.network = StageStatus::Idle;

                let session = controller
                    .session_token
                    .as_ref()
                    .ok_or_else(|| "missing session token for payload preparation".to_string())?;

                let payload = batch_to_payload(&batch, &session.session_id)
                    .map_err(|error| format!("batch-to-payload failed: {error}"))?;

                log_info(
                    "mosaic",
                    "payload_ready",
                    &format!(
                        "width={} height={} rgba_bytes={}",
                        payload.mosaic_width,
                        payload.mosaic_height,
                        payload.mosaic_rgba.len()
                    ),
                );

                match stage_payload_for_upload(&payload) {
                    Ok((jpeg_path, json_path)) => {
                        controller.ui_state.upload = StageStatus::Healthy;
                        controller.prepared_batches = controller.prepared_batches.saturating_add(1);
                        controller.last_prepared_jpeg = Some(jpeg_path.clone());
                        controller.last_prepared_json = Some(json_path.clone());
                        controller.ui_state.analysis_status = format!(
                            "Prepared batch #{} for upload (endpoint not configured).",
                            controller.prepared_batches
                        );

                        log_info(
                            "upload_prep",
                            "artifact_ready",
                            &format!(
                                "prepared_batches={} jpeg={} json={}",
                                controller.prepared_batches,
                                jpeg_path.display(),
                                json_path.display()
                            ),
                        );
                    }
                    Err(error) => {
                        controller.ui_state.upload = StageStatus::Degraded;
                        controller.ui_state.analysis_status =
                            "Failed to prepare upload artifact.".to_string();
                        log_error("upload_prep", "artifact_failed", &error);
                    }
                }
            }

            Ok(())
        });

        if let Err(error) = result {
            log_error("capture", "timer_tick", &error);
            let _ = with_controller_mut(|controller| {
                stop_capture_timer(hwnd, controller);
                controller.ui_state.capture = StageStatus::Degraded;
                controller.ui_state.analysis_status = format!("Capture error: {error}");
                unsafe {
                    // Safety:
                    // - Start/stop handles are valid.
                    EnableWindow(controller.controls.start_button, 1);
                    EnableWindow(controller.controls.stop_button, 0);
                }
                Ok(())
            });
        }

        let _ = refresh_status_texts();
    }

    fn refresh_status_texts() -> Result<(), String> {
        with_controller_mut(|controller| {
            sync_auth_state(controller);

            let runtime = project_runtime_status(&controller.ui_state);
            let auth_label = match controller.ui_state.auth {
                UiAuthState::Unauthenticated => "Unauthenticated",
                UiAuthState::Authenticated => "Authenticated",
                UiAuthState::ReauthRequired => "ReauthRequired",
            };

            set_control_text(
                controller.controls.version_status,
                &format!("Version: {}", controller.ui_state.version),
            );
            set_control_text(
                controller.controls.auth_status,
                &format!("Auth: {auth_label}"),
            );
            set_control_text(
                controller.controls.capture_status,
                &format!(
                    "Capture: {} | backend={} | running={} | buffered_frames={} | interval_ms={}",
                    runtime.capture,
                    controller.capture_backend_name,
                    controller.capturing,
                    controller.frames_buffered,
                    controller.capture_timer_interval_ms
                ),
            );
            set_control_text(
                controller.controls.network_status,
                &format!("Network: {} (endpoint not configured)", runtime.network),
            );
            set_control_text(
                controller.controls.upload_status,
                &format!(
                    "Upload Prep: {} | prepared_batches={}",
                    runtime.upload, controller.prepared_batches
                ),
            );
            set_control_text(
                controller.controls.analysis_status,
                &format!("Analysis: {}", runtime.analysis),
            );

            let selected_display = controller
                .ui_state
                .selected_display
                .as_deref()
                .unwrap_or("None");
            let last_jpeg = controller
                .last_prepared_jpeg
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "none".to_string());
            let last_json = controller
                .last_prepared_json
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "none".to_string());
            set_control_text(
                controller.controls.pipeline_status,
                &format!(
                    "Consent={} | Display={} | CaptureAllowed={} | KillSwitchEnabled={} | LastJpeg={} | LastJson={}",
                    controller.ui_state.consent_granted,
                    selected_display,
                    runtime.capture_allowed,
                    capture_enabled_from_env(),
                    last_jpeg,
                    last_json,
                ),
            );

            Ok(())
        })
    }

    fn sync_auth_state(controller: &mut AppController) {
        controller
            .auth_machine
            .on_tick(unix_timestamp_millis() as u64);
        controller.ui_state.auth = match controller.auth_machine.state() {
            AuthState::Unauthenticated => UiAuthState::Unauthenticated,
            AuthState::Authenticated(_) => UiAuthState::Authenticated,
            AuthState::ReauthRequired => UiAuthState::ReauthRequired,
        };

        if controller.ui_state.auth != UiAuthState::Authenticated {
            controller.session_token = None;
        }
    }

    fn stop_capture_timer(hwnd: HWND, controller: &mut AppController) {
        if controller.capturing {
            unsafe {
                // Safety:
                // - Timer ID is owned by this window and can be cancelled here.
                KillTimer(hwnd, TIMER_CAPTURE_ID);
            }
            controller.capturing = false;
        }
    }

    fn with_controller_mut<F, T>(f: F) -> Result<T, String>
    where
        F: FnOnce(&mut AppController) -> Result<T, String>,
    {
        APP_CONTROLLER.with(|slot| {
            let mut maybe_controller = slot.borrow_mut();
            let controller = maybe_controller
                .as_mut()
                .ok_or_else(|| "app controller is not initialized".to_string())?;
            f(controller)
        })
    }

    fn capture_fps_from_env() -> u32 {
        std::env::var("LOCAL_GUARD_CAPTURE_FPS")
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|fps| *fps > 0)
            .unwrap_or(DEFAULT_CAPTURE_FPS)
    }

    fn stage_payload_for_upload(payload: &MosaicPayload) -> Result<(PathBuf, PathBuf), String> {
        let base_dir = runtime_artifact_dir()?;
        std::fs::create_dir_all(&base_dir)
            .map_err(|error| format!("artifact directory create failed: {error}"))?;

        let stamp = timestamp_compact_utc();
        let jpeg_path = base_dir.join(format!("{stamp}_mosaic.jpg"));
        let json_path = base_dir.join(format!("{stamp}_payload.json"));
        let mosaic_rgb = rgba_to_rgb(&payload.mosaic_rgba)?;
        let mut jpeg_bytes = Vec::new();

        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, MOSAIC_JPEG_QUALITY)
            .encode(
                &mosaic_rgb,
                payload.mosaic_width,
                payload.mosaic_height,
                image::ColorType::Rgb8.into(),
            )
            .map_err(|error| format!("jpeg encoding failed: {error}"))?;

        std::fs::write(&jpeg_path, &jpeg_bytes)
            .map_err(|error| format!("jpeg artifact write failed: {error}"))?;

        let payload_json = serde_json::json!({
            "schema_version": payload.schema_version,
            "metadata": payload.metadata,
            "mosaic_width": payload.mosaic_width,
            "mosaic_height": payload.mosaic_height,
            "mosaic_format": "jpeg",
            "mosaic_color_space": "RGB",
            "mosaic_jpeg_quality": MOSAIC_JPEG_QUALITY,
            "mosaic_jpeg_base64": base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes),
        });
        let payload_json = serde_json::to_vec(&payload_json)
            .map_err(|error| format!("json encode failed: {error}"))?;
        std::fs::write(&json_path, payload_json)
            .map_err(|error| format!("json artifact write failed: {error}"))?;

        Ok((jpeg_path, json_path))
    }

    fn rgba_to_rgb(rgba: &[u8]) -> Result<Vec<u8>, String> {
        if rgba.len() % 4 != 0 {
            return Err(format!(
                "invalid RGBA buffer length {}; expected multiple of 4",
                rgba.len()
            ));
        }

        let mut rgb = Vec::with_capacity((rgba.len() / 4) * 3);
        for px in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
        }

        Ok(rgb)
    }

    fn runtime_artifact_dir() -> Result<PathBuf, String> {
        let exe_path = std::env::current_exe()
            .map_err(|error| format!("failed to resolve executable path: {error}"))?;
        let exe_dir = exe_path
            .parent()
            .ok_or_else(|| "failed to resolve executable directory".to_string())?;
        Ok(exe_dir.join("prepared_uploads"))
    }

    fn create_child_control(
        parent: HWND,
        instance: *mut c_void,
        class_name: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        control_id: i32,
    ) -> Result<HWND, String> {
        let class_name_wide = to_wide(class_name);
        let text_wide = to_wide(text);

        let hwnd = unsafe {
            // Safety:
            // - Input pointers are stable for this call and parent/instance handles are valid.
            CreateWindowExW(
                0,
                class_name_wide.as_ptr(),
                text_wide.as_ptr(),
                style,
                x,
                y,
                width,
                height,
                parent,
                control_id_to_hmenu(control_id),
                instance,
                null(),
            )
        };

        if hwnd.is_null() {
            return Err(format!(
                "failed to create control class={class_name} id={control_id}"
            ));
        }

        Ok(hwnd)
    }

    fn set_control_text(control: HWND, text: &str) {
        let wide = to_wide(text);
        unsafe {
            // Safety:
            // - `control` is a live child HWND and UTF-16 pointer is valid for call.
            SetWindowTextW(control, wide.as_ptr());
        }
    }

    fn read_control_text(control: HWND) -> Result<String, String> {
        let length = unsafe {
            // Safety:
            // - `control` is a valid edit control handle.
            GetWindowTextLengthW(control)
        };
        if length < 0 {
            return Err("GetWindowTextLengthW failed".to_string());
        }

        let mut buffer = vec![0_u16; length as usize + 1];
        let written = unsafe {
            // Safety:
            // - Buffer is large enough for text + null terminator.
            GetWindowTextW(control, buffer.as_mut_ptr(), buffer.len() as i32)
        };
        if written < 0 {
            return Err("GetWindowTextW failed".to_string());
        }

        Ok(String::from_utf16_lossy(&buffer[..written as usize]))
    }

    fn initialize_logger() -> Result<(), String> {
        if RUN_LOGGER.get().is_some() {
            return Ok(());
        }

        let logger = RunLogger::new()?;
        let path = logger.path.display().to_string();
        let _ = RUN_LOGGER.set(logger);
        log_info("logging", "file_created", &format!("log_file={path}"));
        Ok(())
    }

    fn log_info(stage: &str, action: &str, detail: &str) {
        if let Some(logger) = RUN_LOGGER.get() {
            logger.write_line("INFO", stage, action, detail);
        }
    }

    fn log_error(stage: &str, action: &str, detail: &str) {
        if let Some(logger) = RUN_LOGGER.get() {
            logger.write_line("ERROR", stage, action, detail);
        }
    }

    fn control_id_to_hmenu(control_id: i32) -> *mut c_void {
        control_id as usize as *mut c_void
    }

    fn loword(value: usize) -> u16 {
        (value & 0xFFFF) as u16
    }

    fn hiword(value: usize) -> u16 {
        ((value >> 16) & 0xFFFF) as u16
    }

    fn unix_timestamp_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis())
    }

    fn timestamp_compact_utc() -> String {
        let now = OffsetDateTime::now_utc();
        format!(
            "{:04}{:02}{:02}_{:02}{:02}{:02}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
