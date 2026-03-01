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
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread::JoinHandle;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, COLOR_WINDOW, DIB_RGB_COLORS, EndPaint,
        InvalidateRect, PAINTSTRUCT, SRCCOPY, StretchDIBits,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_ERR,
        CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL,
        ES_PASSWORD, GetMessageW, GetWindowTextLengthW, GetWindowTextW, IDC_ARROW, KillTimer,
        LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW,
        SetTimer, SetWindowTextW, ShowWindow, TranslateMessage, WM_APP, WM_COMMAND, WM_DESTROY,
        WM_PAINT, WM_TIMER, WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
        WS_VISIBLE, WS_VSCROLL,
    };

    const CONTROL_ID_USERNAME_EDIT: i32 = 1001;
    const CONTROL_ID_PASSWORD_EDIT: i32 = 1002;
    const CONTROL_ID_LOGIN_BUTTON: i32 = 1003;
    const CONTROL_ID_CONSENT_CHECKBOX: i32 = 1004;
    const CONTROL_ID_DISPLAY_COMBO: i32 = 1005;
    const CONTROL_ID_START_BUTTON: i32 = 1006;
    const CONTROL_ID_STOP_BUTTON: i32 = 1007;
    const CONTROL_ID_FRAME_STATUS: i32 = 1008;

    const TIMER_CAPTURE_ID: usize = 1;
    const DEFAULT_CAPTURE_FPS: u32 = 1;
    const MOSAIC_JPEG_QUALITY: u8 = 9;
    const PREVIEW_MAX_WIDTH: u32 = 300;
    const PREVIEW_MAX_HEIGHT: u32 = 170;
    const PREVIEW_DRAW_X: i32 = 20;
    const PREVIEW_DRAW_Y: i32 = 500;
    const PREVIEW_DRAW_WIDTH: i32 = 300;
    const PREVIEW_DRAW_HEIGHT: i32 = 170;
    const WM_CAPTURE_WORKER_EVENT: u32 = WM_APP + 1;

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
                if level == "ERROR" {
                    let _ = file.flush();
                }
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
        frame_status: HWND,
    }

    /// Small BGR24 bitmap used for on-window preview rendering.
    struct PreviewBitmap {
        width: i32,
        height: i32,
        bgr24: Vec<u8>,
    }

    /// Files staged for later upload plus lightweight preview bytes.
    struct StagedPayloadArtifacts {
        jpeg_path: PathBuf,
        json_path: PathBuf,
        jpeg_size_bytes: usize,
        json_size_bytes: usize,
        preview_bitmap: PreviewBitmap,
    }

    enum WorkerCommand {
        CaptureTick {
            display_id: String,
            session_id: String,
            captured_at_ms: u64,
        },
        ResetBatch,
        Shutdown,
    }

    enum WorkerEvent {
        TickCaptured {
            frame_number: u64,
            buffered_frames: usize,
            capture_duration_ms: u128,
        },
        BatchPrepared {
            frame_number: u64,
            prepared_batches: u64,
            mosaic_width: u32,
            mosaic_height: u32,
            encode_duration_ms: u128,
            artifacts: StagedPayloadArtifacts,
        },
        WorkerError(String),
    }

    struct CaptureWorkerRuntime {
        command_tx: Sender<WorkerCommand>,
        event_rx: Receiver<WorkerEvent>,
        worker_join: JoinHandle<()>,
    }

    struct AppController {
        ui_state: UiState,
        auth_machine: AuthStateMachine,
        session_token: Option<SessionToken>,
        displays: Vec<DisplayInfo>,
        controls: ControlHandles,
        capturing: bool,
        capture_tick_in_flight: bool,
        capture_timer_interval_ms: u32,
        current_frame_number: u64,
        current_capture_duration_ms: u128,
        current_encode_duration_ms: u128,
        frames_buffered: usize,
        prepared_batches: u64,
        capture_backend_name: String,
        last_prepared_jpeg: Option<PathBuf>,
        last_prepared_json: Option<PathBuf>,
        preview_bitmap: Option<PreviewBitmap>,
        worker_runtime: Option<CaptureWorkerRuntime>,
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
                displays,
                controls: ControlHandles::default(),
                capturing: false,
                capture_tick_in_flight: false,
                capture_timer_interval_ms: 1_000,
                current_frame_number: 0,
                current_capture_duration_ms: 0,
                current_encode_duration_ms: 0,
                frames_buffered: 0,
                prepared_batches: 0,
                capture_backend_name: "real".to_string(),
                last_prepared_jpeg: None,
                last_prepared_json: None,
                preview_bitmap: None,
                worker_runtime: None,
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
            hbrBackground: (COLOR_WINDOW as usize + 1) as *mut c_void,
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
            WM_CAPTURE_WORKER_EVENT => {
                handle_capture_worker_events(hwnd);
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
                    let paint_hdc = BeginPaint(hwnd, &mut paint);
                    draw_preview_bitmap(paint_hdc);
                    EndPaint(hwnd, &paint);
                }
                0
            }
            WM_DESTROY => {
                let _ = with_controller_mut(|controller| {
                    stop_capture_timer(hwnd, controller);
                    shutdown_capture_worker(controller);
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
            controls.frame_status = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "",
                static_style,
                20,
                414,
                920,
                22,
                CONTROL_ID_FRAME_STATUS,
            )?;

            let _preview_label = create_child_control(
                hwnd,
                instance,
                "STATIC",
                "Latest 3x3 mosaic preview (reduced):",
                static_style,
                PREVIEW_DRAW_X,
                470,
                360,
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

            ensure_capture_worker(controller, hwnd)?;

            let fps = capture_fps_from_env();
            let interval_ms = (1_000 / fps.max(1)).max(1);
            if let Some(worker) = controller.worker_runtime.as_ref() {
                worker
                    .command_tx
                    .send(WorkerCommand::ResetBatch)
                    .map_err(|error| format!("capture worker reset command failed: {error}"))?;
            }

            controller.current_frame_number = 0;
            controller.current_capture_duration_ms = 0;
            controller.current_encode_duration_ms = 0;
            controller.frames_buffered = 0;
            controller.prepared_batches = 0;
            controller.last_prepared_jpeg = None;
            controller.last_prepared_json = None;
            controller.preview_bitmap = None;
            controller.capture_tick_in_flight = false;
            controller.ui_state.capture = StageStatus::Running;
            controller.ui_state.network = StageStatus::Idle;
            controller.ui_state.upload = StageStatus::Idle;
            controller.ui_state.analysis_status =
                "Capture started. Preparing first 9-frame mosaic.".to_string();
            unsafe {
                // Safety:
                // - Invalidating client rect triggers redraw and clears old preview.
                InvalidateRect(hwnd, null(), 0);
            }

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
                    "fps={fps} interval_ms={interval_ms} backend={} worker=enabled",
                    controller.capture_backend_name
                ),
            );
            Ok(())
        })
    }

    fn handle_stop_capture(hwnd: HWND) -> Result<(), String> {
        with_controller_mut(|controller| {
            stop_capture_timer(hwnd, controller);
            if let Some(worker) = controller.worker_runtime.as_ref() {
                let _ = worker.command_tx.send(WorkerCommand::ResetBatch);
            }

            controller.capture_tick_in_flight = false;
            controller.frames_buffered = 0;
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

            if controller.capture_tick_in_flight {
                log_info(
                    "capture",
                    "tick_skipped",
                    "worker is still processing previous tick",
                );
                return Ok(());
            }

            let session = controller
                .session_token
                .as_ref()
                .ok_or_else(|| "missing session token for payload preparation".to_string())?;

            let command = WorkerCommand::CaptureTick {
                display_id: selected_display,
                session_id: session.session_id.clone(),
                captured_at_ms: unix_timestamp_millis() as u64,
            };
            let worker = controller
                .worker_runtime
                .as_ref()
                .ok_or_else(|| "capture worker is not initialized".to_string())?;
            worker
                .command_tx
                .send(command)
                .map_err(|error| format!("capture worker send failed: {error}"))?;

            controller.capture_tick_in_flight = true;
            controller.ui_state.upload = StageStatus::Running;
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

    fn handle_capture_worker_events(hwnd: HWND) {
        let result = with_controller_mut(|controller| {
            let mut drained_events = Vec::new();
            {
                let Some(worker) = controller.worker_runtime.as_ref() else {
                    return Ok(());
                };

                loop {
                    match worker.event_rx.try_recv() {
                        Ok(event) => drained_events.push(event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err("capture worker channel disconnected".to_string());
                        }
                    }
                }
            }

            let mut preview_changed = false;
            for event in drained_events {
                match event {
                    WorkerEvent::TickCaptured {
                        frame_number,
                        buffered_frames,
                        capture_duration_ms,
                    } => {
                        controller.capture_tick_in_flight = false;
                        controller.current_frame_number = frame_number;
                        controller.frames_buffered = buffered_frames;
                        controller.current_capture_duration_ms = capture_duration_ms;
                        controller.ui_state.analysis_status = format!(
                            "Captured frame {} (batch position {}/9).",
                            frame_number,
                            buffered_frames.max(1)
                        );

                        log_info(
                            "capture",
                            "frame_acquired",
                            &format!(
                                "frame={} buffered_frames={} capture_ms={}",
                                frame_number, buffered_frames, capture_duration_ms
                            ),
                        );
                    }
                    WorkerEvent::BatchPrepared {
                        frame_number,
                        prepared_batches,
                        mosaic_width,
                        mosaic_height,
                        encode_duration_ms,
                        artifacts,
                    } => {
                        controller.current_frame_number = frame_number;
                        controller.current_encode_duration_ms = encode_duration_ms;
                        controller.prepared_batches = prepared_batches;
                        controller.frames_buffered = 0;
                        controller.last_prepared_jpeg = Some(artifacts.jpeg_path.clone());
                        controller.last_prepared_json = Some(artifacts.json_path.clone());
                        controller.preview_bitmap = Some(artifacts.preview_bitmap);
                        controller.ui_state.upload = StageStatus::Healthy;
                        controller.ui_state.analysis_status = format!(
                            "Prepared batch #{} for upload ({}x{}, jpeg={} bytes, json={} bytes).",
                            prepared_batches,
                            mosaic_width,
                            mosaic_height,
                            artifacts.jpeg_size_bytes,
                            artifacts.json_size_bytes
                        );
                        preview_changed = true;

                        log_info(
                            "upload_prep",
                            "artifact_ready",
                            &format!(
                                "prepared_batches={} jpeg={} json={} encode_ms={}",
                                prepared_batches,
                                artifacts.jpeg_path.display(),
                                artifacts.json_path.display(),
                                encode_duration_ms
                            ),
                        );
                    }
                    WorkerEvent::WorkerError(error) => {
                        controller.capture_tick_in_flight = false;
                        stop_capture_timer(hwnd, controller);
                        controller.ui_state.capture = StageStatus::Degraded;
                        controller.ui_state.upload = StageStatus::Degraded;
                        controller.ui_state.analysis_status =
                            "Capture pipeline failed. Review log for details.".to_string();
                        unsafe {
                            // Safety:
                            // - Start/stop handles are valid.
                            EnableWindow(controller.controls.start_button, 1);
                            EnableWindow(controller.controls.stop_button, 0);
                        }
                        log_error("capture_worker", "failure", &error);
                    }
                }
            }

            if preview_changed {
                unsafe {
                    // Safety:
                    // - Invalidating client rect asks the window to repaint preview region.
                    InvalidateRect(hwnd, null(), 0);
                }
            }

            Ok(())
        });

        if let Err(error) = result {
            log_error("capture_worker", "event_drain", &error);
            let _ = with_controller_mut(|controller| {
                stop_capture_timer(hwnd, controller);
                controller.ui_state.capture = StageStatus::Degraded;
                controller.ui_state.analysis_status = format!("Capture worker error: {error}");
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
                    "Capture: {} | backend={} | running={} | buffered_frames={} | in_flight={} | interval_ms={}",
                    runtime.capture,
                    controller.capture_backend_name,
                    controller.capturing,
                    controller.frames_buffered,
                    controller.capture_tick_in_flight,
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
            set_control_text(
                controller.controls.frame_status,
                &format!(
                    "Frame: total={} | current_batch={}/9 | capture_ms={} | batch_prepare_ms={}",
                    controller.current_frame_number,
                    controller.frames_buffered,
                    controller.current_capture_duration_ms,
                    controller.current_encode_duration_ms
                ),
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

    fn ensure_capture_worker(controller: &mut AppController, hwnd: HWND) -> Result<(), String> {
        if controller.worker_runtime.is_some() {
            return Ok(());
        }

        let worker_runtime = spawn_capture_worker(hwnd)?;
        controller.worker_runtime = Some(worker_runtime);
        log_info("capture_worker", "spawned", "worker thread initialized");
        Ok(())
    }

    fn shutdown_capture_worker(controller: &mut AppController) {
        if let Some(worker_runtime) = controller.worker_runtime.take() {
            let _ = worker_runtime.command_tx.send(WorkerCommand::Shutdown);
            let _ = worker_runtime.worker_join.join();
            log_info("capture_worker", "shutdown", "worker thread joined");
        }
    }

    fn spawn_capture_worker(hwnd: HWND) -> Result<CaptureWorkerRuntime, String> {
        let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();
        let (event_tx, event_rx) = mpsc::channel::<WorkerEvent>();
        let hwnd_value = hwnd as isize;

        let worker_join = std::thread::Builder::new()
            .name("local-guard-capture-worker".to_string())
            .spawn(move || {
                let capture_backend = match RealCaptureBackend::discover() {
                    Ok(backend) => backend,
                    Err(error) => {
                        let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                            "capture backend initialization failed: {error}"
                        )));
                        notify_capture_worker_event(hwnd_value);
                        return;
                    }
                };
                let mut frame_batch = match FrameBatch::new(9) {
                    Ok(batch) => batch,
                    Err(error) => {
                        let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                            "frame batch initialization failed: {error}"
                        )));
                        notify_capture_worker_event(hwnd_value);
                        return;
                    }
                };

                let mut frame_number: u64 = 0;
                let mut prepared_batches: u64 = 0;

                while let Ok(command) = command_rx.recv() {
                    match command {
                        WorkerCommand::CaptureTick {
                            display_id,
                            session_id,
                            captured_at_ms,
                        } => {
                            let capture_started = Instant::now();
                            let frame =
                                match capture_backend.capture_frame(&display_id, captured_at_ms) {
                                    Ok(frame) => frame,
                                    Err(error) => {
                                        let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                                            "frame capture failed: {error}"
                                        )));
                                        notify_capture_worker_event(hwnd_value);
                                        continue;
                                    }
                                };

                            let maybe_batch = match frame_batch.push_frame(frame) {
                                Ok(maybe_batch) => maybe_batch,
                                Err(error) => {
                                    let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                                        "frame batch push failed: {error}"
                                    )));
                                    notify_capture_worker_event(hwnd_value);
                                    continue;
                                }
                            };
                            frame_number = frame_number.saturating_add(1);
                            let buffered_frames = frame_batch.len();
                            let capture_duration_ms = capture_started.elapsed().as_millis();

                            let _ = event_tx.send(WorkerEvent::TickCaptured {
                                frame_number,
                                buffered_frames,
                                capture_duration_ms,
                            });
                            notify_capture_worker_event(hwnd_value);

                            if let Some(batch) = maybe_batch {
                                let prepare_started = Instant::now();
                                let payload = match batch_to_payload(&batch, &session_id) {
                                    Ok(payload) => payload,
                                    Err(error) => {
                                        let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                                            "batch-to-payload failed: {error}"
                                        )));
                                        notify_capture_worker_event(hwnd_value);
                                        continue;
                                    }
                                };
                                let staged = match stage_payload_for_upload(&payload) {
                                    Ok(staged) => staged,
                                    Err(error) => {
                                        let _ = event_tx.send(WorkerEvent::WorkerError(format!(
                                            "artifact staging failed: {error}"
                                        )));
                                        notify_capture_worker_event(hwnd_value);
                                        continue;
                                    }
                                };

                                prepared_batches = prepared_batches.saturating_add(1);
                                let _ = event_tx.send(WorkerEvent::BatchPrepared {
                                    frame_number,
                                    prepared_batches,
                                    mosaic_width: payload.mosaic_width,
                                    mosaic_height: payload.mosaic_height,
                                    encode_duration_ms: prepare_started.elapsed().as_millis(),
                                    artifacts: staged,
                                });
                                notify_capture_worker_event(hwnd_value);
                            }
                        }
                        WorkerCommand::ResetBatch => {
                            frame_number = 0;
                            prepared_batches = 0;
                            if let Ok(new_batch) = FrameBatch::new(9) {
                                frame_batch = new_batch;
                            }
                        }
                        WorkerCommand::Shutdown => break,
                    }
                }
            })
            .map_err(|error| format!("failed to spawn capture worker thread: {error}"))?;

        Ok(CaptureWorkerRuntime {
            command_tx,
            event_rx,
            worker_join,
        })
    }

    fn notify_capture_worker_event(hwnd_value: isize) {
        unsafe {
            // Safety:
            // - Posts a custom message to the UI thread queue; no pointers are transferred.
            PostMessageW(hwnd_value as HWND, WM_CAPTURE_WORKER_EVENT, 0, 0);
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

    fn stage_payload_for_upload(payload: &MosaicPayload) -> Result<StagedPayloadArtifacts, String> {
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
        let jpeg_size_bytes = jpeg_bytes.len();

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
        let json_size_bytes = payload_json.len();
        std::fs::write(&json_path, payload_json)
            .map_err(|error| format!("json artifact write failed: {error}"))?;
        let preview_bitmap =
            build_preview_bitmap(&mosaic_rgb, payload.mosaic_width, payload.mosaic_height)?;

        Ok(StagedPayloadArtifacts {
            jpeg_path,
            json_path,
            jpeg_size_bytes,
            json_size_bytes,
            preview_bitmap,
        })
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

    fn build_preview_bitmap(
        mosaic_rgb: &[u8],
        mosaic_width: u32,
        mosaic_height: u32,
    ) -> Result<PreviewBitmap, String> {
        let source_image =
            image::RgbImage::from_raw(mosaic_width, mosaic_height, mosaic_rgb.to_vec())
                .ok_or_else(|| {
                    format!(
                        "failed to construct RGB image buffer {}x{}",
                        mosaic_width, mosaic_height
                    )
                })?;

        let x_scale = PREVIEW_MAX_WIDTH as f32 / mosaic_width.max(1) as f32;
        let y_scale = PREVIEW_MAX_HEIGHT as f32 / mosaic_height.max(1) as f32;
        let scale = x_scale.min(y_scale).max(0.001);
        let target_width = (mosaic_width as f32 * scale).round().max(1.0) as u32;
        let target_height = (mosaic_height as f32 * scale).round().max(1.0) as u32;

        let preview_image = image::imageops::resize(
            &source_image,
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        );

        let mut bgr24 = Vec::with_capacity((target_width as usize) * (target_height as usize) * 3);
        for pixel in preview_image.pixels() {
            let [r, g, b] = pixel.0;
            bgr24.extend_from_slice(&[b, g, r]);
        }

        Ok(PreviewBitmap {
            width: target_width as i32,
            height: target_height as i32,
            bgr24,
        })
    }

    fn draw_preview_bitmap(paint_hdc: *mut c_void) {
        let _ = with_controller_mut(|controller| {
            let Some(preview_bitmap) = controller.preview_bitmap.as_ref() else {
                return Ok(());
            };

            let mut bitmap_info: BITMAPINFO = unsafe {
                // Safety:
                // - Zeroed `BITMAPINFO` is a valid baseline before header assignment.
                std::mem::zeroed()
            };
            bitmap_info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: preview_bitmap.width,
                // Negative height marks top-down row order.
                biHeight: -preview_bitmap.height,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB,
                ..unsafe {
                    // Safety:
                    // - Remaining fields are optional for BI_RGB source buffers.
                    std::mem::zeroed()
                }
            };

            unsafe {
                // Safety:
                // - Preview buffer remains alive for the duration of the call.
                // - BITMAPINFO header matches BGR24 top-down memory layout.
                StretchDIBits(
                    paint_hdc,
                    PREVIEW_DRAW_X,
                    PREVIEW_DRAW_Y,
                    PREVIEW_DRAW_WIDTH,
                    PREVIEW_DRAW_HEIGHT,
                    0,
                    0,
                    preview_bitmap.width,
                    preview_bitmap.height,
                    preview_bitmap.bgr24.as_ptr() as *const c_void,
                    &bitmap_info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }

            Ok(())
        });
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
