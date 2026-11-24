use crate::shared::simple_handler::*;
use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, extern_methods,
    msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSEvent, NSMenu,
    NSMenuItem, NSUserInterfaceValidations, NSValidatedUserInterfaceItem,
};
use objc2_foundation::{NSBundle, NSObjectNSThreadPerformAdditions, NSString, ns_string};
use std::{cell::Cell, ptr};

fn run_chrome(script: &str) {
    if let Some(handler) = SimpleHandler::instance()
        && let Ok(inner) = handler.lock()
    {
        inner.chrome_execute(0, script);
    }
}

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    pub struct SimpleAppDelegate;

    impl SimpleAppDelegate {
        /// Create the application on the UI thread.
        #[unsafe(method(createApplication:))]
        unsafe fn create_application(&self, _object: Option<&AnyObject>) {
            let app = NSApp(MainThreadMarker::new().expect("Not running on the main thread"));
            assert!(app.isKindOfClass(SimpleApplication::class()));
            assert!(
                app.delegate()
                    .unwrap()
                    .isKindOfClass(SimpleAppDelegate::class())
            );

            let main_bundle = NSBundle::mainBundle();
            let _: Bool = msg_send![&main_bundle,
                loadNibNamed: ns_string!("MainMenu"),
                owner: &*app,
                topLevelObjects: ptr::null_mut::<*const AnyObject>()
            ];
            unsafe {
                self.install_main_menu(MainThreadMarker::new().expect("Not running on the main thread"));
            }
        }

        #[unsafe(method(fdbNewTab:))]
        unsafe fn fdb_new_tab(&self, _sender: &AnyObject) {
            run_chrome("window.fdbNewTab&&fdbNewTab()");
        }

        #[unsafe(method(fdbCloseTab:))]
        unsafe fn fdb_close_tab(&self, _sender: &AnyObject) {
            run_chrome("window.fdbCloseActive&&fdbCloseActive()");
        }

        #[unsafe(method(fdbFocusUrl:))]
        unsafe fn fdb_focus_url(&self, _sender: &AnyObject) {
            if let Some(handler) = SimpleHandler::instance()
                && let Ok(inner) = handler.lock() {
                    inner.chrome_focus("window.fdbFocusUrl&&fdbFocusUrl()");
                }
        }

        #[unsafe(method(fdbBack:))]
        unsafe fn fdb_back(&self, _sender: &AnyObject) {
            run_chrome("window.fdbBack&&fdbBack()");
        }

        #[unsafe(method(fdbForward:))]
        unsafe fn fdb_forward(&self, _sender: &AnyObject) {
            run_chrome("window.fdbForward&&fdbForward()");
        }

        #[unsafe(method(fdbReload:))]
        unsafe fn fdb_reload(&self, _sender: &AnyObject) {
            run_chrome("window.fdbReload&&fdbReload()");
        }

        #[unsafe(method(fdbPrevTab:))]
        unsafe fn fdb_prev_tab(&self, _sender: &AnyObject) {
            run_chrome("window.fdbCycleTab&&fdbCycleTab(-1)");
        }

        #[unsafe(method(fdbNextTab:))]
        unsafe fn fdb_next_tab(&self, _sender: &AnyObject) {
            run_chrome("window.fdbCycleTab&&fdbCycleTab(1)");
        }

        #[unsafe(method(fdbSettings:))]
        unsafe fn fdb_settings(&self, _sender: &AnyObject) {
            run_chrome("window.fdbOpenSettings&&fdbOpenSettings()");
        }

        #[unsafe(method(fdbChat:))]
        unsafe fn fdb_chat(&self, _sender: &AnyObject) {
            run_chrome("window.fdbToggleChat&&fdbToggleChat()");
        }
    }

    unsafe impl NSObjectProtocol for SimpleAppDelegate {}

    unsafe impl NSApplicationDelegate for SimpleAppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        unsafe fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            NSApplicationTerminateReply::TerminateNow
        }

        /// Called when the user clicks the app dock icon while the application is
        /// already running.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        unsafe fn application_should_handle_reopen(&self, _sender: &NSApplication, _has_visible_windows: Bool) -> Bool {
            if let Some(handler) = SimpleHandler::instance() {
                let mut handler = handler.lock().expect("Failed to lock SimpleHandler");
                if !handler.is_closing() {
                    handler.show_main_window();
                }
            }
            Bool::NO
        }

        /// Requests that any state restoration archive be created with secure encoding
        /// (macOS 12+ only). See https://crrev.com/c737387656 for details. This also
        /// fixes an issue with macOS default behavior incorrectly restoring windows
        /// after hard reset (holding down the power button).
        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        unsafe fn application_supports_secure_restorable_state(&self, _sender: &NSApplication) -> Bool {
            Bool::YES
        }
    }

    unsafe impl NSUserInterfaceValidations for SimpleAppDelegate {
        #[unsafe(method(validateUserInterfaceItem:))]
        unsafe fn validate_user_interface_item(&self, item: &ProtocolObject<dyn NSValidatedUserInterfaceItem>) -> Bool {
            const IDC_FIND: isize = 37000;

            let tag = item.tag();
            if tag == IDC_FIND {
                Bool::YES
            } else {
                Bool::NO
            }
        }
    }
}

impl SimpleAppDelegate {
    unsafe fn install_main_menu(&self, mtm: MainThreadMarker) {
        unsafe {
            let main = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("MainMenu"));
            main.setAutoenablesItems(false);

            let app_item = main.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Free Domain Browser"),
                None,
                &NSString::from_str(""),
            );
            let app_menu = NSMenu::initWithTitle(
                NSMenu::alloc(mtm),
                &NSString::from_str("Free Domain Browser"),
            );
            let about = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("About Free Domain Browser"),
                Some(sel!(orderFrontStandardAboutPanel:)),
                &NSString::from_str(""),
            );
            about.setTarget(None);
            app_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let services_item = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Services"),
                None,
                &NSString::from_str(""),
            );
            let services_menu =
                NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Services"));
            services_item.setSubmenu(Some(&services_menu));
            NSApp(mtm).setServicesMenu(Some(&services_menu));
            app_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let hide = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Hide Free Domain Browser"),
                Some(sel!(hide:)),
                &NSString::from_str("h"),
            );
            hide.setTarget(None);
            let hide_others = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Hide Others"),
                Some(sel!(hideOtherApplications:)),
                &NSString::from_str("h"),
            );
            hide_others.setTarget(None);
            hide_others.setKeyEquivalentModifierMask(
                objc2_app_kit::NSEventModifierFlags::Command
                    | objc2_app_kit::NSEventModifierFlags::Option,
            );
            let show_all = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Show All"),
                Some(sel!(unhideAllApplications:)),
                &NSString::from_str(""),
            );
            show_all.setTarget(None);
            app_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let quit = app_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Quit Free Domain Browser"),
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            );
            quit.setTarget(None);
            app_item.setSubmenu(Some(&app_menu));

            let file_item = main.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Browser"),
                None,
                &NSString::from_str(""),
            );
            let file_menu =
                NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Browser"));
            for (title, selector, key) in [
                ("Back", sel!(fdbBack:), "["),
                ("Forward", sel!(fdbForward:), "]"),
                ("Reload", sel!(fdbReload:), "r"),
            ] {
                let item = file_menu.addItemWithTitle_action_keyEquivalent(
                    &NSString::from_str(title),
                    Some(selector),
                    &NSString::from_str(key),
                );
                let _: () = msg_send![&item, setTarget: self];
            }
            file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let new_tab = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("New Tab"),
                Some(sel!(fdbNewTab:)),
                &NSString::from_str("t"),
            );
            let _: () = msg_send![&new_tab, setTarget: self];
            let close_tab = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Close Tab"),
                Some(sel!(fdbCloseTab:)),
                &NSString::from_str("w"),
            );
            let _: () = msg_send![&close_tab, setTarget: self];
            let prev_tab = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Previous Tab"),
                Some(sel!(fdbPrevTab:)),
                &NSString::from_str("\t"),
            );
            let _: () = msg_send![&prev_tab, setTarget: self];
            prev_tab.setKeyEquivalentModifierMask(
                objc2_app_kit::NSEventModifierFlags::Control
                    | objc2_app_kit::NSEventModifierFlags::Shift,
            );
            let next_tab = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Next Tab"),
                Some(sel!(fdbNextTab:)),
                &NSString::from_str("\t"),
            );
            let _: () = msg_send![&next_tab, setTarget: self];
            next_tab.setKeyEquivalentModifierMask(objc2_app_kit::NSEventModifierFlags::Control);
            file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let open_location = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Open Location…"),
                Some(sel!(fdbFocusUrl:)),
                &NSString::from_str("l"),
            );
            let _: () = msg_send![&open_location, setTarget: self];
            file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            let appearance = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Appearance"),
                Some(sel!(fdbSettings:)),
                &NSString::from_str(","),
            );
            let _: () = msg_send![&appearance, setTarget: self];
            // The chat belongs to the menu on every platform: the other
            // platforms reach it through the menu window.
            let chat = file_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Community Chat"),
                Some(sel!(fdbChat:)),
                &NSString::from_str(""),
            );
            let _: () = msg_send![&chat, setTarget: self];
            file_item.setSubmenu(Some(&file_menu));

            let edit_item = main.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Edit"),
                None,
                &NSString::from_str(""),
            );
            let edit_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Edit"));
            for (title, selector, key) in [
                ("Undo", sel!(undo:), "z"),
                ("Redo", sel!(redo:), "Z"),
                ("Cut", sel!(cut:), "x"),
                ("Copy", sel!(copy:), "c"),
                ("Paste", sel!(paste:), "v"),
                ("Select All", sel!(selectAll:), "a"),
            ] {
                let item = edit_menu.addItemWithTitle_action_keyEquivalent(
                    &NSString::from_str(title),
                    Some(selector),
                    &NSString::from_str(key),
                );
                item.setTarget(None);
            }
            edit_item.setSubmenu(Some(&edit_menu));

            let window_item = main.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Window"),
                None,
                &NSString::from_str(""),
            );
            let window_menu =
                NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Window"));
            let minimize = window_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Minimize"),
                Some(sel!(performMiniaturize:)),
                &NSString::from_str("m"),
            );
            minimize.setTarget(None);
            let zoom = window_menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str("Zoom"),
                Some(sel!(performZoom:)),
                &NSString::from_str(""),
            );
            zoom.setTarget(None);
            window_item.setSubmenu(Some(&window_menu));

            NSApp(mtm).setMainMenu(Some(&main));
            debug_assert!(NSApp(mtm).mainMenu().is_some());
            debug_assert!(main.numberOfItems() >= 4);
        }
    }

    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = SimpleAppDelegate::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Instance variables of `SimpleApplication`.
#[derive(Default)]
pub struct SimpleApplicationIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    /// A `NSApplication` subclass that implements the required CEF protocols.
    ///
    /// This class provides the necessary `CefAppProtocol` conformance to
    /// ensure that events are handled correctly by the Chromium framework on macOS.
    #[unsafe(super(NSApplication))]
    #[ivars = SimpleApplicationIvars]
    pub struct SimpleApplication;

    impl SimpleApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let was_sending_event = self.is_handling_send_event();
            if !was_sending_event {
                self.set_handling_send_event(true);
            }

            let _: () = msg_send![super(self), sendEvent:event];

            if !was_sending_event {
                self.set_handling_send_event(false);
            }
        }

        /// |-terminate:| is the entry point for orderly "quit" operations in Cocoa. This
        /// includes the application menu's quit menu item and keyboard equivalent, the
        /// application's dock icon menu's quit menu item, "quit" (not "force quit") in
        /// the Activity Monitor, and quits triggered by user logout and system restart
        /// and shutdown.
        ///
        /// The default |-terminate:| implementation ends the process by calling exit(),
        /// and thus never leaves the main run loop. This is unsuitable for Chromium
        /// since Chromium depends on leaving the main run loop to perform an orderly
        /// shutdown. We support the normal |-terminate:| interface by overriding the
        /// default implementation. Our implementation, which is very specific to the
        /// needs of Chromium, works by asking the application delegate to terminate
        /// using its |-tryToTerminateApplication:| method.
        ///
        /// |-tryToTerminateApplication:| differs from the standard
        /// |-applicationShouldTerminate:| in that no special event loop is run in the
        /// case that immediate termination is not possible (e.g., if dialog boxes
        /// allowing the user to cancel have to be shown). Instead, this method tries to
        /// close all browsers by calling CloseBrowser(false) via
        /// ClientHandler::CloseAllBrowsers. Calling CloseBrowser will result in a call
        /// to ClientHandler::DoClose and execution of |-performClose:| on the NSWindow.
        /// DoClose sets a flag that is used to differentiate between new close events
        /// (e.g., user clicked the window close button) and in-progress close events
        /// (e.g., user approved the close window dialog). The NSWindowDelegate
        /// |-windowShouldClose:| method checks this flag and either calls
        /// CloseBrowser(false) in the case of a new close event or destructs the
        /// NSWindow in the case of an in-progress close event.
        /// ClientHandler::OnBeforeClose will be called after the CEF NSView hosted in
        /// the NSWindow is dealloc'ed.
        ///
        /// After the final browser window has closed ClientHandler::OnBeforeClose will
        /// begin actual tear-down of the application by calling CefQuitMessageLoop.
        /// This ends the NSApplication event loop and execution then returns to the
        /// main() function for cleanup before application termination.
        ///
        /// The standard |-applicationShouldTerminate:| is not supported, and code paths
        /// leading to it must be redirected.
        #[unsafe(method(terminate:))]
        unsafe fn terminate(&self, _sender: &AnyObject) {
            if let Some(handler) = SimpleHandler::instance() {
                let mut handler = handler.lock().expect("Failed to lock SimpleHandler");
                if !handler.is_closing() {
                    handler.close_all_browsers(true);
                }
            }
        }
    }

    unsafe impl CrAppControlProtocol for SimpleApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn _set_handling_send_event(&self, handling_send_event: Bool) {
            self.ivars().handling_send_event.set(handling_send_event);
        }
    }

    unsafe impl CrAppProtocol for SimpleApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn _is_handling_send_event(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CefAppProtocol for SimpleApplication {}
);

impl SimpleApplication {
    extern_methods! {
        #[unsafe(method(sharedApplication))]
        fn shared_application() -> Retained<Self>;

        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling_send_event(&self, handling_send_event: bool);

        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling_send_event(&self) -> bool;
    }
}

pub fn setup_simple_application() {
    // Initialize the SimpleApplication instance.
    // SAFETY: mtm ensures that here is the main thread.
    let _ = SimpleApplication::shared_application();

    // If there was an invocation to NSApp prior to here,
    // then the NSApp will not be a SimpleApplication.
    // The following assertion ensures that this doesn't happen.
    assert!(
        NSApp(MainThreadMarker::new().expect("Not running on the main thread"))
            .isKindOfClass(SimpleApplication::class())
    );
}

pub fn setup_simple_app_delegate() -> Retained<SimpleAppDelegate> {
    let mtm = MainThreadMarker::new().expect("Not running on the main thread");

    // Create the application delegate.
    let simple_delegate = SimpleAppDelegate::new(mtm);
    let delegate_proto =
        ProtocolObject::<dyn NSApplicationDelegate>::from_retained(simple_delegate.clone());
    let app = NSApp(MainThreadMarker::new().expect("Not running on the main thread"));
    assert!(app.isKindOfClass(SimpleApplication::class()));
    app.setDelegate(Some(&delegate_proto));
    assert!(
        app.delegate()
            .unwrap()
            .isKindOfClass(SimpleAppDelegate::class())
    );

    unsafe {
        simple_delegate.performSelectorOnMainThread_withObject_waitUntilDone(
            sel!(createApplication:),
            None,
            false,
        );
    }

    simple_delegate
}
